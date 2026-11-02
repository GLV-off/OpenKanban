use serde::Serialize;
use utoipa::ToSchema;

const SELECT_BY_EMAIL: &str = "SELECT id, email, password_hash, display_name, email_verified, status, created_at, updated_at FROM users WHERE email = ?";
const SELECT_BY_ID: &str = "SELECT id, email, password_hash, display_name, email_verified, status, created_at, updated_at FROM users WHERE id = ?";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: String,
    pub email_verified: bool,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl UserRecord {
    pub fn public(&self) -> PublicUser {
        PublicUser {
            id: self.id.clone(),
            email: self.email.clone(),
            display_name: self.display_name.clone(),
            email_verified: self.email_verified,
            created_at: self.created_at.clone(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == "active"
    }
}

/// User profile safe to return over the API (never contains the password hash).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PublicUser {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub email_verified: bool,
    pub created_at: String,
}

pub async fn find_by_email<'e, E>(
    executor: E,
    email: &str,
) -> Result<Option<UserRecord>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, UserRecord>(SELECT_BY_EMAIL)
        .bind(email)
        .fetch_optional(executor)
        .await
}

pub async fn find_by_id<'e, E>(executor: E, id: &str) -> Result<Option<UserRecord>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, UserRecord>(SELECT_BY_ID)
        .bind(id)
        .fetch_optional(executor)
        .await
}
