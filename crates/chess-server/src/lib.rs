//! Library half of `chess-server`, split from `main.rs` so integration tests
//! can build the router without binding a socket.

pub mod api;
pub mod config;
pub mod engine;

use std::path::PathBuf;
use std::sync::Arc;

use chess_reviewer::ReviewService;
use engine::EngineService;

/// State shared by every request handler.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Runs engine searches.
    pub engine: EngineService,
    /// Directory holding weights and training metrics.
    pub data_dir: PathBuf,
    /// Base URL of the training control daemon, if any.
    pub trainer_url: Option<String>,
    /// HTTP client used to reach the training daemon.
    pub http: reqwest::Client,
    /// Chess.com reviews, when their cache could be opened.
    pub review: Option<Arc<ReviewService>>,
    /// The NNUE lab's API (`/api/v1`), when its database could be opened.
    pub nnue: Option<chess_nnue_api::NnueApi>,
}

impl AppState {
    /// State configured from `config`.
    #[must_use]
    pub fn new(config: &config::ServerConfig) -> Self {
        Self {
            engine: EngineService::new(
                config.max_searches,
                config.engine_hash_mb,
                Some(config.eval_file.clone()),
            )
            .with_bots(config.data_dir.join("bots"))
            .with_stockfish(config.stockfish.clone()),
            data_dir: config.data_dir.clone(),
            trainer_url: config
                .trainer_url
                .as_deref()
                .map(|url| url.trim_end_matches('/').to_owned())
                .filter(|url| !url.is_empty()),
            http: reqwest::Client::new(),
            review: None,
            nnue: None,
        }
    }

    /// Serves the NNUE lab's API `api`; activating a model there changes
    /// the engine's network.
    #[must_use]
    pub fn with_nnue(mut self, api: chess_nnue_api::NnueApi) -> Self {
        let engine = self.engine.clone();
        self.nnue = Some(api.on_activate(Arc::new(move |file| engine.activate(file))));
        self
    }

    /// Serves the Chess.com reviews of `service`.
    #[must_use]
    pub fn with_review(mut self, service: Arc<ReviewService>) -> Self {
        self.review = Some(service);
        self
    }
}

impl Default for AppState {
    /// Classical evaluation, no training daemon, `data` directory, no reviews.
    fn default() -> Self {
        Self {
            engine: EngineService::default(),
            data_dir: PathBuf::from("data"),
            trainer_url: None,
            http: reqwest::Client::new(),
            review: None,
            nnue: None,
        }
    }
}
