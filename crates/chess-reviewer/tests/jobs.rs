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
    EngineKind, GameJobRequest, JobProgress, JobRequest, JobState, ReviewConfig, ReviewService,
    Store,
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
    assert_eq!(other.eta_secs, None, "no time left once done");
}

#[tokio::test]
async fn estimates_before_and_after_reviewing() {
    let service = service().await;
    let deeper = SearchSettings {
        budget: Budget::Depth(5),
        ..SHALLOW
    };
    let filter = GameFilter::default();
    let before = service
        .estimate("alice", &filter, None, &[SHALLOW, deeper])
        .await
        .unwrap();
    assert_eq!(before.len(), 2);
    for estimate in &before {
        assert_eq!((estimate.pending_games, estimate.cached_games), (3, 0));
        assert_eq!(estimate.positions, 23);
        // Never used: measured on the benchmark.
        assert!(!estimate.measured);
        assert!(estimate.seconds_per_position > 0.0);
    }
    assert!(before[1].seconds_per_position > before[0].seconds_per_position);

    // The review measures the real speed, and caches the games.
    let progress = wait(&service.start(request(SHALLOW)).unwrap()).await;
    assert_eq!(progress.state, JobState::Done);
    let after = service
        .estimate("alice", &filter, None, &[SHALLOW])
        .await
        .unwrap();
    assert_eq!((after[0].pending_games, after[0].cached_games), (0, 3));
    assert_eq!((after[0].positions, after[0].seconds), (0, 0));
    assert!(after[0].measured);

    let mut bad = filter.clone();
    bad.max_games = 0;
    assert!(
        service
            .estimate("alice", &bad, None, &[SHALLOW])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn reports_the_time_left() {
    let service = service().await;
    let slow = SearchSettings {
        budget: Budget::MoveTimeMs(100),
        multipv: 1,
    };
    let job = service.start(request(slow)).unwrap();
    let mut updates = job.subscribe();
    // After 20 of the 23 positions (100 ms each, 2 workers): the measured speed.
    let progress = updates
        .wait_for(|p| p.positions_done >= 20 || p.state.is_final())
        .await
        .unwrap()
        .clone();
    if progress.state.is_final() {
        // Too fast to be observed while analysing (a very loaded machine).
        return;
    }
    let eta = progress.eta_secs.expect("time left while analysing");
    let left = progress.positions_total - progress.positions_done;
    // About 50 ms per position on 2 workers, rounded up.
    assert!(
        eta as f64 <= left as f64 * 0.1 + 1.0,
        "{eta} s for {left} positions"
    );
    job.cancel();
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

#[tokio::test]
async fn reviews_a_single_game_deeper() {
    let service = service().await;
    // Brings the games into the cache, analysed shallowly.
    let progress = wait(&service.start(request(SHALLOW)).unwrap()).await;
    assert_eq!(progress.state, JobState::Done, "{:?}", progress.error);

    let deeper = SearchSettings {
        budget: Budget::Depth(5),
        multipv: 3,
    };
    let game = |settings| GameJobRequest {
        game_id: "game-1".into(),
        engine: None,
        settings: Some(settings),
    };
    let job = service.start_game(game(deeper)).unwrap();
    assert!(job.username.is_empty());
    let progress = wait(&job).await;
    assert_eq!(progress.state, JobState::Done, "{:?}", progress.error);
    assert_eq!(progress.games, ["game-1"]);
    assert_eq!((progress.games_done, progress.games_cached), (1, 0));
    assert_eq!((progress.positions_done, progress.positions_total), (9, 9));

    // Its page shows the deeper analysis, even after a later shallow one.
    let shown = service.deepest_analysis("game-1").unwrap().unwrap();
    assert_eq!(shown.settings, deeper);
    assert_eq!(shown.positions.len(), 9);
    assert_eq!(shown.positions[0].lines.len(), 3);
    let fast = SearchSettings {
        budget: Budget::Depth(2),
        ..SHALLOW
    };
    wait(&service.start(request(fast)).unwrap()).await;
    assert_eq!(
        service
            .deepest_analysis("game-1")
            .unwrap()
            .unwrap()
            .settings,
        deeper
    );

    // Done already: nothing to analyse.
    let again = wait(&service.start_game(game(deeper)).unwrap()).await;
    assert_eq!((again.games_done, again.games_cached), (0, 1));

    let mut unknown = game(deeper);
    unknown.game_id = "nope".into();
    assert!(service.start_game(unknown).is_err());
    let no_lines = SearchSettings {
        multipv: 0,
        ..deeper
    };
    assert!(service.start_game(game(no_lines)).is_err());
}
