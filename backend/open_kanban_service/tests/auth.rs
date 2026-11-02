use axum::http::{Method, StatusCode, header};
use serde_json::json;

mod support;

use support::{request_json, set_cookie, set_cookie_header, test_app, test_app_with, test_config};

const PASSWORD: &str = "correct horse battery staple";
const ORIGIN: &str = "http://localhost:5173";

async fn register(
    app: &support::TestApp,
    email: &str,
    password: &str,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    request_json(
        app,
        Method::POST,
        "/api/v1/auth/register",
        Some(json!({
            "email": email,
            "password": password,
            "display_name": "Test User"
        })),
        &[("origin", ORIGIN)],
    )
    .await
}

async fn login(
    app: &support::TestApp,
    email: &str,
    password: &str,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    request_json(
        app,
        Method::POST,
        "/api/v1/auth/login",
        Some(json!({ "email": email, "password": password })),
        &[("origin", ORIGIN)],
    )
    .await
}

async fn refresh(
    app: &support::TestApp,
    cookie: &str,
    origin: &str,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let cookie = format!("ok_refresh={cookie}");
    request_json(
        app,
        Method::POST,
        "/api/v1/auth/refresh",
        None,
        &[("cookie", &cookie), ("origin", origin)],
    )
    .await
}

#[tokio::test]
async fn register_creates_account_and_usable_session() {
    let app = test_app().await;

    let (status, headers, body) = register(&app, "  Alice@Example.COM ", PASSWORD).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["user"]["email"], "alice@example.com");
    assert_eq!(body["token_type"], "Bearer");
    assert!(
        body["access_token"]
            .as_str()
            .is_some_and(|token| token.len() > 40)
    );

    let cookie_header = set_cookie_header(&headers, "ok_refresh").expect("refresh cookie");
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Strict"));
    assert!(cookie_header.contains("Path=/api/v1/auth"));

    let authorization = format!(
        "Bearer {}",
        body["access_token"].as_str().expect("access token")
    );
    let (status, _, me) = request_json(
        &app,
        Method::GET,
        "/api/v1/auth/me",
        None,
        &[("authorization", &authorization)],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["id"], body["user"]["id"]);
    assert_eq!(me["email"], "alice@example.com");
    assert!(
        me.get("password_hash").is_none(),
        "password hash must never be exposed"
    );
}

#[tokio::test]
async fn register_rejects_duplicate_email_ignoring_case() {
    let app = test_app().await;

    let (status, _, _) = register(&app, "alice@example.com", PASSWORD).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _, body) = register(&app, "ALICE@EXAMPLE.COM", PASSWORD).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "EMAIL_TAKEN");
}

#[tokio::test]
async fn register_rejects_weak_password() {
    let app = test_app().await;

    let (status, _, body) = register(&app, "weak@example.com", "short").await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "VALIDATION_ERROR");
    assert_eq!(body["errors"][0]["field"], "password");
}

#[tokio::test]
async fn login_accepts_normalized_email_and_rejects_bad_credentials() {
    let app = test_app().await;
    register(&app, "alice@example.com", PASSWORD).await;

    let (status, headers, body) = login(&app, " ALICE@example.com ", PASSWORD).await;

    assert_eq!(status, StatusCode::OK);
    assert!(set_cookie(&headers, "ok_refresh").is_some());
    assert_eq!(body["user"]["email"], "alice@example.com");

    let (status, _, body) = login(&app, "alice@example.com", "wrong password").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "INVALID_CREDENTIALS");

    let (status, _, body) = login(&app, "nobody@example.com", PASSWORD).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn disabled_account_cannot_login() {
    let app = test_app().await;
    register(&app, "disabled@example.com", PASSWORD).await;

    sqlx::query("UPDATE users SET status = 'disabled' WHERE email = ?")
        .bind("disabled@example.com")
        .execute(&app.pool)
        .await
        .expect("disable user");

    let (status, _, body) = login(&app, "disabled@example.com", PASSWORD).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "ACCOUNT_DISABLED");
}

#[tokio::test]
async fn refresh_rotates_tokens_and_revokes_chain_on_reuse() {
    let app = test_app().await;
    register(&app, "alice@example.com", PASSWORD).await;
    let (status, headers, body) = login(&app, "alice@example.com", PASSWORD).await;
    assert_eq!(status, StatusCode::OK);

    let old_cookie = set_cookie(&headers, "ok_refresh").expect("refresh cookie");
    let old_access = body["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();

    let (status, headers, body) = refresh(&app, &old_cookie, ORIGIN).await;
    assert_eq!(status, StatusCode::OK);

    let new_cookie = set_cookie(&headers, "ok_refresh").expect("rotated cookie");
    assert_ne!(new_cookie, old_cookie);

    let new_access = body["access_token"].as_str().expect("new access token");
    assert_ne!(new_access, old_access);

    // Reusing the rotated token must fail and revoke the whole chain.
    let (status, _, body) = refresh(&app, &old_cookie, ORIGIN).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "INVALID_REFRESH_TOKEN");

    let (status, _, _) = refresh(&app, &new_cookie, ORIGIN).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_revokes_refresh_session() {
    let app = test_app().await;
    register(&app, "alice@example.com", PASSWORD).await;
    let (_, headers, _) = login(&app, "alice@example.com", PASSWORD).await;
    let cookie = set_cookie(&headers, "ok_refresh").expect("refresh cookie");
    let cookie_header = format!("ok_refresh={cookie}");

    let (status, headers, _) = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/logout",
        None,
        &[("cookie", &cookie_header), ("origin", ORIGIN)],
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let cleared = set_cookie_header(&headers, "ok_refresh").expect("cleared cookie");
    assert!(cleared.contains("Max-Age=0"));

    let (status, _, _) = refresh(&app, &cookie, ORIGIN).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn refresh_rejects_foreign_origin_without_consuming_session() {
    let app = test_app().await;
    register(&app, "alice@example.com", PASSWORD).await;
    let (_, headers, _) = login(&app, "alice@example.com", PASSWORD).await;
    let cookie = set_cookie(&headers, "ok_refresh").expect("refresh cookie");

    let (status, _, body) = refresh(&app, &cookie, "https://evil.example").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "ORIGIN_NOT_ALLOWED");

    let (status, _, _) = refresh(&app, &cookie, ORIGIN).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn login_is_rate_limited_per_email() {
    let mut config = test_config();
    config.login_max_attempts = 2;
    let app = test_app_with(config).await;

    for _ in 0..2 {
        let (status, _, _) = login(&app, "nobody@example.com", "wrong password").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    let (status, headers, body) = login(&app, "nobody@example.com", "wrong password").await;

    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    assert!(headers.get(header::RETRY_AFTER).is_some());
}

#[tokio::test]
async fn me_requires_a_valid_bearer_token() {
    let app = test_app().await;

    let (status, headers, body) =
        request_json(&app, Method::GET, "/api/v1/auth/me", None, &[]).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "MISSING_TOKEN");
    assert_eq!(
        headers
            .get(header::WWW_AUTHENTICATE)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer")
    );

    let (status, _, body) = request_json(
        &app,
        Method::GET,
        "/api/v1/auth/me",
        None,
        &[("authorization", "Bearer not-a-token")],
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "INVALID_TOKEN");
}

#[tokio::test]
async fn refresh_cookie_is_secure_when_configured() {
    let mut config = test_config();
    config.cookie_secure = true;
    let app = test_app_with(config).await;

    let (status, headers, _) = register(&app, "secure@example.com", PASSWORD).await;

    assert_eq!(status, StatusCode::CREATED);
    let cookie_header = set_cookie_header(&headers, "ok_refresh").expect("refresh cookie");
    assert!(cookie_header.contains("Secure"));
}

#[tokio::test]
async fn malformed_json_returns_problem_details() {
    let app = test_app().await;
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from("{not json"))
        .expect("request");

    let (status, headers, raw) = support::call_request(&app.router, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(support::json(&raw)["code"], "INVALID_BODY");
}
