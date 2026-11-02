// Helpers are shared by several test binaries; not every binary uses every helper.
#![allow(dead_code)]

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use open_kanban_service::{app, config::Config, db, state::AppState};
use serde_json::Value;
use sqlx::SqlitePool;
use tempfile::TempDir;
use tower::ServiceExt;

pub struct TestApp {
    pub router: Router,
    pub pool: SqlitePool,
    pub _dir: TempDir,
}

const TEST_JWT_SECRET: &str = "test-only-secret-with-at-least-32-characters";

pub fn test_config() -> Config {
    Config::from_lookup(|key| match key {
        "OK_JWT_SECRET" => Some(TEST_JWT_SECRET.to_owned()),
        "OK_LOGIN_MAX_ATTEMPTS" => Some("5".to_owned()),
        _ => None,
    })
    .expect("valid test config")
}

pub fn production_config() -> Config {
    Config::from_lookup(|key| match key {
        "OK_ENV" => Some("production".to_owned()),
        "OK_JWT_SECRET" => Some(TEST_JWT_SECRET.to_owned()),
        _ => None,
    })
    .expect("valid production test config")
}

pub async fn test_app() -> TestApp {
    test_app_with(test_config()).await
}

pub async fn test_app_with(config: Config) -> TestApp {
    let dir = tempfile::tempdir().expect("temporary directory");
    let pool = db::open(db::path_options(dir.path().join("test.db")))
        .await
        .expect("test database");
    let router = app::router(AppState::new(pool.clone(), config));

    TestApp {
        router,
        pool,
        _dir: dir,
    }
}

pub async fn call(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request");

    call_request(app, request).await
}

pub async fn call_request(
    app: &Router,
    request: Request<Body>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes()
        .to_vec();

    (status, headers, body)
}

pub fn json(body: &[u8]) -> Value {
    serde_json::from_slice(body).unwrap_or(Value::Null)
}

pub async fn request_json(
    app: &TestApp,
    method: Method,
    uri: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }

    let body = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };

    let request = builder.body(body).expect("request");
    let (status, headers, raw) = call_request(&app.router, request).await;
    (status, headers, json(&raw))
}

/// Full `Set-Cookie` header value for the cookie with the given name.
pub fn set_cookie_header(headers: &HeaderMap, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with(&prefix))
        .map(str::to_owned)
}

/// Cookie value (token) for the cookie with the given name.
pub fn set_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    set_cookie_header(headers, name).and_then(|value| {
        value
            .strip_prefix(&format!("{name}="))
            .and_then(|rest| rest.split(';').next())
            .map(str::to_owned)
    })
}
