use axum::{Json, extract::State};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

impl HealthResponse {
    fn new(status: &str) -> Self {
        Self {
            status: status.to_owned(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/healthz",
    tag = "system",
    summary = "Liveness probe",
    responses((status = 200, description = "Service process is alive", body = HealthResponse))
)]
pub async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse::new("ok"))
}

#[utoipa::path(
    get,
    path = "/readyz",
    tag = "system",
    summary = "Readiness probe",
    responses(
        (status = 200, description = "Service can serve traffic", body = HealthResponse),
        (status = 503, description = "Database is not reachable", body = crate::error::ProblemDetails)
    )
)]
pub async fn readyz(State(state): State<AppState>) -> Result<Json<HealthResponse>, AppError> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(|error| {
            tracing::error!(%error, "readiness check failed");
            AppError::unavailable("database is not reachable")
        })?;

    Ok(Json(HealthResponse::new("ready")))
}
