use axum::{Json, Router, routing::get};
use tower_http::{
    catch_panic::CatchPanicLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{auth, error, health, state::AppState};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Open Kanban API",
        version = "0.1.0",
        description = "HTTP contract of the Open Kanban service. All endpoints live under /api/v1."
    ),
    paths(
        crate::health::healthz,
        crate::health::readyz,
        crate::auth::routes::register,
        crate::auth::routes::login,
        crate::auth::routes::refresh,
        crate::auth::routes::logout,
        crate::auth::routes::me
    ),
    components(
        schemas(
            crate::health::HealthResponse,
            crate::error::ProblemDetails,
            crate::error::FieldError,
            crate::users::PublicUser,
            crate::auth::routes::RegisterRequest,
            crate::auth::routes::LoginRequest,
            crate::auth::routes::AuthResponse
        )
    ),
    tags(
        (name = "system", description = "Health and readiness probes"),
        (name = "auth", description = "Registration, login and session management")
    )
)]
pub struct ApiDoc;

pub fn router(state: AppState) -> Router {
    let environment = state.environment();

    let router = Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .nest("/api/v1", api_routes())
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(CatchPanicLayer::custom(|_| error::panic_response()))
        .with_state(state);

    if environment.is_production() {
        router.route("/api/v1/openapi.json", get(openapi_json))
    } else {
        router.merge(SwaggerUi::new("/api/v1/docs").url("/api/v1/openapi.json", openapi_document()))
    }
}

fn api_routes() -> Router<AppState> {
    Router::new().nest("/auth", auth::routes::router())
}

fn openapi_document() -> utoipa::openapi::OpenApi {
    use utoipa::openapi::security::{
        ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme,
    };

    let mut document = ApiDoc::openapi();
    // The license is not chosen yet (Q11); do not publish an empty license object.
    document.info.license = None;

    let components = document.components.get_or_insert_with(Default::default);
    components.add_security_scheme(
        "bearer_auth",
        SecurityScheme::Http(
            HttpBuilder::new()
                .scheme(HttpAuthScheme::Bearer)
                .bearer_format("JWT")
                .build(),
        ),
    );
    components.add_security_scheme(
        "refresh_cookie",
        SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("ok_refresh"))),
    );

    document
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi_document())
}

async fn not_found() -> error::AppError {
    error::AppError::NotFound
}

async fn method_not_allowed() -> error::AppError {
    error::AppError::MethodNotAllowed
}
