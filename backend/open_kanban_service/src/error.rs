use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

/// Machine-readable error payload (RFC 9457, `application/problem+json`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<FieldError>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("method not allowed")]
    MethodNotAllowed,
    #[error("{detail}")]
    BadRequest { code: &'static str, detail: String },
    #[error("validation failed")]
    Validation(Vec<FieldError>),
    #[error("{detail}")]
    Unauthorized { code: &'static str, detail: String },
    #[error("{detail}")]
    Forbidden { code: &'static str, detail: String },
    #[error("{detail}")]
    Conflict { code: &'static str, detail: String },
    #[error("too many requests")]
    TooManyRequests { retry_after_seconds: u64 },
    #[error("service unavailable: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn bad_request(code: &'static str, detail: impl Into<String>) -> Self {
        Self::BadRequest {
            code,
            detail: detail.into(),
        }
    }

    pub fn unauthorized(code: &'static str, detail: impl Into<String>) -> Self {
        Self::Unauthorized {
            code,
            detail: detail.into(),
        }
    }

    pub fn forbidden(code: &'static str, detail: impl Into<String>) -> Self {
        Self::Forbidden {
            code,
            detail: detail.into(),
        }
    }

    pub fn conflict(code: &'static str, detail: impl Into<String>) -> Self {
        Self::Conflict {
            code,
            detail: detail.into(),
        }
    }

    pub fn validation_field(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation(vec![FieldError {
            field: field.into(),
            message: message.into(),
        }])
    }

    pub fn too_many_requests(retry_after_seconds: u64) -> Self {
        Self::TooManyRequests {
            retry_after_seconds,
        }
    }

    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self::Unavailable(detail.into())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, title, detail, errors, retry_after_seconds) = match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Not found",
                None,
                Vec::new(),
                None,
            ),
            Self::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                "METHOD_NOT_ALLOWED",
                "Method not allowed",
                None,
                Vec::new(),
                None,
            ),
            Self::BadRequest { code, detail } => (
                StatusCode::BAD_REQUEST,
                code,
                "Bad request",
                Some(detail),
                Vec::new(),
                None,
            ),
            Self::Validation(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR",
                "Validation failed",
                None,
                errors,
                None,
            ),
            Self::Unauthorized { code, detail } => (
                StatusCode::UNAUTHORIZED,
                code,
                "Unauthorized",
                Some(detail),
                Vec::new(),
                None,
            ),
            Self::Forbidden { code, detail } => (
                StatusCode::FORBIDDEN,
                code,
                "Forbidden",
                Some(detail),
                Vec::new(),
                None,
            ),
            Self::Conflict { code, detail } => (
                StatusCode::CONFLICT,
                code,
                "Conflict",
                Some(detail),
                Vec::new(),
                None,
            ),
            Self::TooManyRequests {
                retry_after_seconds,
            } => (
                StatusCode::TOO_MANY_REQUESTS,
                "TOO_MANY_REQUESTS",
                "Too many requests",
                Some(format!("retry after {retry_after_seconds} seconds")),
                Vec::new(),
                Some(retry_after_seconds),
            ),
            Self::Unavailable(detail) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                "Service unavailable",
                Some(detail),
                Vec::new(),
                None,
            ),
            Self::Internal(source) => {
                tracing::error!(error = %source, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Internal server error",
                    None,
                    Vec::new(),
                    None,
                )
            }
        };

        let body = ProblemDetails {
            problem_type: format!(
                "https://openkanban.local/problems/{}",
                code.to_ascii_lowercase().replace('_', "-")
            ),
            title: title.to_owned(),
            status: status.as_u16(),
            code: code.to_owned(),
            detail,
            errors,
        };

        let mut response = Json(body).into_response();
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );

        if status == StatusCode::UNAUTHORIZED {
            headers.insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        if let Some(seconds) = retry_after_seconds {
            headers.insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }

        *response.status_mut() = status;
        response
    }
}

/// Response for panics caught by `CatchPanicLayer`: same contract as other errors.
pub fn panic_response() -> Response {
    AppError::Internal(anyhow::anyhow!("request handler panicked")).into_response()
}
