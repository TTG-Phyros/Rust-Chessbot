//! Library half of `chess-server`, split from `main.rs` so integration tests
//! can build the router without binding a socket.

pub mod api;
pub mod config;
pub mod engine;

use engine::EngineService;

/// State shared by every request handler.
#[derive(Debug, Clone, Default)]
pub struct AppState {
    /// Runs engine searches.
    pub engine: EngineService,
}

impl AppState {
    /// State configured from `config`.
    #[must_use]
    pub fn new(config: &config::ServerConfig) -> Self {
        Self {
            engine: EngineService::new(config.max_searches, config.engine_hash_mb),
        }
    }
}
