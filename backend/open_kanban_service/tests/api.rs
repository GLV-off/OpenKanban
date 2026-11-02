use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

mod support;

use support::{call, call_request, json, production_config, test_app, test_app_with};

#[tokio::test]
async fn healthz_reports_ok() {
    let app = test_app().await;

    let (status, headers, body) = call(&app.router, "/healthz").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    assert_eq!(json(&body)["status"], "ok");

    let request_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .expect("request id header");
    assert_eq!(request_id.len(), 36, "request id should be a UUID");
}

#[tokio::test]
async fn readyz_checks_database() {
    let app = test_app().await;

    let (status, _, body) = call(&app.router, "/readyz").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["status"], "ready");
}

#[tokio::test]
async fn readyz_returns_problem_when_database_is_closed() {
    let app = test_app().await;
    app.pool.close().await;

    let (status, headers, body) = call(&app.router, "/readyz").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/problem+json")
    );

    let problem = json(&body);
    assert_eq!(problem["status"], 503);
    assert_eq!(problem["code"], "SERVICE_UNAVAILABLE");
    assert!(problem["type"].is_string());
    assert_eq!(problem["title"], "Service unavailable");
}

#[tokio::test]
async fn unknown_route_returns_problem_json() {
    let app = test_app().await;

    let (status, headers, body) = call(&app.router, "/missing").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(json(&body)["code"], "NOT_FOUND");
}

#[tokio::test]
async fn wrong_method_returns_problem_json() {
    let app = test_app().await;
    let request = Request::builder()
        .method("POST")
        .uri("/healthz")
        .body(Body::empty())
        .expect("request");

    let (status, headers, body) = call_request(&app.router, request).await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(json(&body)["code"], "METHOD_NOT_ALLOWED");
}

#[tokio::test]
async fn swagger_ui_is_hidden_in_production() {
    let app = test_app_with(production_config()).await;

    let (status, _, _) = call(&app.router, "/api/v1/docs").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn openapi_describes_health_and_auth_endpoints() {
    let app = test_app().await;

    let (status, headers, body) = call(&app.router, "/api/v1/openapi.json").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );

    let document = json(&body);
    assert!(
        document["openapi"]
            .as_str()
            .is_some_and(|version| version.starts_with("3.1")),
        "specification must be OpenAPI 3.1, got {:?}",
        document["openapi"]
    );
    assert!(document["paths"]["/healthz"].is_object());
    assert!(document["paths"]["/readyz"].is_object());
    assert!(document["paths"]["/api/v1/auth/login"].is_object());
    assert!(document["paths"]["/api/v1/auth/me"].is_object());
    assert!(document["components"]["securitySchemes"]["bearer_auth"].is_object());
    assert!(document["components"]["securitySchemes"]["refresh_cookie"].is_object());
    assert!(
        document["info"].get("license").is_none(),
        "license must not be published before it is chosen"
    );
}

#[tokio::test]
async fn production_serves_openapi_without_swagger_assets() {
    let app = test_app_with(production_config()).await;

    let request = Request::builder()
        .uri("/api/v1/openapi.json")
        .body(Body::empty())
        .expect("request");
    let response = app.router.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes()
        .to_vec();

    assert_eq!(status, StatusCode::OK);
    assert!(json(&body)["paths"]["/api/v1/auth/register"].is_object());
}
