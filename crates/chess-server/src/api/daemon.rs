//! Client of the training control daemon (`chess-train serve`). The daemon
//! makes every change to the data directory (networks, bots, tournaments);
//! the server reads the files and relays the commands.

use std::time::Duration;

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::ApiError;
use super::types::{ControllerState, ControllerView};
use crate::AppState;

/// `GET /status` of the control daemon.
#[derive(Debug, Deserialize)]
struct DaemonStatus {
    state: ControllerState,
    run: String,
    error: Option<String>,
    #[serde(default)]
    tournament: Option<String>,
}

const NOT_RUNNING: &str =
    "training service not reachable: start it with `docker compose --profile train up`";

fn unavailable(error: impl Into<String>) -> ControllerView {
    ControllerView {
        state: ControllerState::Unavailable,
        run: None,
        error: Some(error.into()),
        tournament: None,
    }
}

/// State of the daemon (never fails: an unreachable daemon is a state).
pub(super) async fn controller(state: &AppState) -> ControllerView {
    let Some(url) = &state.trainer_url else {
        return unavailable("no training service configured (CHESS_SERVER_TRAINER_URL)");
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
                tournament: status.tournament,
            },
            Err(err) => unavailable(format!(
                "unexpected answer from the training service: {err}"
            )),
        },
        Err(_) => unavailable(NOT_RUNNING),
    }
}

/// Sends a command to the daemon and returns its JSON answer (`null` when
/// empty). Its refusals keep their status and message.
pub(super) async fn send(
    state: &AppState,
    method: Method,
    path: &str,
    body: Option<&impl Serialize>,
) -> Result<Value, ApiError> {
    let url = state
        .trainer_url
        .as_ref()
        .ok_or_else(|| ApiError::unavailable("no training service configured"))?;
    let mut request = state
        .http
        .request(method, format!("{url}{path}"))
        .timeout(Duration::from_secs(10));
    if let Some(body) = body {
        request = request.json(body);
    }
    let response = request
        .send()
        .await
        .map_err(|_| ApiError::unavailable(NOT_RUNNING))?;
    let code = response.status().as_u16();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if (200..300).contains(&code) {
        return Ok(body);
    }
    let message = body.get("error").and_then(Value::as_str).map_or_else(
        || format!("training service answered HTTP {code}"),
        str::to_owned,
    );
    Err(match code {
        404 => ApiError::not_found(message),
        409 => ApiError::conflict(message),
        400 | 422 => ApiError::unprocessable(message),
        _ => ApiError::internal(message),
    })
}

/// Network and bot names, checked before they go into a URL or a path:
/// 1 to 32 letters, digits, `-` or `_`, starting with a letter or a digit.
pub(super) fn check_name(name: &str) -> Result<(), ApiError> {
    let valid = (1..=32).contains(&name.len())
        && name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(ApiError::unprocessable(format!(
            "invalid name {name:?}: 1 to 32 letters, digits, '-' or '_'"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_checked() {
        assert!(check_name("main-2_b").is_ok());
        for name in ["", "../x", "a/b", "-a", "a b", &"x".repeat(33)] {
            assert!(check_name(name).is_err(), "{name}");
        }
    }
}
