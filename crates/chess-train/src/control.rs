//! Control daemon (`chess-train serve`): a small internal HTTP API through
//! which `chess-server` starts and stops the training pipeline.
//!
//! - `GET /status` → `{"state": "idle"|"running"|"stopping", "run": ..., "error": ...}`
//! - `POST /start` (optional body `{"fresh": bool, "superbatches": n}`) → 202, or 409 if running
//! - `POST /stop` → 202 (the pipeline finishes its step and checkpoints)
//!
//! Progress itself is read by the server from the metrics files.

use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chess_search::StopFlag;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::device::DevicePreference;

/// State of the pipeline as seen by the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RunState {
    Idle,
    Running,
    Stopping,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ControlStatus {
    pub(crate) state: RunState,
    pub(crate) run: String,
    /// Why the last run ended in error, if it did.
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct StartRequest {
    /// Ignore the checkpoint and start a new network.
    pub(crate) fresh: bool,
    /// Stop after this many superbatches (0 or absent: until stopped).
    pub(crate) superbatches: Option<u64>,
}

struct Running {
    stop: StopFlag,
    thread: JoinHandle<()>,
}

/// Owns at most one pipeline run.
pub(crate) struct Controller {
    config: Config,
    device: DevicePreference,
    running: Option<Running>,
    error: Arc<Mutex<Option<String>>>,
}

type Shared = Arc<Mutex<Controller>>;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Controller {
    pub(crate) fn new(config: Config, device: DevicePreference) -> Self {
        Self {
            config,
            device,
            running: None,
            error: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn status(&mut self) -> ControlStatus {
        // Forget a run that ended on its own (superbatch limit, error).
        if self
            .running
            .as_ref()
            .is_some_and(|run| run.thread.is_finished())
            && let Some(run) = self.running.take()
        {
            let _ = run.thread.join();
        }
        let state = match &self.running {
            None => RunState::Idle,
            Some(run) if run.stop.is_stopped() => RunState::Stopping,
            Some(_) => RunState::Running,
        };
        ControlStatus {
            state,
            run: self.config.run.clone(),
            error: lock(&self.error).clone(),
        }
    }

    /// Starts the pipeline; `false` if one is already running.
    pub(crate) fn start(&mut self, request: &StartRequest) -> bool {
        if self.status().state != RunState::Idle {
            return false;
        }
        let mut config = self.config.clone();
        if let Some(superbatches) = request.superbatches {
            config.train.superbatches = superbatches;
        }
        let (device, resume, stop) = (self.device, !request.fresh, StopFlag::new());
        let error = Arc::clone(&self.error);
        *lock(&error) = None;
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            if let Err(err) = crate::run_pipeline_with(&config, device, resume, &flag) {
                tracing::error!(error = %format!("{err:#}"), "training run failed");
                *lock(&error) = Some(format!("{err:#}"));
            }
        });
        tracing::info!(run = %self.config.run, fresh = request.fresh, "training started from the control API");
        self.running = Some(Running { stop, thread });
        true
    }

    /// Asks the running pipeline to stop (it checkpoints first).
    pub(crate) fn stop(&mut self) {
        if let Some(run) = &self.running {
            run.stop.stop();
        }
    }

    /// Stops and waits for the pipeline (daemon shutdown).
    fn shutdown(&mut self) {
        self.stop();
        if let Some(run) = self.running.take() {
            let _ = run.thread.join();
        }
    }
}

pub(crate) fn router(controller: Shared) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/start", post(start))
        .route("/stop", post(stop))
        .with_state(controller)
}

async fn status(State(controller): State<Shared>) -> Json<ControlStatus> {
    Json(lock(&controller).status())
}

async fn start(
    State(controller): State<Shared>,
    body: Option<Json<StartRequest>>,
) -> (StatusCode, Json<ControlStatus>) {
    let request = body.map(|Json(request)| request).unwrap_or_default();
    let mut controller = lock(&controller);
    let code = if controller.start(&request) {
        StatusCode::ACCEPTED
    } else {
        StatusCode::CONFLICT
    };
    (code, Json(controller.status()))
}

async fn stop(State(controller): State<Shared>) -> (StatusCode, Json<ControlStatus>) {
    let mut controller = lock(&controller);
    controller.stop();
    (StatusCode::ACCEPTED, Json(controller.status()))
}

/// Serves the control API on `port` until Ctrl+C / SIGTERM, then stops the
/// pipeline cleanly.
pub(crate) fn serve(
    config: Config,
    device: DevicePreference,
    port: u16,
    autostart: bool,
) -> Result<()> {
    let controller = Arc::new(Mutex::new(Controller::new(config, device)));
    if autostart {
        lock(&controller).start(&StartRequest::default());
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .context("cannot start the async runtime")?;
    let app = router(Arc::clone(&controller));
    runtime.block_on(async {
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("cannot listen on {addr}"))?;
        tracing::info!(%addr, "control API listening");
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_signal().await;
            })
            .await
            .context("control API error")
    })?;
    tracing::info!("shutting down: stopping the pipeline");
    lock(&controller).shutdown();
    Ok(())
}

async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;
    use crate::test_dir;

    async fn call(app: &Router, method: &str, uri: &str) -> (StatusCode, ControlStatus) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let code = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (code, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn start_stop_cycle() {
        let dir = test_dir("control");
        let config = Config {
            run: "ctl".into(),
            data_dir: dir.join("data"),
            dataset_dir: dir.join("datasets"),
            // Never enough data: the run waits until it is stopped.
            min_positions: u64::MAX,
            ..Config::default()
        };
        let mut config = config;
        config.selfplay.threads = 1;
        config.selfplay.nodes = 50;
        config.evaluation.enabled = false;
        let controller = Arc::new(Mutex::new(Controller::new(config, DevicePreference::Cpu)));
        let app = router(Arc::clone(&controller));

        let (code, status) = call(&app, "GET", "/status").await;
        assert_eq!((code, status.state), (StatusCode::OK, RunState::Idle));
        let (code, status) = call(&app, "POST", "/start").await;
        assert_eq!(
            (code, status.state),
            (StatusCode::ACCEPTED, RunState::Running)
        );
        let (code, _) = call(&app, "POST", "/start").await;
        assert_eq!(code, StatusCode::CONFLICT);
        let (code, status) = call(&app, "POST", "/stop").await;
        assert_eq!(code, StatusCode::ACCEPTED);
        assert_eq!(status.state, RunState::Stopping);

        // The run ends after its current step.
        let mut idle = false;
        for _ in 0..200 {
            if call(&app, "GET", "/status").await.1.state == RunState::Idle {
                idle = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert!(idle, "pipeline did not stop");
        assert_eq!(call(&app, "GET", "/status").await.1.error, None);
    }
}
