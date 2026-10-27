//! `/ws`: live analysis. The client sends `analyze` (replacing any analysis
//! in progress) or `stop`; the server streams one `info` per completed depth
//! and a final `done`.

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use chess_search::StopFlag;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use super::convert::{analysis_info, replay};
use super::types::{ClientMessage, EngineLimits, GameRequest, ServerMessage};
use crate::AppState;
use crate::engine::analysis_limits;

pub(super) async fn upgrade(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| session(socket, state))
}

async fn session(socket: WebSocket, state: AppState) {
    let (mut sink, mut stream) = socket.split();
    // Every producer (this loop, analysis callbacks on blocking threads)
    // goes through one channel; a single task owns the socket's write half.
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
    let writer = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let Ok(json) = serde_json::to_string(&message) else {
                continue;
            };
            if sink.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let _ = tx.send(ServerMessage::Hello {
        version: env!("CARGO_PKG_VERSION").into(),
    });
    let mut current: Option<StopFlag> = None;

    while let Some(Ok(message)) = stream.next().await {
        let text = match message {
            Message::Text(text) => text,
            Message::Close(_) => break,
            _ => continue,
        };
        match serde_json::from_str::<ClientMessage>(&text) {
            Ok(ClientMessage::Ping { id }) => {
                let _ = tx.send(ServerMessage::Pong { id });
            }
            Ok(ClientMessage::Analyze {
                id,
                game,
                limits,
                bot,
            }) => {
                if let Some(stop) = current.take() {
                    stop.stop();
                }
                current = start_analysis(&state, &tx, id, &game, limits, bot);
            }
            Ok(ClientMessage::Stop) => {
                if let Some(stop) = current.take() {
                    stop.stop();
                }
            }
            Err(err) => {
                let _ = tx.send(ServerMessage::Error {
                    message: err.to_string(),
                });
            }
        }
    }

    if let Some(stop) = current {
        stop.stop();
    }
    drop(tx);
    let _ = writer.await;
    tracing::debug!("websocket session closed");
}

/// Starts an analysis in the background; returns its stop flag, or `None`
/// (after reporting the error) if the game is invalid.
fn start_analysis(
    state: &AppState,
    tx: &mpsc::UnboundedSender<ServerMessage>,
    id: u32,
    game: &GameRequest,
    limits: EngineLimits,
    bot: Option<String>,
) -> Option<StopFlag> {
    let position = match replay(game) {
        Ok(replayed) => replayed.position,
        Err(err) => {
            let _ = tx.send(ServerMessage::Error {
                message: err.message().to_owned(),
            });
            return None;
        }
    };
    let stop = StopFlag::new();
    let engine = state.engine.clone();
    let tx = tx.clone();
    let flag = stop.clone();
    tokio::spawn(async move {
        let info_tx = tx.clone();
        let root = position.clone();
        let on_info = move |info: &chess_search::SearchInfo| {
            let _ = info_tx.send(ServerMessage::Info {
                info: analysis_info(id, &root, info),
            });
        };
        let message = match engine
            .search(
                bot.as_deref(),
                position,
                analysis_limits(limits),
                flag,
                on_info,
            )
            .await
        {
            Ok(result) => ServerMessage::Done {
                id,
                best_move: result.best_move.map(|mv| mv.to_string()),
            },
            Err(err) => ServerMessage::Error {
                message: err.message().to_owned(),
            },
        };
        let _ = tx.send(message);
    });
    Some(stop)
}
