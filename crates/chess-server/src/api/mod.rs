//! HTTP routes, OpenAPI document and WebSocket endpoint.

mod health;
pub mod types;
mod ws;

use axum::routing::get;
use axum::{Json, Router};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

/// OpenAPI description of the REST API, served at `/api/openapi.json`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "chess-bot API",
        description = "Play, analyse and monitor training."
    ),
    paths(health::health),
    components(schemas(types::HealthResponse))
)]
pub struct ApiDoc;

/// Builds the application router.
///
/// Every route lives under `/api` or `/ws` so that a reverse proxy (Nginx in
/// Docker, Vite in development) can expose the UI and the API on one origin.
pub fn router() -> Router {
    Router::new()
        .route("/api/health", get(health::health))
        .route(
            "/api/openapi.json",
            get(|| async { Json(ApiDoc::openapi()) }),
        )
        .route("/ws", get(ws::upgrade))
        .layer(TraceLayer::new_for_http())
}
