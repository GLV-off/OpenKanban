use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use cookie::{Cookie, SameSite, time::Duration as CookieDuration};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    auth::{extract::AuthenticatedUser, service},
    error::{AppError, ProblemDetails},
    state::AppState,
    users::PublicUser,
};

pub const REFRESH_COOKIE: &str = "ok_refresh";
const COOKIE_PATH: &str = "/api/v1/auth";

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: PublicUser,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    summary = "Register a new account and start a session",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "Account created and session started", body = AuthResponse),
        (status = 409, description = "Email is already registered", body = ProblemDetails),
        (status = 422, description = "Validation failed", body = ProblemDetails)
    )
)]
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    ensure_allowed_origin(&state, &headers)?;
    let request = parse_json(payload)?;

    let session = service::register(
        &state,
        &request.email,
        &request.password,
        &request.display_name,
        user_agent(&headers),
    )
    .await?;

    auth_response(&state, StatusCode::CREATED, session)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    summary = "Authenticate with email and password",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Session started", body = AuthResponse),
        (status = 401, description = "Invalid credentials", body = ProblemDetails),
        (status = 429, description = "Too many login attempts", body = ProblemDetails)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    ensure_allowed_origin(&state, &headers)?;
    let request = parse_json(payload)?;

    let session = service::login(
        &state,
        &request.email,
        &request.password,
        user_agent(&headers),
    )
    .await?;

    auth_response(&state, StatusCode::OK, session)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "auth",
    summary = "Rotate the refresh token and issue a new access token",
    security(("refresh_cookie" = [])),
    responses(
        (status = 200, description = "Session rotated", body = AuthResponse),
        (status = 401, description = "Refresh token is missing, invalid or reused", body = ProblemDetails),
        (status = 403, description = "Origin is not allowed", body = ProblemDetails)
    )
)]
pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    ensure_allowed_origin(&state, &headers)?;

    let refresh_token = refresh_token_from(&headers).ok_or_else(|| {
        AppError::unauthorized("MISSING_REFRESH_TOKEN", "refresh cookie is missing")
    })?;

    let session = service::refresh(&state, &refresh_token, user_agent(&headers)).await?;
    auth_response(&state, StatusCode::OK, session)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "auth",
    summary = "Revoke the current refresh session",
    security(("refresh_cookie" = [])),
    responses((status = 204, description = "Session revoked"))
)]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    ensure_allowed_origin(&state, &headers)?;

    let refresh_token = refresh_token_from(&headers);
    service::logout(&state, refresh_token.as_deref()).await?;

    let mut response = StatusCode::NO_CONTENT.into_response();
    append_cookie(&mut response, clear_refresh_cookie(&state))?;
    Ok(response)
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    summary = "Return the current user",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current user", body = PublicUser),
        (status = 401, description = "Access token is missing or invalid", body = ProblemDetails)
    )
)]
pub async fn me(AuthenticatedUser(user): AuthenticatedUser) -> Json<PublicUser> {
    Json(user)
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AppError> {
    match payload {
        Ok(Json(value)) => Ok(value),
        Err(rejection) => Err(AppError::bad_request("INVALID_BODY", rejection.body_text())),
    }
}

fn user_agent(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
}

fn ensure_allowed_origin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return Ok(());
    };

    let origin = origin
        .to_str()
        .map_err(|_| AppError::forbidden("ORIGIN_NOT_ALLOWED", "request origin is not allowed"))?;

    if state
        .config
        .allowed_origins
        .iter()
        .any(|allowed| allowed == origin)
    {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "ORIGIN_NOT_ALLOWED",
            "request origin is not allowed",
        ))
    }
}

fn refresh_token_from(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;

    cookies.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == REFRESH_COOKIE).then(|| value.to_owned())
    })
}

fn auth_response(
    state: &AppState,
    status: StatusCode,
    session: service::IssuedSession,
) -> Result<Response, AppError> {
    let cookie = refresh_cookie(state, session.refresh_token);
    let body = AuthResponse {
        access_token: session.access_token,
        token_type: session.token_type.to_owned(),
        expires_in: session.expires_in,
        user: session.user,
    };

    let mut response = (status, Json(body)).into_response();
    append_cookie(&mut response, cookie)?;
    Ok(response)
}

fn refresh_cookie(state: &AppState, token: String) -> Cookie<'static> {
    let mut cookie = Cookie::build((REFRESH_COOKIE, token))
        .path(COOKIE_PATH)
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(CookieDuration::seconds(
            state.config.refresh_token_ttl_seconds as i64,
        ))
        .build();
    cookie.set_secure(state.config.cookie_secure);
    cookie
}

fn clear_refresh_cookie(state: &AppState) -> Cookie<'static> {
    let mut cookie = Cookie::build((REFRESH_COOKIE, ""))
        .path(COOKIE_PATH)
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(CookieDuration::ZERO)
        .build();
    cookie.set_secure(state.config.cookie_secure);
    cookie
}

fn append_cookie(response: &mut Response, cookie: Cookie<'static>) -> Result<(), AppError> {
    let value = HeaderValue::from_str(&cookie.to_string()).map_err(|error| {
        AppError::Internal(anyhow::anyhow!("cannot build Set-Cookie header: {error}"))
    })?;
    response.headers_mut().append(header::SET_COOKIE, value);
    Ok(())
}
