//! Sync against a local stand-in for the PubAPI: what is downloaded, what
//! comes from the cache.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chess_chesscom::{Client, ClientConfig, GameFilter, Month, TimeClass};
use chess_reviewer::{MonthSource, Store, SyncEvent, sync_player};
use serde_json::{Value, json};

/// Months served, oldest first: two finished ones and the current one. The
/// player has no game in the month before the current one, so that the
/// finished months are over for more than a day, whatever today's date.
fn served_months() -> Vec<Month> {
    let before = |month: Month| Month::containing(month.start() - 1);
    let current = Month::containing(chess_reviewer::now());
    let previous = before(before(current));
    vec![before(previous), previous, current]
}

/// Requests received, as paths (304 answers prefixed with `304 `).
type Log = Arc<Mutex<Vec<String>>>;

async fn archives(State(log): State<Log>, headers: HeaderMap) -> Response {
    if headers.contains_key(header::IF_NONE_MATCH) {
        log.lock().unwrap().push("304 archives".into());
        return StatusCode::NOT_MODIFIED.into_response();
    }
    log.lock().unwrap().push("archives".into());
    let urls: Vec<String> = served_months()
        .iter()
        .map(|m| {
            format!(
                "https://api.chess.com/pub/player/alice/games/{:04}/{:02}",
                m.year, m.month
            )
        })
        .collect();
    (
        [(header::ETAG, "\"list\"")],
        Json(json!({ "archives": urls })),
    )
        .into_response()
}

/// Three blitz games and one bullet game per month.
async fn month(
    State(log): State<Log>,
    Path((_, year, month)): Path<(String, i32, u8)>,
    headers: HeaderMap,
) -> Response {
    let month = Month { year, month };
    if headers.contains_key(header::IF_NONE_MATCH) {
        log.lock().unwrap().push(format!("304 {month}"));
        return StatusCode::NOT_MODIFIED.into_response();
    }
    log.lock().unwrap().push(month.to_string());
    let games: Vec<Value> = (0..4)
        .map(|i| {
            json!({
                "url": format!("https://www.chess.com/game/live/{month}-{i}"),
                "uuid": format!("{month}-{i}"),
                "pgn": "1. e4 e5 *",
                "time_control": "180", "end_time": month.start() + 3600 * (i + 1),
                "rated": true, "rules": "chess",
                "time_class": if i == 3 { "bullet" } else { "blitz" },
                "white": {"username": "Alice", "rating": 1500, "result": "win"},
                "black": {"username": "bob", "rating": 1480, "result": "resigned"},
            })
        })
        .collect();
    ([(header::ETAG, "\"m\"")], Json(json!({ "games": games }))).into_response()
}

async fn serve() -> (Client, Log) {
    let log = Log::default();
    let app = Router::new()
        .route("/pub/player/{name}/games/archives", get(archives))
        .route("/pub/player/{name}/games/{year}/{month}", get(month))
        .with_state(Arc::clone(&log));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new(ClientConfig {
        base_url: format!("http://{address}/pub"),
        user_agent: "chess-bot tests".into(),
        min_interval: Duration::ZERO,
        ..ClientConfig::default()
    })
    .unwrap();
    (client, log)
}

fn requests(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.lock().unwrap())
}

#[tokio::test]
async fn reads_only_what_is_needed_then_uses_the_cache() {
    let (client, log) = serve().await;
    let store = Store::in_memory().unwrap();
    let [oldest, previous, current] = served_months()[..] else {
        unreachable!()
    };
    let blitz = |max_games| GameFilter {
        time_classes: vec![TimeClass::Blitz],
        max_games,
        ..GameFilter::default()
    };

    // 5 blitz games: the current month (3) and the previous one (2 of 3).
    let mut events = Vec::new();
    let games = sync_player(&client, &store, "Alice", &blitz(5), 3600, |e| {
        events.push(e)
    })
    .await
    .unwrap();
    assert_eq!(games.len(), 5);
    assert!(games.windows(2).all(|w| w[0].end_time >= w[1].end_time));
    assert!(games.iter().all(|g| g.time_class == TimeClass::Blitz));
    assert_eq!(
        requests(&log),
        [
            "archives".to_owned(),
            current.to_string(),
            previous.to_string()
        ]
    );
    assert_eq!(
        events[0],
        SyncEvent::Months {
            total: 3,
            cached: false
        }
    );
    assert_eq!(events.len(), 3, "the oldest month is not read");

    // Same request within the TTL: nothing is downloaded.
    let again = sync_player(&client, &store, "alice", &blitz(5), 3600, |_| {})
        .await
        .unwrap();
    assert_eq!(again, games);
    assert!(requests(&log).is_empty());

    // TTL elapsed: the list and the current month are revalidated (304), the
    // finished month comes from the cache; the oldest one is downloaded.
    let mut sources = Vec::new();
    let all = sync_player(&client, &store, "alice", &blitz(100), 0, |event| {
        if let SyncEvent::Month { month, source, .. } = event {
            sources.push((month, source));
        }
    })
    .await
    .unwrap();
    assert_eq!(all.len(), 9);
    assert_eq!(
        requests(&log),
        [
            "304 archives".to_owned(),
            format!("304 {current}"),
            oldest.to_string()
        ]
    );
    assert_eq!(
        sources,
        [
            (current, MonthSource::Revalidated),
            (previous, MonthSource::Cache),
            (oldest, MonthSource::Downloaded),
        ]
    );
}
