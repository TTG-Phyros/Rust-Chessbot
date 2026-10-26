//! Training endpoints: progress from the metrics files written by
//! `chess-train` (shared `data` volume), start/stop relayed to its control
//! daemon.

use std::path::Path;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;

use super::error::ApiError;
use super::types::{
    ControllerState, ControllerView, EloPoint, EngineInfo, ErrorResponse, LossPoint,
    StartTrainingRequest, TrainingStatus, TrainingView,
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

/// `GET /status` of the control daemon.
#[derive(Debug, Deserialize)]
struct DaemonStatus {
    state: ControllerState,
    run: String,
    error: Option<String>,
}

const NOT_RUNNING: &str =
    "training service not reachable: start it with `docker compose --profile train up`";

async fn controller(state: &AppState) -> ControllerView {
    let Some(url) = &state.trainer_url else {
        return ControllerView {
            state: ControllerState::Unavailable,
            run: None,
            error: Some("no training service configured (CHESS_SERVER_TRAINER_URL)".into()),
        };
    };
    let response = state
        .http
        .get(format!("{url}/status"))
        .timeout(Duration::from_secs(2))
        .send()
        .await;
    match response {
        Ok(response) => match response.json::<DaemonStatus>().await {
            Ok(status) => ControllerView {
                state: status.state,
                run: Some(status.run),
                error: status.error,
            },
            Err(err) => ControllerView {
                state: ControllerState::Unavailable,
                run: None,
                error: Some(format!(
                    "unexpected answer from the training service: {err}"
                )),
            },
        },
        Err(_) => ControllerView {
            state: ControllerState::Unavailable,
            run: None,
            error: Some(NOT_RUNNING.into()),
        },
    }
}

/// Training progress: daemon state, run status, loss and Elo curves, and the
/// evaluation the engine currently uses.
#[utoipa::path(
    get,
    path = "/api/training",
    tag = "training",
    responses((status = 200, description = "Training progress", body = TrainingView))
)]
pub(super) async fn training(State(state): State<AppState>) -> Json<TrainingView> {
    let controller = controller(&state).await;
    let metrics_dir = state.data_dir.join("metrics");
    let status = read_status(&metrics_dir);
    let run = controller
        .run
        .clone()
        .or_else(|| status.as_ref().map(|s| s.run.clone()));
    // Only show the status file if it belongs to the run being shown.
    let status = status.filter(|s| run.as_deref() == Some(s.run.as_str()));
    let (loss, elo) = run
        .as_deref()
        .map(|run| read_curves(&metrics_dir, run))
        .unwrap_or_default();
    Json(TrainingView {
        controller,
        status,
        loss,
        elo,
        engine: state.engine.info(),
    })
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

async fn relay(
    state: &AppState,
    path: &str,
    body: Option<&StartTrainingRequest>,
) -> Result<StatusCode, ApiError> {
    let url = state
        .trainer_url
        .as_ref()
        .ok_or_else(|| ApiError::unavailable("no training service configured"))?;
    let mut request = state
        .http
        .post(format!("{url}{path}"))
        .timeout(Duration::from_secs(5));
    if let Some(body) = body {
        request = request.json(body);
    }
    let response = request
        .send()
        .await
        .map_err(|_| ApiError::unavailable(NOT_RUNNING))?;
    match response.status().as_u16() {
        200..=299 => Ok(StatusCode::ACCEPTED),
        409 => Err(ApiError::conflict("a training run is already in progress")),
        code => Err(ApiError::internal(format!(
            "training service answered HTTP {code}"
        ))),
    }
}

/// Starts the self-play + training pipeline.
#[utoipa::path(
    post,
    path = "/api/training/start",
    tag = "training",
    request_body = StartTrainingRequest,
    responses(
        (status = 202, description = "Run started"),
        (status = 409, description = "A run is already in progress", body = ErrorResponse),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn start(
    State(state): State<AppState>,
    body: Option<Json<StartTrainingRequest>>,
) -> Result<StatusCode, ApiError> {
    let request = body.map(|Json(request)| request).unwrap_or_default();
    relay(&state, "/start", Some(&request)).await
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
    relay(&state, "/stop", None).await
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
