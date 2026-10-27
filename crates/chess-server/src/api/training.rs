//! Training endpoints: networks and their progress, read from the files
//! written by `chess-train` (shared `data` volume); commands relayed to its
//! control daemon.

use std::path::Path;

use axum::Json;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::StatusCode;
use reqwest::Method;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use utoipa::IntoParams;

use super::daemon::{self, check_name};
use super::error::ApiError;
use super::types::{
    CreateNetworkRequest, EloPoint, EngineInfo, ErrorResponse, LiveGame, LossPoint, NetworkParams,
    NetworkSummary, SelfplayView, StartTrainingRequest, TrainingStatus, TrainingTimerRequest,
    TrainingView,
};
use crate::AppState;

/// `status.json`, as written by `chess-train` (snake_case).
#[derive(Debug, Deserialize)]
struct StatusFile {
    run: String,
    #[serde(default)]
    phase: String,
    #[serde(default)]
    device: String,
    #[serde(default)]
    started_at: u64,
    #[serde(default)]
    updated_at: u64,
    #[serde(default)]
    games: u64,
    #[serde(default)]
    positions: u64,
    #[serde(default)]
    superbatch: u64,
    loss: Option<f64>,
    learning_rate: Option<f64>,
    positions_per_second: Option<f64>,
    vram_used_mb: Option<u64>,
    latest_weights: Option<String>,
    elo: Option<f64>,
    elo_error: Option<f64>,
}

/// One line of `events.jsonl` (only the fields the charts need).
#[derive(Debug, Deserialize)]
struct Event {
    kind: String,
    run: Option<String>,
    superbatch: Option<u64>,
    loss: Option<f64>,
    learning_rate: Option<f64>,
    positions_per_second: Option<f64>,
    elo: Option<f64>,
    elo_error: Option<f64>,
    games: Option<u32>,
}

fn read_status(metrics_dir: &Path) -> Option<TrainingStatus> {
    let bytes = std::fs::read(metrics_dir.join("status.json")).ok()?;
    let file: StatusFile = serde_json::from_slice(&bytes).ok()?;
    Some(TrainingStatus {
        run: file.run,
        phase: file.phase,
        device: file.device,
        started_at: file.started_at,
        updated_at: file.updated_at,
        games: file.games,
        positions: file.positions,
        superbatch: file.superbatch,
        loss: file.loss,
        learning_rate: file.learning_rate,
        positions_per_second: file.positions_per_second,
        vram_used_mb: file.vram_used_mb,
        latest_weights: file.latest_weights,
        elo: file.elo,
        elo_error: file.elo_error,
    })
}

/// Loss and Elo curves of `run`. A run restarted from scratch reuses
/// superbatch numbers: only the points after the last restart are kept.
fn read_curves(metrics_dir: &Path, run: &str) -> (Vec<LossPoint>, Vec<EloPoint>) {
    let Ok(text) = std::fs::read_to_string(metrics_dir.join("events.jsonl")) else {
        return (Vec::new(), Vec::new());
    };
    let mut loss: Vec<LossPoint> = Vec::new();
    let mut elo: Vec<EloPoint> = Vec::new();
    for event in text
        .lines()
        .filter_map(|line| serde_json::from_str::<Event>(line).ok())
    {
        if event.run.as_deref() != Some(run) {
            continue;
        }
        let Some(superbatch) = event.superbatch else {
            continue;
        };
        match event.kind.as_str() {
            "superbatch" => {
                if loss
                    .last()
                    .is_some_and(|last| last.superbatch >= superbatch)
                {
                    loss.retain(|point| point.superbatch < superbatch);
                    elo.retain(|point| point.superbatch < superbatch);
                }
                loss.push(LossPoint {
                    superbatch,
                    loss: event.loss.unwrap_or_default(),
                    learning_rate: event.learning_rate.unwrap_or_default(),
                    positions_per_second: event.positions_per_second.unwrap_or_default(),
                });
            }
            "match" => {
                elo.retain(|point| point.superbatch != superbatch);
                elo.push(EloPoint {
                    superbatch,
                    elo: event.elo.unwrap_or_default(),
                    error: event.elo_error.unwrap_or_default(),
                    games: event.games.unwrap_or_default(),
                });
            }
            _ => {}
        }
    }
    elo.sort_by_key(|point| point.superbatch);
    (loss, elo)
}

/// `checkpoints/<name>/state.json`, as written by `chess-train`.
#[derive(Debug, Deserialize)]
struct CheckpointState {
    superbatch: u64,
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Every network: with settings (`runs/`), a checkpoint or exported weights.
fn read_networks(data_dir: &Path) -> Vec<NetworkSummary> {
    let entries = |dir: &str| {
        std::fs::read_dir(data_dir.join(dir))
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|e| e.path()))
    };
    let stem = |path: &Path| Some(path.file_stem()?.to_str()?.to_owned());
    let mut names: Vec<String> = entries("runs")
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .chain(entries("checkpoints").filter(|path| path.is_dir()))
        .chain(entries("weights").filter(|path| path.extension().is_some_and(|ext| ext == "nnue")))
        .filter_map(|path| stem(&path))
        .filter(|name| check_name(name).is_ok())
        .collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|name| NetworkSummary {
            params: read_json(&data_dir.join("runs").join(format!("{name}.json"))),
            superbatch: read_json::<CheckpointState>(
                &data_dir.join("checkpoints").join(&name).join("state.json"),
            )
            .map(|state| state.superbatch),
            best: read_json(&data_dir.join("weights").join(&name).join("best.json"))
                .unwrap_or_default(),
            has_weights: data_dir
                .join("weights")
                .join(format!("{name}.nnue"))
                .exists(),
            name,
        })
        .collect()
}

#[derive(Debug, Deserialize, IntoParams)]
pub(super) struct TrainingQuery {
    /// Network to show (default: the one training, or the last trained).
    network: Option<String>,
}

/// Training progress of a network: daemon state, status, loss and Elo
/// curves, every network, and the evaluation the engine currently uses.
#[utoipa::path(
    get,
    path = "/api/training",
    tag = "training",
    params(TrainingQuery),
    responses((status = 200, description = "Training progress", body = TrainingView))
)]
pub(super) async fn training(
    State(state): State<AppState>,
    Query(query): Query<TrainingQuery>,
) -> Json<TrainingView> {
    let controller = daemon::controller(&state).await;
    let metrics_dir = state.data_dir.join("metrics");
    let status = read_status(&metrics_dir);
    let network = query
        .network
        .filter(|name| check_name(name).is_ok())
        .or_else(|| controller.run.clone())
        .or_else(|| status.as_ref().map(|s| s.run.clone()));
    // Only show the status file if it belongs to the network shown.
    let status = status.filter(|s| network.as_deref() == Some(s.run.as_str()));
    let (loss, elo) = network
        .as_deref()
        .map(|network| read_curves(&metrics_dir, network))
        .unwrap_or_default();
    Json(TrainingView {
        controller,
        network,
        status,
        loss,
        elo,
        networks: read_networks(&state.data_dir),
        engine: state.engine.info(),
    })
}

/// `selfplay.json`, as written by `chess-train` (snake_case).
#[derive(Debug, Deserialize)]
struct SelfplayFile {
    updated_at: u64,
    #[serde(default)]
    boards: Vec<BoardFile>,
}

#[derive(Debug, Deserialize)]
struct BoardFile {
    worker: u32,
    game: u64,
    fen: String,
    ply: u32,
    last_move: Option<String>,
}

/// Boards older than this are a stopped run's last state.
const SELFPLAY_STALE_SECS: u64 = 10;

/// Reads the boards of the self-play workers; `now` in Unix seconds.
fn read_selfplay(metrics_dir: &Path, now: u64) -> SelfplayView {
    let Some(file) = std::fs::read(metrics_dir.join("selfplay.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SelfplayFile>(&bytes).ok())
    else {
        return SelfplayView {
            updated_at: None,
            live: false,
            games: Vec::new(),
        };
    };
    SelfplayView {
        updated_at: Some(file.updated_at),
        live: now.saturating_sub(file.updated_at) <= SELFPLAY_STALE_SECS,
        games: file
            .boards
            .into_iter()
            .map(|board| LiveGame {
                worker: board.worker,
                game: board.game,
                fen: board.fen,
                ply: board.ply,
                last_move: board.last_move,
            })
            .collect(),
    }
}

/// The games being played by the self-play workers, one board each.
#[utoipa::path(
    get,
    path = "/api/training/selfplay",
    tag = "training",
    responses((status = 200, description = "Self-play boards", body = SelfplayView))
)]
pub(super) async fn selfplay(State(state): State<AppState>) -> Json<SelfplayView> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |age| age.as_secs());
    Json(read_selfplay(&state.data_dir.join("metrics"), now))
}

/// Evaluation used by the engine.
#[utoipa::path(
    get,
    path = "/api/engine",
    tag = "engine",
    responses((status = 200, description = "Evaluation in use", body = EngineInfo))
)]
pub(super) async fn engine(State(state): State<AppState>) -> Json<EngineInfo> {
    Json(state.engine.info())
}

/// Starts the self-play + training pipeline of a network.
#[utoipa::path(
    post,
    path = "/api/training/start",
    tag = "training",
    request_body = StartTrainingRequest,
    responses(
        (status = 202, description = "Run started"),
        (status = 409, description = "A run is already in progress", body = ErrorResponse),
        (status = 422, description = "Invalid network name", body = ErrorResponse),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn start(
    State(state): State<AppState>,
    body: Option<Json<StartTrainingRequest>>,
) -> Result<StatusCode, ApiError> {
    let request = body.map(|Json(request)| request).unwrap_or_default();
    if let Some(network) = &request.network {
        check_name(network)?;
    }
    daemon::send(&state, Method::POST, "/start", Some(&request)).await?;
    Ok(StatusCode::ACCEPTED)
}

/// Stops the run (it finishes its step and writes a checkpoint first).
#[utoipa::path(
    post,
    path = "/api/training/stop",
    tag = "training",
    responses(
        (status = 202, description = "Stop requested"),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn stop(State(state): State<AppState>) -> Result<StatusCode, ApiError> {
    daemon::send(&state, Method::POST, "/stop", None::<&()>).await?;
    Ok(StatusCode::ACCEPTED)
}

/// Sets, moves or removes the time limit of the running training.
#[utoipa::path(
    post,
    path = "/api/training/timer",
    tag = "training",
    request_body = TrainingTimerRequest,
    responses(
        (status = 202, description = "Time limit changed"),
        (status = 409, description = "No training in progress", body = ErrorResponse),
        (status = 422, description = "Time limit out of range", body = ErrorResponse),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn timer(
    State(state): State<AppState>,
    Json(request): Json<TrainingTimerRequest>,
) -> Result<StatusCode, ApiError> {
    daemon::send(&state, Method::POST, "/timer", Some(&request)).await?;
    Ok(StatusCode::ACCEPTED)
}

/// Hyperparameters of a new network (the training service's settings).
#[utoipa::path(
    get,
    path = "/api/networks/defaults",
    tag = "training",
    responses(
        (status = 200, description = "Default hyperparameters", body = NetworkParams),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn network_defaults(
    State(state): State<AppState>,
) -> Result<Json<NetworkParams>, ApiError> {
    let body = daemon::send(&state, Method::GET, "/networks/defaults", None::<&()>).await?;
    serde_json::from_value(body)
        .map(Json)
        .map_err(|err| ApiError::internal(format!("unexpected answer: {err}")))
}

/// Creates a network with its hyperparameters (it trains once started).
#[utoipa::path(
    post,
    path = "/api/networks",
    tag = "training",
    request_body = CreateNetworkRequest,
    responses(
        (status = 201, description = "Network created"),
        (status = 409, description = "The name is taken", body = ErrorResponse),
        (status = 422, description = "Invalid name or hyperparameters", body = ErrorResponse),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn create_network(
    State(state): State<AppState>,
    Json(request): Json<CreateNetworkRequest>,
) -> Result<StatusCode, ApiError> {
    check_name(&request.name)?;
    daemon::send(&state, Method::POST, "/networks", Some(&request)).await?;
    Ok(StatusCode::CREATED)
}

/// Deletes a network: settings, checkpoint and weights (its games stay).
#[utoipa::path(
    delete,
    path = "/api/networks/{name}",
    tag = "training",
    params(("name" = String, Path, description = "Network")),
    responses(
        (status = 204, description = "Network deleted"),
        (status = 404, description = "No such network", body = ErrorResponse),
        (status = 409, description = "The network is training", body = ErrorResponse),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn delete_network(
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
) -> Result<StatusCode, ApiError> {
    check_name(&name)?;
    daemon::send(
        &state,
        Method::DELETE,
        &format!("/networks/{name}"),
        None::<&()>,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_keep_the_latest_restart_of_the_run() {
        let dir = std::env::temp_dir().join(format!("chess-server-curves-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lines = [
            r#"{"kind":"superbatch","run":"a","superbatch":1,"loss":0.5,"learning_rate":0.001,"positions_per_second":10}"#,
            r#"{"kind":"superbatch","run":"b","superbatch":1,"loss":0.9}"#,
            r#"{"kind":"superbatch","run":"a","superbatch":2,"loss":0.4}"#,
            r#"{"kind":"match","run":"a","superbatch":2,"elo":-150,"elo_error":60,"games":64}"#,
            "not json",
            // Run "a" restarted from scratch.
            r#"{"kind":"superbatch","run":"a","superbatch":1,"loss":0.6}"#,
            r#"{"kind":"checkpoint","run":"a","superbatch":1}"#,
        ];
        std::fs::write(dir.join("events.jsonl"), lines.join("\n")).unwrap();
        let (loss, elo) = read_curves(&dir, "a");
        assert_eq!(loss.len(), 1);
        assert!((loss[0].loss - 0.6).abs() < 1e-12);
        assert!(elo.is_empty());

        std::fs::write(dir.join("events.jsonl"), lines[..4].join("\n")).unwrap();
        let (loss, elo) = read_curves(&dir, "a");
        assert_eq!(
            loss.iter().map(|p| p.superbatch).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(elo.len(), 1);
        assert!((elo[0].elo + 150.0).abs() < 1e-12);
    }

    #[test]
    fn status_file_is_parsed() {
        let dir = std::env::temp_dir().join(format!("chess-server-status-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("status.json"),
            r#"{"run":"main","phase":"pipeline","device":"GPU","games":10,"superbatch":3,"loss":0.02,"vram_used_mb":1350,"elo":-40.5}"#,
        )
        .unwrap();
        let status = read_status(&dir).unwrap();
        assert_eq!(
            (status.run.as_str(), status.games, status.superbatch),
            ("main", 10, 3)
        );
        assert_eq!(status.vram_used_mb, Some(1350));
        assert_eq!(status.elo, Some(-40.5));
        assert!(read_status(&dir.join("missing")).is_none());
    }
}
