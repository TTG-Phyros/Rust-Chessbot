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
use chess_reviewer::{EngineKind, Preset, ReviewConfig, ReviewService, Store};
use chess_server::AppState;
use chess_server::api::router;
use chess_server::api::types::ScoreView;
use chess_server::api::types::review::{
    ReviewClass, ReviewEstimates, ReviewGameSummary, ReviewGameView, ReviewJobState, ReviewJobView,
    ReviewOptions, ReviewProfile, ReviewStarted,
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
    // Presets shallow enough for our engine in a test (the real ones go to
    // depth 16, and an estimate times each of them).
    for (preset, depth) in config.analysis.presets.iter_mut().zip([1, 2, 3]) {
        *preset = Preset {
            depth: Some(depth),
            ..preset.clone()
        };
    }
    config.analysis.game.depth = Some(4);
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
    let opening = view.game.opening.as_ref().unwrap();
    assert_eq!(
        (opening.eco.as_str(), opening.name.as_str()),
        ("A00", "Barnes Opening: Fool's Mate")
    );
    assert_eq!(
        (opening.family.as_str(), opening.variation.as_deref()),
        ("Barnes Opening", Some("Fool's Mate"))
    );
    assert_eq!((opening.ply, opening.book_plies), (4, 4));
    assert!(view.moves.iter().all(|m| m.book));
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
    // White's chances are gone once Black mates in one.
    assert!(analysis.positions[3].win.abs() < 1e-9);
    assert!(analysis.positions[4].win.abs() < 1e-9);
    // Every move is theory, but 2. g4?? still costs White the game.
    assert_eq!(analysis.moves.len(), 4);
    assert!(analysis.moves.iter().all(|m| m.class == ReviewClass::Book));
    assert!(analysis.moves[2].win_after.abs() < 1e-9);
    assert!(analysis.moves[2].accuracy < 10.0);
    assert_eq!(analysis.white.moves, 2);
    assert_eq!(analysis.white.counts.book, 2);
    assert!(analysis.white.accuracy.unwrap() < analysis.black.accuracy.unwrap());
    assert!(analysis.white.acpl.unwrap() > 400.0);

    // Alice's profile over both games.
    let (status, profile): (_, ReviewProfile) =
        get_json(&state, "/api/review/players/alice/profile").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (profile.games, profile.unanalysed, profile.skipped),
        (2, 0, 0)
    );
    assert_eq!(
        (
            profile.record.wins,
            profile.record.draws,
            profile.record.losses
        ),
        (0, 1, 1)
    );
    assert_eq!(profile.rating, Some(900));
    let families: Vec<&str> = profile
        .repertoire
        .white
        .iter()
        .map(|opening| opening.family.as_str())
        .collect();
    assert_eq!(families.len(), 2);
    assert!(families.contains(&"Barnes Opening"), "{families:?}");
    assert!(profile.repertoire.black.is_empty());
    assert_eq!(profile.phases.len(), 3);
    assert_eq!(profile.per_game.len(), 2);
    assert_eq!(profile.moves, 2 + 3);
    // Only the blitz game has clocks.
    assert_eq!(profile.time.map(|time| time.games), Some(1));
    let (status, _): (_, Value) =
        get_json(&state, "/api/review/players/alice/profile?since=2026-13-01").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn presets_and_estimates() {
    let state = review_state("chess-bot tests").await;
    let (status, options): (_, ReviewOptions) = get_json(&state, "/api/review/options").await;
    assert_eq!(status, StatusCode::OK);
    assert!(options.chesscom_enabled);
    let names: Vec<&str> = options.presets.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["fast", "balanced", "deep"]);
    assert_eq!(options.default_preset, "deep");
    assert_eq!(options.presets[0].depth, Some(1));
    assert_eq!(options.workers, 2);

    // Shallow custom settings: the presets, then them.
    let request = Request::post("/api/review/estimate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "alice", "depth": 2, "multipv": 1 }).to_string(),
        ))
        .unwrap();
    let (status, body) = call(&state, request).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let estimates: ReviewEstimates = serde_json::from_slice(&body).unwrap();
    assert_eq!(estimates.games, 2);
    let labels: Vec<&str> = estimates.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(
        labels,
        ["Rapide", "Équilibré", "Approfondi", "Personnalisé"]
    );
    let custom = &estimates.options[3];
    assert_eq!((custom.preset.as_deref(), custom.depth), (None, Some(2)));
    assert_eq!((custom.pending_games, custom.positions), (2, 11));
    assert!(!custom.measured);

    // A preset by name; after the review, its estimate is measured and cached.
    let (status, started) = start(&state, json!({ "username": "alice", "preset": "fast" })).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let id = started["jobId"].as_u64().unwrap();
    let last = events(&state, id).await.pop().unwrap();
    assert_eq!(last.state, ReviewJobState::Done);
    assert_eq!(last.eta_seconds, None);
    let request = Request::post("/api/review/estimate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "alice", "preset": "fast" }).to_string(),
        ))
        .unwrap();
    let (_, body) = call(&state, request).await;
    let estimates: ReviewEstimates = serde_json::from_slice(&body).unwrap();
    assert_eq!(estimates.options.len(), 3, "the preset is not repeated");
    let fast = &estimates.options[0];
    assert_eq!(
        (fast.cached_games, fast.pending_games, fast.seconds),
        (2, 0, 0)
    );
    assert!(fast.measured);

    let (status, body) = start(&state, json!({ "username": "alice", "preset": "turbo" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"].as_str().unwrap().contains("turbo"));
}

async fn analyse(state: &AppState, game: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::post(format!("/api/review/games/{game}/analyse"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, body) = call(state, request).await;
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn reviews_one_game_deeper() {
    let state = review_state("chess-bot tests").await;
    let (_, options): (_, ReviewOptions) = get_json(&state, "/api/review/options").await;
    assert_eq!(options.game_preset.name, "game");
    assert_eq!(
        (options.game_preset.depth, options.game_preset.multipv),
        (Some(4), 3)
    );

    // Not in the cache yet.
    let (status, _) = analyse(&state, "london", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, started) = start(&state, json!({ "username": "alice", "preset": "fast" })).await;
    let id = started["jobId"].as_u64().unwrap();
    assert_eq!(
        events(&state, id).await.pop().unwrap().state,
        ReviewJobState::Done
    );

    let (status, started) = analyse(&state, "london", json!({})).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{started}");
    let id = started["jobId"].as_u64().unwrap();
    let last = events(&state, id).await.pop().unwrap();
    assert_eq!(last.state, ReviewJobState::Done, "{:?}", last.error);
    assert_eq!(last.games, ["london"]);
    assert_eq!((last.games_done, last.positions_done), (1, 6));

    // The game shows its deeper analysis.
    let (_, view): (_, ReviewGameView) = get_json(&state, "/api/review/games/london").await;
    let analysis = view.analysis.unwrap();
    assert_eq!((analysis.budget.as_str(), analysis.multipv), ("depth 4", 3));
    assert_eq!(analysis.moves.len(), 5);

    let (status, body) = analyse(&state, "london", json!({ "preset": "turbo" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"].as_str().unwrap().contains("turbo"));
    let (status, _) = analyse(&state, "london", json!({ "multipv": 9 })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
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
