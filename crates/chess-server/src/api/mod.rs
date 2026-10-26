//! HTTP routes, OpenAPI document and WebSocket endpoint.

mod arena;
mod convert;
mod daemon;
pub mod error;
mod game;
mod health;
mod training;
pub mod types;
mod ws;

use axum::routing::{delete, get, post};
use axum::{Json, Router};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

use crate::AppState;

/// OpenAPI description of the REST API, served at `/api/openapi.json`.
#[derive(OpenApi)]
#[openapi(
    info(title = "chess-bot API", description = "Play, analyse and monitor training."),
    paths(
        health::health, game::position, game::engine_move, game::pgn_import, game::pgn_export,
        training::training, training::engine, training::start, training::stop, training::timer,
        training::network_defaults, training::create_network, training::delete_network,
        arena::arena, arena::save_bot, arena::delete_bot, arena::start_tournament,
        arena::stop_tournament,
    ),
    tags(
        (name = "system", description = "Server status"),
        (name = "game", description = "Positions, legal moves and PGN"),
        (name = "engine", description = "Engine moves and evaluation"),
        (name = "training", description = "Networks: NNUE training progress and control"),
        (name = "arena", description = "Bots and tournaments between them"),
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
        .route("/api/engine", get(training::engine))
        .route("/api/training", get(training::training))
        .route("/api/training/start", post(training::start))
        .route("/api/training/stop", post(training::stop))
        .route("/api/training/timer", post(training::timer))
        .route("/api/networks", post(training::create_network))
        .route("/api/networks/defaults", get(training::network_defaults))
        .route("/api/networks/{name}", delete(training::delete_network))
        .route("/api/arena", get(arena::arena))
        .route("/api/bots", post(arena::save_bot))
        .route("/api/bots/{name}", delete(arena::delete_bot))
        .route("/api/tournaments", post(arena::start_tournament))
        .route("/api/tournaments/stop", post(arena::stop_tournament))
        .route("/ws", get(ws::upgrade))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
