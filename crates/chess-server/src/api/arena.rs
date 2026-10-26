//! Arena endpoints: bots (saved networks) and the tournaments they play,
//! read from the files written by `chess-train`; commands relayed to its
//! control daemon, which plays the games.

use std::path::Path;

use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use reqwest::Method;
use serde::de::DeserializeOwned;

use super::daemon::{self, check_name};
use super::error::ApiError;
use super::types::{
    ArenaView, BotInfo, ErrorResponse, SaveBotRequest, Tournament, TournamentRequest,
    TournamentStarted, TournamentState,
};
use crate::AppState;

/// Tournaments listed by `GET /api/arena`.
const TOURNAMENTS_SHOWN: usize = 20;

/// Every `*.json` of `dir` that parses as `T`.
fn read_all<T: DeserializeOwned>(dir: &Path) -> Vec<T> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| serde_json::from_slice(&std::fs::read(path).ok()?).ok())
        .collect()
}

fn read_bots(data_dir: &Path) -> Vec<BotInfo> {
    let dir = data_dir.join("bots");
    let mut bots: Vec<BotInfo> = read_all(&dir);
    // A description without its network is a leftover.
    bots.retain(|bot| dir.join(format!("{}.nnue", bot.name)).exists());
    bots.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.name.cmp(&b.name)));
    bots
}

/// The latest tournaments. One left `running` by a daemon that no longer
/// plays it (stopped, restarted) is shown as stopped.
fn read_tournaments(data_dir: &Path, playing: Option<&str>) -> Vec<Tournament> {
    let mut tournaments: Vec<Tournament> = read_all(&data_dir.join("tournaments"));
    tournaments.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    tournaments.truncate(TOURNAMENTS_SHOWN);
    for tournament in &mut tournaments {
        if tournament.state == TournamentState::Running && playing != Some(tournament.id.as_str()) {
            tournament.state = TournamentState::Stopped;
        }
    }
    tournaments
}

/// Bots and tournaments.
#[utoipa::path(
    get,
    path = "/api/arena",
    tag = "arena",
    responses((status = 200, description = "Bots and tournaments", body = ArenaView))
)]
pub(super) async fn arena(State(state): State<AppState>) -> Json<ArenaView> {
    let controller = daemon::controller(&state).await;
    let tournaments = read_tournaments(&state.data_dir, controller.tournament.as_deref());
    Json(ArenaView {
        controller,
        bots: read_bots(&state.data_dir),
        tournaments,
    })
}

/// Saves a snapshot (or the latest export) of a network as a bot.
#[utoipa::path(
    post,
    path = "/api/bots",
    tag = "arena",
    request_body = SaveBotRequest,
    responses(
        (status = 201, description = "Bot saved", body = BotInfo),
        (status = 404, description = "No such snapshot", body = ErrorResponse),
        (status = 409, description = "The name is taken", body = ErrorResponse),
        (status = 422, description = "Invalid name", body = ErrorResponse),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn save_bot(
    State(state): State<AppState>,
    Json(request): Json<SaveBotRequest>,
) -> Result<(StatusCode, Json<BotInfo>), ApiError> {
    check_name(&request.name)?;
    check_name(&request.network)?;
    let body = daemon::send(&state, Method::POST, "/bots", Some(&request)).await?;
    let info = serde_json::from_value(body)
        .map_err(|err| ApiError::internal(format!("unexpected answer: {err}")))?;
    Ok((StatusCode::CREATED, Json(info)))
}

/// Deletes a bot.
#[utoipa::path(
    delete,
    path = "/api/bots/{name}",
    tag = "arena",
    params(("name" = String, Path, description = "Bot")),
    responses(
        (status = 204, description = "Bot deleted"),
        (status = 404, description = "No such bot", body = ErrorResponse),
        (status = 409, description = "A tournament is in progress", body = ErrorResponse),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn delete_bot(
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
) -> Result<StatusCode, ApiError> {
    check_name(&name)?;
    daemon::send(
        &state,
        Method::DELETE,
        &format!("/bots/{name}"),
        None::<&()>,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Starts a round-robin tournament between bots.
#[utoipa::path(
    post,
    path = "/api/tournaments",
    tag = "arena",
    request_body = TournamentRequest,
    responses(
        (status = 202, description = "Tournament started", body = TournamentStarted),
        (status = 404, description = "Unknown bot", body = ErrorResponse),
        (status = 409, description = "A tournament is in progress", body = ErrorResponse),
        (status = 422, description = "Invalid request", body = ErrorResponse),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn start_tournament(
    State(state): State<AppState>,
    Json(request): Json<TournamentRequest>,
) -> Result<(StatusCode, Json<TournamentStarted>), ApiError> {
    let body = daemon::send(&state, Method::POST, "/tournaments", Some(&request)).await?;
    let started = serde_json::from_value(body)
        .map_err(|err| ApiError::internal(format!("unexpected answer: {err}")))?;
    Ok((StatusCode::ACCEPTED, Json(started)))
}

/// Stops the tournament in progress (the games being played are finished).
#[utoipa::path(
    post,
    path = "/api/tournaments/stop",
    tag = "arena",
    responses(
        (status = 202, description = "Stop requested"),
        (status = 503, description = "Training service not running", body = ErrorResponse),
    )
)]
pub(super) async fn stop_tournament(State(state): State<AppState>) -> Result<StatusCode, ApiError> {
    daemon::send(&state, Method::POST, "/tournaments/stop", None::<&()>).await?;
    Ok(StatusCode::ACCEPTED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bots_and_tournaments() {
        let dir = std::env::temp_dir().join(format!("chess-server-arena-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bots")).unwrap();
        std::fs::create_dir_all(dir.join("tournaments")).unwrap();
        for (name, created) in [("old", 1), ("new", 2), ("gone", 3)] {
            std::fs::write(
                dir.join("bots").join(format!("{name}.json")),
                format!(r#"{{"name":"{name}","network":"main","superbatch":5,"elo":12.5,"eloError":40.0,"params":null,"createdAt":{created}}}"#),
            )
            .unwrap();
            if name != "gone" {
                std::fs::write(dir.join("bots").join(format!("{name}.nnue")), b"").unwrap();
            }
        }
        let bots = read_bots(&dir);
        assert_eq!(
            bots.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
            ["new", "old"]
        );
        assert_eq!(bots[0].elo, Some(12.5));

        let tournament = |id: &str, created: u64| {
            format!(
                r#"{{"id":"{id}","state":"running","error":null,"createdAt":{created},"updatedAt":{created},"bots":["classic","new"],"gamesPerPair":2,"limit":{{"kind":"depth","value":6}},"pairings":[],"standings":[]}}"#
            )
        };
        std::fs::write(dir.join("tournaments/t1.json"), tournament("t1", 1)).unwrap();
        std::fs::write(dir.join("tournaments/t2.json"), tournament("t2", 2)).unwrap();
        std::fs::write(dir.join("tournaments/broken.json"), "{").unwrap();
        let tournaments = read_tournaments(&dir, Some("t2"));
        assert_eq!(
            tournaments
                .iter()
                .map(|t| (t.id.as_str(), t.state))
                .collect::<Vec<_>>(),
            [
                ("t2", TournamentState::Running),
                ("t1", TournamentState::Stopped)
            ]
        );
    }
}
