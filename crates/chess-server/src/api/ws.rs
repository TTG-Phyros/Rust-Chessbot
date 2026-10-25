use axum::extract::WebSocketUpgrade;
use axum::extract::ws::{Message, WebSocket};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};

use super::types::{ClientMessage, ServerMessage};

pub(super) async fn upgrade(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(session)
}

async fn session(socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();

    let hello = ServerMessage::Hello {
        version: env!("CARGO_PKG_VERSION").into(),
    };
    if send(&mut sink, &hello).await.is_err() {
        return;
    }

    while let Some(Ok(message)) = stream.next().await {
        let reply = match message {
            Message::Text(text) => match serde_json::from_str::<ClientMessage>(&text) {
                Ok(ClientMessage::Ping { id }) => ServerMessage::Pong { id },
                Err(err) => ServerMessage::Error {
                    message: err.to_string(),
                },
            },
            Message::Close(_) => break,
            _ => continue,
        };
        if send(&mut sink, &reply).await.is_err() {
            break;
        }
    }
    tracing::debug!("websocket session closed");
}

async fn send<S>(sink: &mut S, message: &ServerMessage) -> Result<(), axum::Error>
where
    S: SinkExt<Message, Error = axum::Error> + Unpin,
{
    // Serializing these plain enums cannot fail; fall back to an empty object anyway.
    let json = serde_json::to_string(message).unwrap_or_else(|_| "{}".into());
    sink.send(Message::Text(json.into())).await
}
