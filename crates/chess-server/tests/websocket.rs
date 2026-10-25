//! Integration tests of `/ws` over a real socket.

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use chess_server::AppState;
use chess_server::api::router;
use chess_server::api::types::{
    ClientMessage, EngineLimits, GameRequest, ScoreView, ServerMessage,
};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect() -> Socket {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router(AppState::default()))
            .await
            .unwrap()
    });
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .unwrap();
    assert!(matches!(
        next(&mut socket).await,
        ServerMessage::Hello { .. }
    ));
    socket
}

async fn next(socket: &mut Socket) -> ServerMessage {
    let read = async {
        loop {
            if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
                return serde_json::from_str(&text).unwrap();
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(20), read)
        .await
        .expect("no message in time")
}

async fn send(socket: &mut Socket, message: &ClientMessage) {
    socket
        .send(Message::text(serde_json::to_string(message).unwrap()))
        .await
        .unwrap();
}

fn analyze(id: u32, fen: Option<&str>, limits: EngineLimits) -> ClientMessage {
    ClientMessage::Analyze {
        id,
        game: GameRequest {
            fen: fen.map(str::to_owned),
            moves: Vec::new(),
        },
        limits,
    }
}

#[tokio::test]
async fn ping_pong_and_errors() {
    let mut socket = connect().await;
    send(&mut socket, &ClientMessage::Ping { id: 7 }).await;
    assert_eq!(next(&mut socket).await, ServerMessage::Pong { id: 7 });
    socket
        .send(Message::text("{\"type\":\"nope\"}"))
        .await
        .unwrap();
    assert!(matches!(
        next(&mut socket).await,
        ServerMessage::Error { .. }
    ));
    send(
        &mut socket,
        &analyze(1, Some("bad fen"), EngineLimits::default()),
    )
    .await;
    assert!(matches!(
        next(&mut socket).await,
        ServerMessage::Error { .. }
    ));
}

#[tokio::test]
async fn analysis_streams_each_depth_then_done() {
    let mut socket = connect().await;
    let limits = EngineLimits {
        depth: Some(4),
        ..EngineLimits::default()
    };
    send(
        &mut socket,
        &analyze(3, Some("6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1"), limits),
    )
    .await;
    let mut depths = Vec::new();
    loop {
        match next(&mut socket).await {
            ServerMessage::Info { info } => {
                assert_eq!(info.id, 3);
                assert_eq!(info.pv.len(), info.pv_san.len());
                depths.push(info.depth);
                if info.depth >= 2 {
                    assert_eq!(info.score, ScoreView::Mate { moves: 1 });
                    assert_eq!(info.pv_san[0], "Rd8#");
                }
            }
            ServerMessage::Done { id, best_move } => {
                assert_eq!(id, 3);
                assert_eq!(best_move.as_deref(), Some("d1d8"));
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(depths, [1, 2, 3, 4]);
}

#[tokio::test]
async fn infinite_analysis_stops_on_request() {
    let mut socket = connect().await;
    send(&mut socket, &analyze(9, None, EngineLimits::default())).await;
    assert!(matches!(
        next(&mut socket).await,
        ServerMessage::Info { .. }
    ));
    send(&mut socket, &ClientMessage::Stop).await;
    loop {
        if let ServerMessage::Done { id, best_move } = next(&mut socket).await {
            assert_eq!(id, 9);
            assert!(best_move.is_some());
            break;
        }
    }
}

#[tokio::test]
async fn new_analysis_replaces_the_previous_one() {
    let mut socket = connect().await;
    send(&mut socket, &analyze(1, None, EngineLimits::default())).await;
    let limits = EngineLimits {
        depth: Some(3),
        ..EngineLimits::default()
    };
    send(&mut socket, &analyze(2, None, limits)).await;
    let mut done = Vec::new();
    while done.len() < 2 {
        if let ServerMessage::Done { id, .. } = next(&mut socket).await {
            done.push(id);
        }
    }
    done.sort_unstable();
    assert_eq!(done, [1, 2]);
}
