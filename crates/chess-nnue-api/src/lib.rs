//! REST API of the NNUE lab: architectures, datasets, training and
//! self-play runs, models (`/api/v1`), its OpenAPI 3.1 document
//! (`/api/v1/openapi.json`) and interactive documentation (`/docs`), and
//! the internal API the workers use (`/internal/v1`, never exposed by the
//! reverse proxy).
//!
//! Conventions: errors are RFC 9457 problem details; lists page with an
//! opaque cursor; creations accept an `Idempotency-Key`; every `/api/v1`
//! route but the OpenAPI document needs an API key (`X-API-Key`) unless
//! authentication is disabled, and is rate limited per key. Custom methods
//! follow the `resource:method` form (`/architectures:validate`,
//! `/training-runs/{id}:stop`).

mod auth;
mod docs;
mod idempotency;
mod internal;
mod openapi;
mod pagination;
mod problem;
mod routes;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::middleware::from_fn_with_state;
use chess_jobs::{Lab, LabError};

pub use openapi::openapi;
pub use problem::{FieldError, PROBLEM_JSON, Problem};

use crate::auth::RateLimiter;
use crate::problem::ApiProblem;

/// Who may call `/api/v1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auth {
    /// Requests must carry one of these keys.
    Keys(Vec<String>),
    /// Anyone (development only).
    Disabled,
}

/// Settings of the API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiConfig {
    /// Authentication of `/api/v1`.
    pub auth: Auth,
    /// Requests per minute per API key (0: unlimited).
    pub rate_limit: u32,
    /// Largest dataset upload, in bytes.
    pub max_upload_bytes: u64,
    /// Token of the workers (`Authorization: Bearer`); without one, the
    /// internal API is not served.
    pub worker_token: Option<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            auth: Auth::Keys(Vec::new()),
            rate_limit: 600,
            max_upload_bytes: 4 << 30,
            worker_token: None,
        }
    }
}

/// Called with the network file the engine should play with (`None`: the
/// server's configured network).
pub type ActivateHook = Arc<dyn Fn(Option<PathBuf>) + Send + Sync>;

/// The API: the lab and how it is exposed.
#[derive(Clone)]
pub struct NnueApi {
    pub(crate) lab: Arc<Lab>,
    pub(crate) config: Arc<ApiConfig>,
    pub(crate) limiter: Arc<RateLimiter>,
    pub(crate) on_activate: ActivateHook,
}

impl std::fmt::Debug for NnueApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NnueApi")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Jobs whose worker has been silent this long are marked interrupted.
pub const WORKER_SILENCE: Duration = Duration::from_secs(120);

impl NnueApi {
    /// The API of `lab`.
    #[must_use]
    pub fn new(lab: Arc<Lab>, config: ApiConfig) -> Self {
        if let Auth::Keys(keys) = &config.auth
            && keys.is_empty()
        {
            tracing::warn!(
                "no API key configured (CHESS_API_KEYS): every /api/v1 request will be refused"
            );
        }
        Self {
            lab,
            limiter: Arc::new(RateLimiter::new(config.rate_limit)),
            config: Arc::new(config),
            on_activate: Arc::new(|_| {}),
        }
    }

    /// Tells `hook` which network to play with: now (the active model, if
    /// any) and whenever a model is activated.
    #[must_use]
    pub fn on_activate(mut self, hook: ActivateHook) -> Self {
        self.on_activate = hook;
        if let Err(err) = self.apply_active_model() {
            tracing::warn!(error = %err, "cannot read the active model");
        }
        self
    }

    /// The lab.
    #[must_use]
    pub fn lab(&self) -> &Lab {
        &self.lab
    }

    pub(crate) fn apply_active_model(&self) -> Result<(), LabError> {
        let file = self
            .lab
            .active_model()?
            .map(|id| self.lab.paths().model_file(id));
        (self.on_activate)(file);
        Ok(())
    }

    /// `/api/v1`, `/docs` and, with a worker token, `/internal/v1`.
    pub fn router(&self) -> Router {
        let v1 = routes::router()
            .layer(from_fn_with_state(self.clone(), idempotency::layer))
            .layer(from_fn_with_state(self.clone(), auth::layer))
            .merge(openapi::router())
            .merge(docs::router());
        let router = match &self.config.worker_token {
            Some(_) => v1.merge(internal::router(self.clone())),
            None => v1,
        };
        router.with_state(self.clone())
    }

    /// Housekeeping until the process ends: jobs of silent workers become
    /// `interrupted` (and resumable).
    pub async fn maintain(self) {
        let mut ticks = tokio::time::interval(Duration::from_secs(30));
        loop {
            ticks.tick().await;
            match self.lab.reap_silent(WORKER_SILENCE) {
                Ok(jobs) if !jobs.is_empty() => {
                    tracing::warn!(?jobs, "jobs interrupted: their worker stopped reporting");
                }
                Ok(_) => {}
                Err(err) => tracing::warn!(error = %err, "cannot check for silent workers"),
            }
        }
    }
}

/// Runs blocking lab work (file I/O, hashing) off the async runtime.
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ApiProblem> + Send + 'static,
) -> Result<T, ApiProblem> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(ApiProblem::internal)?
}
