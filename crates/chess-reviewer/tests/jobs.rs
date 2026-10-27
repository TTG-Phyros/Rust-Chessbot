//! Review jobs end to end: a local stand-in for the PubAPI, our own engine
//! (no external binary needed), an in-memory cache.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::extract::Path;
use axum::routing::get;
use axum::{Json, Router};
use chess_chesscom::{GameFilter, Month};
use chess_review::{Budget, SearchSettings};
use chess_reviewer::{
    EngineKind, JobProgress, JobRequest, JobState, ReviewConfig, ReviewService, Store,
};
use serde_json::{Value, json};

const GAMES: [&str; 3] = [
    "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 1/2-1/2",
    "1. d4 d5 2. Bf4 Nf6 3. e3 e6 4. Nf3 c5 1-0",
    "1. f3 e5 2. g4 Qh4# 0-1",
];

async fn serve() -> String {
    let month = Month::containing(chess_reviewer::now());
    let archives = move || async move {
        Json(json!({ "archives": [
            format!("https://api.chess.com/pub/player/alice/games/{:04}/{:02}", month.year, month.month)
        ]}))
    };
    let games = move |Path((_, _, _)): Path<(String, i32, u8)>| async move {
        let games: Vec<Value> = GAMES
            .iter()
            .zip(1..)
            .map(|(pgn, i)| {
                json!({
                    "url": format!("https://www.chess.com/game/live/{i}"),
                    "uuid": format!("game-{i}"),
                    "pgn": pgn, "time_control": "180", "end_time": month.start() + i,
                    "rated": true, "rules": "chess", "time_class": "blitz",
                    "white": {"username": "alice", "rating": 1500, "result": "win"},
                    "black": {"username": "bob", "rating": 1480, "result": "resigned"},
                })
            })
            .collect();
        Json(json!({ "games": games }))
    };
    let app = Router::new()
        .route("/pub/player/{name}/games/archives", get(archives))
        .route("/pub/player/{name}/games/{year}/{month}", get(games));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/pub")
}

async fn service() -> Arc<ReviewService> {
    let mut config = ReviewConfig {
        user_agent: "chess-bot tests".into(),
        ..ReviewConfig::default()
    };
    config.chesscom.base_url = serve().await;
    config.chesscom.min_interval_ms = 0;
    config.analysis.engine = EngineKind::Native;
    config.analysis.workers = 2;
    config.native.eval_file = "does/not/exist.nnue".into();
    config.native.hash_mb = 1;
    Arc::new(ReviewService::with_store(config, Store::in_memory().unwrap()).unwrap())
}

fn request(settings: SearchSettings) -> JobRequest {
    JobRequest {
        username: "alice".into(),
        filter: GameFilter::default(),
        engine: None,
        settings: Some(settings),
    }
}

async fn wait(job: &chess_reviewer::Job) -> JobProgress {
    let mut updates = job.subscribe();
    let progress = updates
        .wait_for(|progress| progress.state.is_final())
        .await
        .unwrap();
    progress.clone()
}

const SHALLOW: SearchSettings = SearchSettings {
    budget: Budget::Depth(3),
    multipv: 2,
};

#[tokio::test]
async fn analyses_every_game_once() {
    let service = service().await;
    let progress = wait(&service.start(request(SHALLOW)).unwrap()).await;
    assert_eq!(progress.state, JobState::Done, "{:?}", progress.error);
    assert_eq!(progress.games, ["game-3", "game-2", "game-1"]);
    assert_eq!((progress.games_done, progress.games_cached), (3, 0));
    // 9 + 9 + 5 positions.
    assert_eq!(progress.positions_total, 23);
    assert_eq!(progress.positions_done, 23);
    assert_eq!(progress.engine.as_deref(), Some("chess-bot classic"));

    let analysis = service.store().latest_analysis("game-3").unwrap().unwrap();
    assert_eq!(analysis.positions.len(), 5);
    assert!(analysis.positions[4].checkmated);
    assert_eq!(analysis.positions[0].lines.len(), 2);

    // Same settings: everything comes from the cache.
    let again = wait(&service.start(request(SHALLOW)).unwrap()).await;
    assert_eq!((again.games_done, again.games_cached), (0, 3));
    assert_eq!(again.positions_total, 0);

    // Other settings: analysed again.
    let deeper = SearchSettings {
        budget: Budget::Depth(4),
        ..SHALLOW
    };
    let other = wait(&service.start(request(deeper)).unwrap()).await;
    assert_eq!((other.games_done, other.games_cached), (3, 0));
}

#[tokio::test]
async fn rejects_bad_requests() {
    let service = service().await;
    let mut bad = request(SHALLOW);
    bad.username = "../etc".into();
    assert!(service.start(bad).is_err());
    let mut too_many = request(SHALLOW);
    too_many.filter.max_games = 10_000;
    assert!(service.start(too_many).is_err());
    let no_lines = SearchSettings {
        multipv: 0,
        ..SHALLOW
    };
    assert!(service.start(request(no_lines)).is_err());

    let config = ReviewConfig {
        user_agent: String::new(),
        ..ReviewConfig::default()
    };
    let offline = Arc::new(ReviewService::with_store(config, Store::in_memory().unwrap()).unwrap());
    assert!(!offline.chesscom_enabled());
    assert!(offline.start(request(SHALLOW)).is_err());
}

#[tokio::test]
async fn jobs_can_be_cancelled() {
    let service = service().await;
    let slow = SearchSettings {
        budget: Budget::MoveTimeMs(500),
        multipv: 1,
    };
    let job = service.start(request(slow)).unwrap();
    let id = job.id;
    let mut updates = job.subscribe();
    updates.wait_for(|p| p.positions_done > 0).await.unwrap();
    service.job(id).unwrap().cancel();
    let progress = wait(&job).await;
    assert_eq!(progress.state, JobState::Cancelled);
    assert!(progress.positions_done < progress.positions_total);
    assert!(service.job(id).is_some(), "finished jobs stay visible");
}
