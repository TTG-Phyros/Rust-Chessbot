//! `/api/review/*` against a local stand-in for the Chess.com API, with our
//! own engine (no Stockfish needed) and an in-memory cache.

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::routing::get;
use axum::{Json, Router};
use chess_chesscom::Month;
use chess_reviewer::{EngineKind, ReviewConfig, ReviewService, Store};
use chess_server::AppState;
use chess_server::api::router;
use chess_server::api::types::ScoreView;
use chess_server::api::types::review::{
    ReviewGameSummary, ReviewGameView, ReviewJobState, ReviewJobView, ReviewStarted,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn fake_chesscom() -> String {
    let month = Month::containing(chess_reviewer::now());
    let archives = move || async move {
        Json(json!({ "archives": [format!(
            "https://api.chess.com/pub/player/alice/games/{:04}/{:02}",
            month.year, month.month
        )]}))
    };
    let games = move || async move {
        Json(json!({ "games": [
            {
                "url": "https://www.chess.com/game/live/1", "uuid": "fools-mate",
                "pgn": "[TimeControl \"180\"]\n\n1. f3 {[%clk 0:02:59]} 1... e5 {[%clk 0:02:58]} \
                        2. g4 {[%clk 0:02:57.5]} 2... Qh4# {[%clk 0:02:50]} 0-1",
                "time_control": "180", "end_time": month.start() + 10, "rated": true,
                "rules": "chess", "time_class": "blitz",
                "white": {"username": "Alice", "rating": 900, "result": "checkmated"},
                "black": {"username": "bob", "rating": 950, "result": "win"},
            },
            {
                "url": "https://www.chess.com/game/live/2", "uuid": "london",
                "pgn": "1. d4 d5 2. Bf4 Nf6 3. e3 1/2-1/2",
                "time_control": "600", "end_time": month.start() + 5, "rated": false,
                "rules": "chess", "time_class": "rapid",
                "white": {"username": "alice", "rating": 1000, "result": "agreed"},
                "black": {"username": "carol", "rating": 1010, "result": "agreed"},
            },
        ]}))
    };
    let app = Router::new()
        .route("/pub/player/{name}/games/archives", get(archives))
        .route("/pub/player/{name}/games/{year}/{month}", get(games));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/pub")
}

async fn review_state(user_agent: &str) -> AppState {
    let mut config = ReviewConfig {
        user_agent: user_agent.into(),
        ..ReviewConfig::default()
    };
    config.chesscom.base_url = fake_chesscom().await;
    config.chesscom.min_interval_ms = 0;
    config.analysis.engine = EngineKind::Native;
    config.analysis.workers = 2;
    config.native.eval_file = "does/not/exist.nnue".into();
    config.native.hash_mb = 1;
    let service = ReviewService::with_store(config, Store::in_memory().unwrap()).unwrap();
    AppState::default().with_review(Arc::new(service))
}

async fn call(state: &AppState, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = router(state.clone()).oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, body.to_vec())
}

async fn get_json<T: DeserializeOwned>(state: &AppState, uri: &str) -> (StatusCode, T) {
    let (status, body) = call(state, Request::get(uri).body(Body::empty()).unwrap()).await;
    let parsed = serde_json::from_slice(&body)
        .unwrap_or_else(|err| panic!("{err}: {}", String::from_utf8_lossy(&body)));
    (status, parsed)
}

async fn start(state: &AppState, body: Value) -> (StatusCode, Value) {
    let request = Request::post("/api/review/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, body) = call(state, request).await;
    (status, serde_json::from_slice(&body).unwrap())
}

/// The `progress` events of a job's stream, until it ends.
async fn events(state: &AppState, id: u64) -> Vec<ReviewJobView> {
    let request = Request::get(format!("/api/review/jobs/{id}/events"))
        .body(Body::empty())
        .unwrap();
    let response = router(state.clone()).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    text.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect()
}

#[tokio::test]
async fn reviews_a_player_and_streams_the_progress() {
    let state = review_state("chess-bot tests").await;
    let (status, started) = start(
        &state,
        json!({ "username": "alice", "maxGames": 5, "depth": 3, "multipv": 2 }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{started}");
    let started: ReviewStarted = serde_json::from_value(started).unwrap();

    let events = events(&state, started.job_id).await;
    let last = events.last().unwrap();
    assert_eq!(last.state, ReviewJobState::Done, "{:?}", last.error);
    assert_eq!(last.games, ["fools-mate", "london"]);
    assert_eq!(last.games_done, 2);
    assert_eq!(last.positions_done, 5 + 6);
    assert!(events.iter().all(|e| e.username == "alice"));

    let (status, progress): (_, ReviewJobView) =
        get_json(&state, &format!("/api/review/jobs/{}", started.job_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(&progress, last);

    // The games, from the cache.
    let (status, games): (_, Vec<ReviewGameSummary>) =
        get_json(&state, "/api/review/players/Alice/games?timeClasses=blitz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].result, "0-1");
    assert_eq!((games[0].plies, games[0].analysed), (4, true));

    let (status, view): (_, ReviewGameView) =
        get_json(&state, "/api/review/games/fools-mate").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view.moves[3].san, "Qh4#");
    assert_eq!(view.moves[2].clock_ms, Some(177_500));
    let analysis = view.analysis.unwrap();
    assert_eq!(analysis.engine, "chess-bot classic");
    assert_eq!(analysis.budget, "depth 3");
    assert_eq!(analysis.positions.len(), 5);
    // Before 2... Qh4#: Black mates in one, from White's point of view.
    let before_mate = &analysis.positions[3];
    assert_eq!(before_mate.score, ScoreView::Mate { moves: -1 });
    assert_eq!(before_mate.lines[0].san[0], "Qh4#");
    assert!(analysis.positions[4].checkmated);
}

#[tokio::test]
async fn bad_requests_and_unknown_resources() {
    let state = review_state("chess-bot tests").await;
    let (status, _) = start(&state, json!({ "username": "x" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = start(&state, json!({ "username": "alice", "since": "yesterday" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = start(&state, json!({ "username": "alice", "multipv": 9 })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _): (_, Value) = get_json(&state, "/api/review/jobs/999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _): (_, Value) = get_json(&state, "/api/review/games/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _): (_, Value) = get_json(
        &state,
        "/api/review/players/alice/games?timeClasses=classical",
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // No User-Agent: nothing is sent to Chess.com.
    let offline = review_state("").await;
    let (status, body) = start(&offline, json!({ "username": "alice" })).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("CHESS_REVIEW_USER_AGENT")
    );
    // Without the service at all.
    let (status, _) = start(&AppState::default(), json!({ "username": "alice" })).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn jobs_can_be_cancelled() {
    let state = review_state("chess-bot tests").await;
    let (_, started) = start(
        &state,
        json!({ "username": "alice", "movetimeMs": 1000, "multipv": 1 }),
    )
    .await;
    let id = started["jobId"].as_u64().unwrap();
    let request = Request::delete(format!("/api/review/jobs/{id}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = call(&state, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let last = events(&state, id).await.pop().unwrap();
    assert_eq!(last.state, ReviewJobState::Cancelled);
}
