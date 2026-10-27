//! Control daemon (`chess-train serve`): the internal HTTP API through
//! which `chess-server` manages networks, bots and tournaments. The daemon
//! writes every file of the data directory; the server only reads them.
//!
//! - `GET /status` → training and tournament state
//! - `POST /start` (optional body `{"network", "fresh", "superbatches", "maxMinutes"}`)
//!   → 202, or 409 if a training is running
//! - `POST /stop` → 202 (the pipeline finishes its step and checkpoints)
//! - `POST /timer` (`{"minutes": n | null}`) → 202: stop the running training
//!   `n` minutes from now, or never; 409 if no training is running
//! - `GET /networks/defaults` → the hyperparameters of a new network
//! - `POST /networks` (`{"name", "params"}`) → 201; `DELETE /networks/{name}` → 204
//! - `POST /bots` (`{"name", "network", "superbatch"}`) → 201; `DELETE /bots/{name}` → 204
//! - `POST /tournaments` (`{"bots", "gamesPerPair", "limit", "threads", "pairings"}`) → 202
//! - `POST /tournaments/stop` → 202
//!
//! Errors are `{"error": "..."}` with 404, 409 or 422. Progress itself is
//! read by the server from the files.

use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chess_search::StopFlag;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::bots::{self, BotInfo, SaveBotRequest};
use crate::config::Config;
use crate::device::DevicePreference;
use crate::network::{self, NetworkParams};
use crate::ranking;
use crate::stockfish;
use crate::timer::Timer;
use crate::tournament::{self, TournamentRequest};

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
    /// Network being trained, or the last one (the configured one at first).
    pub(crate) run: String,
    /// Why the last run ended in error, if it did.
    pub(crate) error: Option<String>,
    /// Id of the tournament being played.
    pub(crate) tournament: Option<String>,
    /// Stockfish found for the `stockfish-N` bots (`id name`), if any.
    pub(crate) stockfish: Option<String>,
    /// When the running training stops on its own (Unix seconds), if limited.
    pub(crate) deadline: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct StartRequest {
    /// Network to train (default: the last one).
    pub(crate) network: Option<String>,
    /// Ignore the checkpoint and start a new network.
    pub(crate) fresh: bool,
    /// Stop after this many superbatches (absent: the network's setting).
    pub(crate) superbatches: Option<u64>,
    /// Stop after this many minutes (absent: no time limit).
    pub(crate) max_minutes: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct TimerRequest {
    /// Minutes from now; absent or `null`: no time limit.
    pub(crate) minutes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct CreateNetworkRequest {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) params: Option<NetworkParams>,
}

/// An HTTP error with a JSON body `{"error": "..."}`.
#[derive(Debug)]
pub(crate) struct ControlError {
    status: StatusCode,
    message: String,
}

impl ControlError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, message)
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }
}

impl From<anyhow::Error> for ControlError {
    fn from(err: anyhow::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}"))
    }
}

impl IntoResponse for ControlError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!(error = %self.message, "control request failed");
        }
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

/// A training run or a tournament, on its own thread.
struct Job {
    /// Network trained, or tournament id.
    name: String,
    stop: StopFlag,
    thread: JoinHandle<()>,
}

/// Forgets a job that ended on its own.
fn reap(slot: &mut Option<Job>) {
    if slot.as_ref().is_some_and(|job| job.thread.is_finished())
        && let Some(job) = slot.take()
    {
        let _ = job.thread.join();
    }
}

/// Owns at most one training run and one tournament.
pub(crate) struct Controller {
    config: Config,
    device: DevicePreference,
    /// Network trained by the running job, or the last one trained.
    run: String,
    training: Option<Job>,
    error: Arc<Mutex<Option<String>>>,
    /// Time limit of the running training.
    timer: Timer,
    tournament: Option<Job>,
    /// Version of the configured Stockfish, probed at startup.
    stockfish: Option<String>,
}

type Shared = Arc<Mutex<Controller>>;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn valid_name(name: &str) -> Result<(), ControlError> {
    network::validate_name(name).map_err(ControlError::invalid)
}

impl Controller {
    pub(crate) fn new(config: Config, device: DevicePreference) -> Self {
        let stockfish = stockfish::probe(&config.arena.stockfish);
        match &stockfish {
            Some(version) => {
                tracing::info!(%version, "Stockfish found: stockfish-1 to 10 can play")
            }
            None => tracing::warn!(
                binary = %config.arena.stockfish.display(),
                "Stockfish not found: no Stockfish bots (set [arena] stockfish)"
            ),
        }
        Self {
            stockfish,
            run: config.run.clone(),
            config,
            device,
            training: None,
            error: Arc::new(Mutex::new(None)),
            timer: Timer::default(),
            tournament: None,
        }
    }

    pub(crate) fn status(&mut self) -> ControlStatus {
        reap(&mut self.training);
        reap(&mut self.tournament);
        let state = match &self.training {
            None => RunState::Idle,
            Some(job) if job.stop.is_stopped() => RunState::Stopping,
            Some(_) => RunState::Running,
        };
        ControlStatus {
            state,
            run: self.run.clone(),
            error: lock(&self.error).clone(),
            tournament: self.tournament.as_ref().map(|job| job.name.clone()),
            stockfish: self.stockfish.clone(),
            deadline: self
                .training
                .is_some()
                .then(|| self.timer.deadline())
                .flatten(),
        }
    }

    /// Whether network `name` is training (or finishing its step).
    fn is_training(&mut self, name: &str) -> bool {
        self.status().state != RunState::Idle && self.run == name
    }

    /// Starts the pipeline on a network (created with the daemon's settings
    /// if it does not exist yet).
    pub(crate) fn start(&mut self, request: &StartRequest) -> Result<(), ControlError> {
        if self.status().state != RunState::Idle {
            return Err(ControlError::conflict(
                "a training run is already in progress",
            ));
        }
        let name = request.network.clone().unwrap_or_else(|| self.run.clone());
        valid_name(&name)?;
        if let Some(minutes) = request.max_minutes {
            Timer::validate(minutes).map_err(ControlError::invalid)?;
        }
        let mut config = network::config_for(&self.config, &name)?;
        if let Some(superbatches) = request.superbatches {
            config.train.superbatches = superbatches;
        }
        let (device, resume, stop) = (self.device, !request.fresh, StopFlag::new());
        let error = Arc::clone(&self.error);
        *lock(&error) = None;
        let flag = stop.clone();
        // A fresh timer per run: the watcher of the previous one has ended.
        self.timer = Timer::default();
        self.timer.set(request.max_minutes);
        let watcher = self.timer.watch(stop.clone());
        let thread = std::thread::spawn(move || {
            if let Err(err) = crate::run_pipeline_with(&config, device, resume, &flag) {
                tracing::error!(error = %format!("{err:#}"), "training run failed");
                *lock(&error) = Some(format!("{err:#}"));
            }
            // Ended on its own (superbatch limit, error): release the watcher.
            flag.stop();
            let _ = watcher.join();
        });
        tracing::info!(network = %name, fresh = request.fresh, "training started from the control API");
        self.run.clone_from(&name);
        self.training = Some(Job { name, stop, thread });
        Ok(())
    }

    /// Sets, moves or removes the time limit of the running training.
    fn set_timer(&mut self, request: &TimerRequest) -> Result<(), ControlError> {
        if let Some(minutes) = request.minutes {
            Timer::validate(minutes).map_err(ControlError::invalid)?;
        }
        if self.status().state != RunState::Running {
            return Err(ControlError::conflict("no training in progress"));
        }
        self.timer.set(request.minutes);
        Ok(())
    }

    /// Asks the running pipeline to stop (it checkpoints first).
    pub(crate) fn stop(&mut self) {
        if let Some(job) = &self.training {
            job.stop.stop();
        }
    }

    fn create_network(&mut self, request: CreateNetworkRequest) -> Result<(), ControlError> {
        valid_name(&request.name)?;
        if network::list(&self.config).contains(&request.name) {
            return Err(ControlError::conflict(format!(
                "a network named {} already exists",
                request.name
            )));
        }
        let params = request
            .params
            .unwrap_or_else(|| NetworkParams::of(&self.config));
        params.validate().map_err(ControlError::invalid)?;
        if params.datasets_from.contains(&request.name) {
            return Err(ControlError::invalid(
                "a network always trains on its own games",
            ));
        }
        network::save_params(&self.config, &request.name, &params)?;
        tracing::info!(network = %request.name, ?params, "network created");
        Ok(())
    }

    fn delete_network(&mut self, name: &str) -> Result<(), ControlError> {
        valid_name(name)?;
        if !network::list(&self.config).iter().any(|n| n == name) {
            return Err(ControlError::not_found(format!("no network named {name}")));
        }
        if self.is_training(name) {
            return Err(ControlError::conflict(format!(
                "{name} is training: stop it first"
            )));
        }
        network::delete(&self.config, name)?;
        Ok(())
    }

    fn save_bot(&mut self, request: &SaveBotRequest) -> Result<BotInfo, ControlError> {
        valid_name(&request.name)?;
        valid_name(&request.network)?;
        if bots::exists(&self.config, &request.name) {
            return Err(ControlError::conflict(format!(
                "a bot named {} already exists",
                request.name
            )));
        }
        if bots::source(&self.config, request).is_none() {
            return Err(ControlError::not_found(match request.superbatch {
                Some(superbatch) => format!(
                    "{} has no snapshot of superbatch {superbatch} (only the best are kept)",
                    request.network
                ),
                None => format!("{} has not exported a network yet", request.network),
            }));
        }
        Ok(bots::save(&self.config, request)?)
    }

    fn delete_bot(&mut self, name: &str) -> Result<(), ControlError> {
        valid_name(name)?;
        if !bots::exists(&self.config, name) {
            return Err(ControlError::not_found(format!("no bot named {name}")));
        }
        if self.status().tournament.is_some() {
            return Err(ControlError::conflict(
                "a tournament is in progress: delete bots once it is over",
            ));
        }
        bots::delete(&self.config, name)?;
        Ok(())
    }

    fn start_tournament(&mut self, request: TournamentRequest) -> Result<String, ControlError> {
        if self.status().tournament.is_some() {
            return Err(ControlError::conflict(
                "a tournament is already in progress",
            ));
        }
        request.validate().map_err(ControlError::invalid)?;
        if self.stockfish.is_none()
            && request
                .bots
                .iter()
                .any(|bot| stockfish::level_of(bot).is_some())
        {
            return Err(ControlError::not_found(format!(
                "Stockfish is not installed ({} not found): set [arena] stockfish",
                self.config.arena.stockfish.display()
            )));
        }
        let engines = request
            .bots
            .iter()
            .map(|bot| {
                bots::contestant(&self.config, bot)
                    .map_err(|err| ControlError::not_found(format!("bot {bot}: {err:#}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let id = tournament::new_id(&self.config);
        let (config, stop) = (self.config.clone(), StopFlag::new());
        let (flag, name) = (stop.clone(), id.clone());
        let thread = std::thread::spawn(move || {
            tournament::play(&config, name, &request, &engines, &flag);
        });
        self.tournament = Some(Job {
            name: id.clone(),
            stop,
            thread,
        });
        Ok(id)
    }

    fn stop_tournament(&mut self) {
        if let Some(job) = &self.tournament {
            job.stop.stop();
        }
    }

    /// Stops and waits for every job (daemon shutdown).
    fn shutdown(&mut self) {
        self.stop();
        self.stop_tournament();
        for job in [self.training.take(), self.tournament.take()]
            .into_iter()
            .flatten()
        {
            let _ = job.thread.join();
        }
    }
}

pub(crate) fn router(controller: Shared) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/start", post(start))
        .route("/stop", post(stop))
        .route("/timer", post(set_timer))
        .route("/networks", post(create_network))
        .route("/networks/defaults", get(network_defaults))
        .route("/networks/{name}", delete(delete_network))
        .route("/bots", post(save_bot))
        .route("/bots/{name}", delete(delete_bot))
        .route("/tournaments", post(start_tournament))
        .route("/tournaments/stop", post(stop_tournament))
        .with_state(controller)
}

async fn status(State(controller): State<Shared>) -> Json<ControlStatus> {
    Json(lock(&controller).status())
}

async fn start(
    State(controller): State<Shared>,
    body: Option<Json<StartRequest>>,
) -> Result<(StatusCode, Json<ControlStatus>), ControlError> {
    let request = body.map(|Json(request)| request).unwrap_or_default();
    let mut controller = lock(&controller);
    controller.start(&request)?;
    Ok((StatusCode::ACCEPTED, Json(controller.status())))
}

async fn stop(State(controller): State<Shared>) -> (StatusCode, Json<ControlStatus>) {
    let mut controller = lock(&controller);
    controller.stop();
    (StatusCode::ACCEPTED, Json(controller.status()))
}

async fn set_timer(
    State(controller): State<Shared>,
    Json(request): Json<TimerRequest>,
) -> Result<(StatusCode, Json<ControlStatus>), ControlError> {
    let mut controller = lock(&controller);
    controller.set_timer(&request)?;
    Ok((StatusCode::ACCEPTED, Json(controller.status())))
}

async fn network_defaults(State(controller): State<Shared>) -> Json<NetworkParams> {
    Json(NetworkParams::of(&lock(&controller).config))
}

async fn create_network(
    State(controller): State<Shared>,
    Json(request): Json<CreateNetworkRequest>,
) -> Result<StatusCode, ControlError> {
    lock(&controller).create_network(request)?;
    Ok(StatusCode::CREATED)
}

async fn delete_network(
    State(controller): State<Shared>,
    Path(name): Path<String>,
) -> Result<StatusCode, ControlError> {
    lock(&controller).delete_network(&name)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn save_bot(
    State(controller): State<Shared>,
    Json(request): Json<SaveBotRequest>,
) -> Result<(StatusCode, Json<BotInfo>), ControlError> {
    let info = lock(&controller).save_bot(&request)?;
    Ok((StatusCode::CREATED, Json(info)))
}

async fn delete_bot(
    State(controller): State<Shared>,
    Path(name): Path<String>,
) -> Result<StatusCode, ControlError> {
    lock(&controller).delete_bot(&name)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn start_tournament(
    State(controller): State<Shared>,
    Json(request): Json<TournamentRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ControlError> {
    let id = lock(&controller).start_tournament(request)?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "id": id }))))
}

async fn stop_tournament(State(controller): State<Shared>) -> StatusCode {
    lock(&controller).stop_tournament();
    StatusCode::ACCEPTED
}

/// Serves the control API on `port` until Ctrl+C / SIGTERM, then stops the
/// pipeline cleanly.
pub(crate) fn serve(
    config: Config,
    device: DevicePreference,
    port: u16,
    autostart: bool,
) -> Result<()> {
    // Networks trained before rankings existed keep their best snapshots.
    for name in network::list(&config) {
        if let Err(err) = ranking::seed_from_events(
            &config.weights_dir().join(&name),
            &config.data_dir.join("metrics").join("events.jsonl"),
            &name,
            config.evaluation.keep_best,
        ) {
            tracing::warn!(network = %name, error = %format!("{err:#}"), "cannot rebuild the snapshot ranking");
        }
    }
    let controller = Arc::new(Mutex::new(Controller::new(config, device)));
    if autostart && let Err(err) = lock(&controller).start(&StartRequest::default()) {
        tracing::error!(error = %err.message, "cannot start the training");
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
    tracing::info!("shutting down: stopping the training and the tournament");
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
    use axum::http::{Request, header};
    use chess_eval::nnue::NnueWeights;
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;
    use crate::ranking::snapshot_name;
    use crate::test_dir;

    async fn call(app: &Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(uri);
        if !body.is_null() {
            request = request.header(header::CONTENT_TYPE, "application/json");
        }
        let body = if body.is_null() {
            Body::empty()
        } else {
            Body::from(body.to_string())
        };
        let request = request.body(body).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let code = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (code, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    async fn status(app: &Router) -> ControlStatus {
        serde_json::from_value(call(app, "GET", "/status", Value::Null).await.1).unwrap()
    }

    /// Waits until `done` holds for the daemon status.
    async fn wait_for(app: &Router, done: impl Fn(&ControlStatus) -> bool) -> ControlStatus {
        for _ in 0..400 {
            let status = status(app).await;
            if done(&status) {
                return status;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("the daemon did not get there in time");
    }

    fn test_config(name: &str) -> Config {
        let dir = test_dir(name);
        let mut config = Config {
            run: "ctl".into(),
            data_dir: dir.join("data"),
            dataset_dir: dir.join("datasets"),
            // Never enough data: the run waits until it is stopped.
            min_positions: u64::MAX,
            ..Config::default()
        };
        config.selfplay.threads = 1;
        config.selfplay.nodes = 100;
        config.evaluation.enabled = false;
        config.arena.max_plies = 60;
        config
    }

    #[tokio::test]
    async fn start_stop_cycle() {
        let config = test_config("control");
        let controller = Arc::new(Mutex::new(Controller::new(config, DevicePreference::Cpu)));
        let app = router(Arc::clone(&controller));

        assert_eq!(status(&app).await.state, RunState::Idle);
        let (code, body) = call(&app, "POST", "/start", Value::Null).await;
        assert_eq!(code, StatusCode::ACCEPTED);
        assert_eq!(body["state"], "running");
        let (code, body) = call(&app, "POST", "/start", Value::Null).await;
        assert_eq!(code, StatusCode::CONFLICT);
        assert!(body["error"].as_str().unwrap().contains("in progress"));
        let (code, body) = call(&app, "POST", "/stop", Value::Null).await;
        assert_eq!(code, StatusCode::ACCEPTED);
        assert_eq!(body["state"], "stopping");

        // The run ends after its current step.
        let status = wait_for(&app, |s| s.state == RunState::Idle).await;
        assert_eq!((status.error, status.run.as_str()), (None, "ctl"));
        assert_eq!(status.deadline, None);
        let (code, _) = call(&app, "POST", "/timer", json!({ "minutes": 5 })).await;
        assert_eq!(code, StatusCode::CONFLICT, "no training to time");

        // A time limit, moved, then reached.
        let (code, body) = call(&app, "POST", "/start", json!({ "maxMinutes": 90 })).await;
        assert_eq!(code, StatusCode::ACCEPTED);
        let deadline = body["deadline"].as_u64().unwrap();
        assert!(deadline >= crate::metrics::now() + 90 * 60 - 5);
        let (code, _) = call(&app, "POST", "/timer", json!({ "minutes": 0 })).await;
        assert_eq!(code, StatusCode::UNPROCESSABLE_ENTITY);
        let (code, body) = call(&app, "POST", "/timer", json!({ "minutes": null })).await;
        assert_eq!(
            (code, &body["deadline"]),
            (StatusCode::ACCEPTED, &Value::Null)
        );
        lock(&controller).timer.set(Some(0));
        let status = wait_for(&app, |s| s.state == RunState::Idle).await;
        assert_eq!(status.error, None, "stopped by its time limit");
        let (code, _) = call(&app, "POST", "/start", json!({ "maxMinutes": 0 })).await;
        assert_eq!(code, StatusCode::UNPROCESSABLE_ENTITY);
        let (code, _) = call(&app, "POST", "/start", json!({ "network": "../x" })).await;
        assert_eq!(code, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn networks_bots_and_tournaments() {
        let config = test_config("control-registry");
        let controller = Arc::new(Mutex::new(Controller::new(
            config.clone(),
            DevicePreference::Cpu,
        )));
        let app = router(Arc::clone(&controller));
        let no_body = Value::Null;

        // Networks.
        let (code, defaults) = call(&app, "GET", "/networks/defaults", no_body.clone()).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(defaults["selfplayNodes"], 100);
        let mut params = defaults.clone();
        params["learningRate"] = json!(0.0005);
        let create = json!({ "name": "fast-lr", "params": params });
        let (code, _) = call(&app, "POST", "/networks", create.clone()).await;
        assert_eq!(code, StatusCode::CREATED);
        assert_eq!(
            call(&app, "POST", "/networks", create).await.0,
            StatusCode::CONFLICT
        );
        let invalid = json!({ "name": "bad", "params": { "learningRate": -1.0 } });
        let (code, body) = call(&app, "POST", "/networks", invalid).await;
        assert_eq!(code, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body["error"].as_str().unwrap().contains("learning rate"));
        let (code, _) = call(&app, "POST", "/networks", json!({ "name": "classic" })).await;
        assert_eq!(code, StatusCode::UNPROCESSABLE_ENTITY);
        let saved = network::load_params(&config, "fast-lr").unwrap().unwrap();
        assert!((saved.learning_rate - 0.0005).abs() < 1e-12);

        // Bots, from a snapshot of the network.
        let snapshots = config.weights_dir().join("fast-lr");
        std::fs::create_dir_all(&snapshots).unwrap();
        std::fs::write(
            snapshots.join(snapshot_name(5)),
            NnueWeights::zeros().to_bytes(),
        )
        .unwrap();
        let bot = json!({ "name": "lr-sb5", "network": "fast-lr", "superbatch": 5 });
        let (code, info) = call(&app, "POST", "/bots", bot.clone()).await;
        assert_eq!(code, StatusCode::CREATED);
        assert_eq!(info["params"]["learningRate"], 0.0005);
        assert_eq!(
            call(&app, "POST", "/bots", bot).await.0,
            StatusCode::CONFLICT
        );
        let missing = json!({ "name": "other", "network": "fast-lr", "superbatch": 6 });
        assert_eq!(
            call(&app, "POST", "/bots", missing).await.0,
            StatusCode::NOT_FOUND
        );

        // A tournament against the classical evaluation.
        let unknown = json!({
            "bots": ["classic", "nobody"], "gamesPerPair": 2,
            "limit": { "kind": "depth", "value": 2 }
        });
        assert_eq!(
            call(&app, "POST", "/tournaments", unknown).await.0,
            StatusCode::NOT_FOUND
        );
        let request = json!({
            "bots": ["classic", "lr-sb5"], "gamesPerPair": 2,
            "limit": { "kind": "nodes", "value": 100 }, "threads": 2
        });
        let (code, body) = call(&app, "POST", "/tournaments", request).await;
        assert_eq!(code, StatusCode::ACCEPTED);
        let id = body["id"].as_str().unwrap().to_owned();
        wait_for(&app, |s| s.tournament.is_none()).await;
        let saved: Value = serde_json::from_slice(
            &std::fs::read(config.tournaments_dir().join(format!("{id}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["state"], "finished");
        assert_eq!(saved["standings"].as_array().unwrap().len(), 2);

        // Deleting.
        let delete = |uri: &'static str| call(&app, "DELETE", uri, Value::Null);
        assert_eq!(delete("/bots/lr-sb5").await.0, StatusCode::NO_CONTENT);
        assert_eq!(delete("/bots/lr-sb5").await.0, StatusCode::NOT_FOUND);
        assert_eq!(delete("/networks/fast-lr").await.0, StatusCode::NO_CONTENT);
        assert!(network::list(&config).is_empty());
        assert_eq!(delete("/networks/fast-lr").await.0, StatusCode::NOT_FOUND);
    }
}
