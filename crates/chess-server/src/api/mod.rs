//! HTTP routes, OpenAPI document and WebSocket endpoint.

mod convert;
pub mod error;
mod game;
mod health;
pub mod types;
mod ws;

use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

use crate::AppState;

/// OpenAPI description of the REST API, served at `/api/openapi.json`.
#[derive(OpenApi)]
#[openapi(
    info(title = "chess-bot API", description = "Play, analyse and monitor training."),
    paths(health::health, game::position, game::engine_move, game::pgn_import, game::pgn_export),
    tags(
        (name = "system", description = "Server status"),
        (name = "game", description = "Positions, legal moves and PGN"),
        (name = "engine", description = "Engine moves"),
    )
)]
pub struct ApiDoc;

/// Builds the application router.
///
/// Every route lives under `/api` or `/ws` so that a reverse proxy (Nginx in
/// Docker, Vite in development) can expose the UI and the API on one origin.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health::health))
        .route(
            "/api/openapi.json",
            get(|| async { Json(ApiDoc::openapi()) }),
        )
        .route("/api/position", post(game::position))
        .route("/api/engine/move", post(game::engine_move))
        .route("/api/pgn/import", post(game::pgn_import))
        .route("/api/pgn/export", post(game::pgn_export))
        .route("/ws", get(ws::upgrade))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
