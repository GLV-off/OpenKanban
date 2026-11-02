use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};

use crate::{
    auth::token,
    error::AppError,
    state::AppState,
    users::{self, PublicUser},
};

/// Extracts and validates the bearer access token, then loads the current user.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub PublicUser);

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                AppError::unauthorized("MISSING_TOKEN", "authorization header is required")
            })?;

        let token = authorization
            .strip_prefix("Bearer ")
            .or_else(|| authorization.strip_prefix("bearer "))
            .ok_or_else(|| {
                AppError::unauthorized(
                    "INVALID_TOKEN",
                    "authorization header must use the Bearer scheme",
                )
            })?;

        let claims = token::verify_access_token(&state.config, token)?;

        let Some(user) = users::find_by_id(&state.pool, &claims.sub).await? else {
            return Err(AppError::unauthorized(
                "INVALID_TOKEN",
                "access token subject does not exist",
            ));
        };

        if !user.is_active() {
            return Err(AppError::forbidden(
                "ACCOUNT_DISABLED",
                "account is disabled",
            ));
        }

        Ok(Self(user.public()))
    }
}
