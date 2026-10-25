//! Integration test of `/ws` over a real socket.

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chess_server::api::router;
use chess_server::api::types::{ClientMessage, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

async fn next_message<S>(stream: &mut S) -> ServerMessage
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        if let Message::Text(text) = stream.next().await.unwrap().unwrap() {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

#[tokio::test]
async fn hello_then_ping_pong() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router()).await.unwrap() });

    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .unwrap();

    assert!(matches!(
        next_message(&mut socket).await,
        ServerMessage::Hello { .. }
    ));

    let ping = serde_json::to_string(&ClientMessage::Ping { id: 7 }).unwrap();
    socket.send(Message::text(ping)).await.unwrap();
    assert_eq!(
        next_message(&mut socket).await,
        ServerMessage::Pong { id: 7 }
    );

    socket
        .send(Message::text("{\"type\":\"nope\"}"))
        .await
        .unwrap();
    assert!(matches!(
        next_message(&mut socket).await,
        ServerMessage::Error { .. }
    ));
}
