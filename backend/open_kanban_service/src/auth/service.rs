use tokio::task;
use uuid::Uuid;

use crate::{
    auth::{password, token},
    clock,
    error::AppError,
    state::AppState,
    users::{self, PublicUser, UserRecord},
};

#[derive(Debug)]
pub struct IssuedSession {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
    pub refresh_token: String,
    pub user: PublicUser,
}

pub async fn register(
    state: &AppState,
    email: &str,
    password: &str,
    display_name: &str,
    user_agent: Option<&str>,
) -> Result<IssuedSession, AppError> {
    let email = normalize_email(email)?;
    let display_name = normalize_display_name(display_name)?;
    password::validate_password(password)?;

    let password = password.to_owned();
    let password_hash = task::spawn_blocking(move || password::hash_password(&password))
        .await
        .map_err(|error| {
            AppError::Internal(anyhow::anyhow!("password hashing task failed: {error}"))
        })??;

    let id = Uuid::now_v7().to_string();
    let now = clock::now_rfc3339();

    let inserted = sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name, email_verified, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, 0, 'active', ?, ?)",
    )
    .bind(&id)
    .bind(&email)
    .bind(&password_hash)
    .bind(&display_name)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await;

    match inserted {
        Ok(_) => {}
        Err(sqlx::Error::Database(database_error)) if database_error.is_unique_violation() => {
            return Err(AppError::conflict(
                "EMAIL_TAKEN",
                "email is already registered",
            ));
        }
        Err(error) => return Err(error.into()),
    }

    let user = UserRecord {
        id,
        email,
        password_hash,
        display_name,
        email_verified: false,
        status: "active".to_owned(),
        created_at: now.clone(),
        updated_at: now,
    };

    issue_session(state, &user, user_agent).await
}

pub async fn login(
    state: &AppState,
    email: &str,
    password: &str,
    user_agent: Option<&str>,
) -> Result<IssuedSession, AppError> {
    let email = normalize_email(email)?;
    let throttle = state
        .throttle
        .check(&email, state.config.login_max_attempts);
    if !throttle.allowed {
        return Err(AppError::too_many_requests(throttle.retry_after_seconds));
    }

    let Some(user) = users::find_by_email(&state.pool, &email).await? else {
        state.throttle.record_failure(&email);
        return Err(invalid_credentials());
    };

    let password = password.to_owned();
    let password_hash = user.password_hash.clone();
    let valid = task::spawn_blocking(move || password::verify_password(&password, &password_hash))
        .await
        .map_err(|error| {
            AppError::Internal(anyhow::anyhow!(
                "password verification task failed: {error}"
            ))
        })?;

    if !valid {
        state.throttle.record_failure(&email);
        return Err(invalid_credentials());
    }
    if !user.is_active() {
        return Err(AppError::forbidden(
            "ACCOUNT_DISABLED",
            "account is disabled",
        ));
    }

    state.throttle.reset(&email);
    issue_session(state, &user, user_agent).await
}

pub async fn refresh(
    state: &AppState,
    refresh_token: &str,
    user_agent: Option<&str>,
) -> Result<IssuedSession, AppError> {
    let token_hash = token::hash_refresh_token(refresh_token);
    let next_token = token::generate_refresh_token();
    let next_session_id = Uuid::now_v7().to_string();
    let now = clock::now_rfc3339();
    let next_expires_at = clock::now_plus_seconds_rfc3339(state.config.refresh_token_ttl_seconds);

    let mut transaction = state.pool.begin().await?;

    let session: Option<SessionRow> = sqlx::query_as(
        "SELECT id, user_id, expires_at, revoked_at FROM sessions WHERE token_hash = ?",
    )
    .bind(&token_hash)
    .fetch_optional(&mut *transaction)
    .await?;

    let Some(session) = session else {
        return Err(invalid_refresh_token());
    };

    // A rotated token that is presented again means the token leaked (or was replayed):
    // revoke every active session of the user.
    if session.revoked_at.is_some() {
        sqlx::query("UPDATE sessions SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL")
            .bind(&now)
            .bind(&session.user_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        return Err(invalid_refresh_token());
    }

    if clock::is_in_past(&session.expires_at) {
        sqlx::query("UPDATE sessions SET revoked_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&session.id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        return Err(invalid_refresh_token());
    }

    let Some(user) = users::find_by_id(&mut *transaction, &session.user_id).await? else {
        return Err(invalid_refresh_token());
    };

    if !user.is_active() {
        return Err(AppError::forbidden(
            "ACCOUNT_DISABLED",
            "account is disabled",
        ));
    }

    // Insert first: `sessions.replaced_by` references the next session row.
    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, issued_at, expires_at, user_agent)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&next_session_id)
    .bind(&user.id)
    .bind(&next_token.hash)
    .bind(&now)
    .bind(&next_expires_at)
    .bind(user_agent.map(truncate_user_agent))
    .execute(&mut *transaction)
    .await?;

    let rotated = sqlx::query(
        "UPDATE sessions SET revoked_at = ?, replaced_by = ? WHERE id = ? AND revoked_at IS NULL",
    )
    .bind(&now)
    .bind(&next_session_id)
    .bind(&session.id)
    .execute(&mut *transaction)
    .await?;

    if rotated.rows_affected() != 1 {
        return Err(invalid_refresh_token());
    }

    transaction.commit().await?;

    let (access_token, expires_in) =
        token::issue_access_token(&state.config, &user.id, &next_session_id)?;

    Ok(IssuedSession {
        access_token,
        token_type: token::TOKEN_TYPE,
        expires_in,
        refresh_token: next_token.raw,
        user: user.public(),
    })
}

pub async fn logout(state: &AppState, refresh_token: Option<&str>) -> Result<(), AppError> {
    let Some(refresh_token) = refresh_token else {
        return Ok(());
    };

    let token_hash = token::hash_refresh_token(refresh_token);
    sqlx::query("UPDATE sessions SET revoked_at = ? WHERE token_hash = ? AND revoked_at IS NULL")
        .bind(clock::now_rfc3339())
        .bind(&token_hash)
        .execute(&state.pool)
        .await?;

    Ok(())
}

async fn issue_session(
    state: &AppState,
    user: &UserRecord,
    user_agent: Option<&str>,
) -> Result<IssuedSession, AppError> {
    let refresh_token = token::generate_refresh_token();
    let session_id = Uuid::now_v7().to_string();
    let issued_at = clock::now_rfc3339();
    let expires_at = clock::now_plus_seconds_rfc3339(state.config.refresh_token_ttl_seconds);

    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, issued_at, expires_at, user_agent)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(&user.id)
    .bind(&refresh_token.hash)
    .bind(&issued_at)
    .bind(&expires_at)
    .bind(user_agent.map(truncate_user_agent))
    .execute(&state.pool)
    .await?;

    let (access_token, expires_in) =
        token::issue_access_token(&state.config, &user.id, &session_id)?;

    Ok(IssuedSession {
        access_token,
        token_type: token::TOKEN_TYPE,
        expires_in,
        refresh_token: refresh_token.raw,
        user: user.public(),
    })
}

fn normalize_email(raw: &str) -> Result<String, AppError> {
    let email = raw.trim().to_lowercase();
    let valid = email.len() <= 254
        && email.contains('@')
        && !email.starts_with('@')
        && !email.ends_with('@')
        && !email.chars().any(char::is_whitespace);

    if !valid {
        return Err(AppError::validation_field("email", "email is not valid"));
    }

    Ok(email)
}

fn normalize_display_name(raw: &str) -> Result<String, AppError> {
    let name = raw.trim();

    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::validation_field(
            "display_name",
            "display name must be between 1 and 80 characters",
        ));
    }

    Ok(name.to_owned())
}

fn truncate_user_agent(value: &str) -> String {
    value.chars().take(256).collect()
}

fn invalid_credentials() -> AppError {
    AppError::unauthorized("INVALID_CREDENTIALS", "email or password is incorrect")
}

fn invalid_refresh_token() -> AppError {
    AppError::unauthorized(
        "INVALID_REFRESH_TOKEN",
        "refresh token is invalid or expired",
    )
}

#[derive(Debug, sqlx::FromRow)]
struct SessionRow {
    id: String,
    user_id: String,
    expires_at: String,
    revoked_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_email() {
        assert_eq!(
            normalize_email("  Alice@Example.COM ").expect("valid email"),
            "alice@example.com"
        );
    }

    #[test]
    fn rejects_invalid_emails() {
        assert!(normalize_email("").is_err());
        assert!(normalize_email("no-at-sign").is_err());
        assert!(normalize_email("@example.com").is_err());
        assert!(normalize_email("alice@").is_err());
        assert!(normalize_email("alice @example.com").is_err());
    }

    #[test]
    fn validates_display_name() {
        assert!(normalize_display_name("  Alice  ").is_ok());
        assert!(normalize_display_name("   ").is_err());
        assert!(normalize_display_name(&"x".repeat(81)).is_err());
    }
}
