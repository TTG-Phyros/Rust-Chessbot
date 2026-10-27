//! The client against a local stand-in for the PubAPI.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chess_chesscom::{Client, ClientConfig, Error, Fetched, Month, Validators};
use serde_json::json;

#[derive(Default)]
struct Counters {
    archive_hits: AtomicUsize,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
}

async fn archives(State(counters): State<Arc<Counters>>, Path(name): Path<String>) -> Response {
    if name != "alice" {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"code": 0, "message": "not found"})),
        )
            .into_response();
    }
    // Rate limited the first time.
    if counters.archive_hits.fetch_add(1, Ordering::SeqCst) == 0 {
        return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")]).into_response();
    }
    Json(json!({"archives": [
        "https://api.chess.com/pub/player/alice/games/2026/09",
        "https://api.chess.com/pub/player/alice/games/2026/10",
    ]}))
    .into_response()
}

async fn month(
    State(counters): State<Arc<Counters>>,
    Path((_, year, month)): Path<(String, i32, u8)>,
    headers: HeaderMap,
) -> Response {
    let now = counters.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
    counters.max_in_flight.fetch_max(now, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(30)).await;
    counters.in_flight.fetch_sub(1, Ordering::SeqCst);

    let etag = format!("\"{year}-{month}\"");
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(etag.as_str())
    {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    let game = json!({
        "url": format!("https://www.chess.com/game/live/{month}"),
        "uuid": format!("uuid-{month}"),
        "pgn": "[White \"alice\"]\n[Black \"bob\"]\n\n1. e4 {[%clk 0:02:59]} 1... e5 1-0",
        "time_control": "180", "end_time": 1_790_900_000, "rated": true,
        "time_class": "blitz", "rules": "chess",
        "white": {"username": "alice", "rating": 1500, "result": "win"},
        "black": {"username": "bob", "rating": 1480, "result": "resigned"},
    });
    ([(header::ETAG, etag)], Json(json!({"games": [game]}))).into_response()
}

async fn serve() -> (Client, Arc<Counters>) {
    let counters = Arc::new(Counters::default());
    let app = Router::new()
        .route("/pub/player/{name}/games/archives", get(archives))
        .route("/pub/player/{name}/games/{year}/{month}", get(month))
        .with_state(Arc::clone(&counters));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new(ClientConfig {
        base_url: format!("http://{address}/pub"),
        user_agent: "chess-bot tests".into(),
        min_interval: Duration::from_millis(5),
        initial_backoff: Duration::from_millis(5),
        ..ClientConfig::default()
    })
    .unwrap();
    (client, counters)
}

#[tokio::test]
async fn retries_rate_limited_requests() {
    let (client, counters) = serve().await;
    let Fetched::Fresh(months, _) = client
        .archives("Alice", &Validators::default())
        .await
        .unwrap()
    else {
        panic!("expected archives");
    };
    assert_eq!(
        months,
        [
            Month {
                year: 2026,
                month: 9
            },
            Month {
                year: 2026,
                month: 10
            }
        ]
    );
    assert_eq!(counters.archive_hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn unknown_players_are_reported() {
    let (client, _) = serve().await;
    let err = client
        .archives("nobody", &Validators::default())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::PlayerNotFound(name) if name == "nobody"));
}

#[tokio::test]
async fn revalidates_with_the_etag() {
    let (client, _) = serve().await;
    let october = Month {
        year: 2026,
        month: 10,
    };
    let Fetched::Fresh(games, validators) = client
        .month("alice", october, &Validators::default())
        .await
        .unwrap()
    else {
        panic!("expected games");
    };
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].id(), "uuid-10");
    assert_eq!(validators.etag.as_deref(), Some("\"2026-10\""));
    assert_eq!(
        client.month("alice", october, &validators).await.unwrap(),
        Fetched::NotModified
    );
}

#[tokio::test]
async fn sends_one_request_at_a_time() {
    let (client, counters) = serve().await;
    let client = Arc::new(client);
    let tasks: Vec<_> = (1..=6)
        .map(|month| {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .month("alice", Month { year: 2026, month }, &Validators::default())
                    .await
            })
        })
        .collect();
    for task in tasks {
        assert!(task.await.unwrap().is_ok());
    }
    assert_eq!(counters.max_in_flight.load(Ordering::SeqCst), 1);
}
