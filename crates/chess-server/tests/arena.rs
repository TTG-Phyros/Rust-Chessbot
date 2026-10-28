//! Networks, bots and tournaments: read from the data directory, commands
//! relayed to a fake control daemon.

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chess_server::AppState;
use chess_server::api::router;
use chess_server::api::types::{ArenaView, BotInfo, TournamentState, TrainingView};
use chess_server::engine::EngineService;
use serde_json::{Value, json};
use tower::ServiceExt;

fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chess-server-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in [
        "runs",
        "checkpoints/main",
        "weights/main",
        "bots",
        "tournaments",
        "metrics",
    ] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    dir
}

const PARAMS: &str = r#"{"selfplayNodes":5000,"learningRate":0.001,"lrDropEvery":40,"lrDropFactor":0.3,"wdl":0.25,"batchSize":16384,"batchesPerSuperbatch":200,"superbatches":0,"exportEvery":5,"minPositions":2000000,"maxReuse":30,"evalGames":64,"datasetsFrom":[]}"#;

/// Pretends to be `chess-train serve`: playing tournament `t2`, and
/// refusing what the real daemon would.
async fn fake_daemon() -> String {
    let app = Router::new()
        .route(
            "/status",
            get(|| async { r#"{"state":"idle","run":"main","error":null,"tournament":"t2","stockfish":"Stockfish 19"}"# }),
        )
        .route(
            "/networks/defaults",
            get(|| async { ([(header::CONTENT_TYPE, "application/json")], PARAMS) }),
        )
        .route(
            "/networks",
            post(|Json(body): Json<Value>| async move {
                if body["name"] == "main" {
                    (
                        StatusCode::CONFLICT,
                        Json(json!({ "error": "a network named main already exists" })),
                    )
                } else {
                    (StatusCode::CREATED, Json(Value::Null))
                }
            }),
        )
        .route(
            "/networks/{name}",
            delete(|| async { StatusCode::NO_CONTENT }),
        )
        .route(
            "/bots",
            post(|Json(body): Json<Value>| async move {
                (
                    StatusCode::CREATED,
                    Json(json!({
                        "name": body["name"], "network": body["network"],
                        "superbatch": body["superbatch"], "elo": null, "eloError": null,
                        "params": null, "createdAt": 7,
                    })),
                )
            }),
        )
        .route(
            "/bots/{name}",
            delete(|| async {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": "no bot named x" })),
                )
            }),
        )
        .route(
            "/tournaments",
            post(|| async { (StatusCode::ACCEPTED, Json(json!({ "id": "t3" }))) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

async fn call(state: &AppState, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if !body.is_null() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let body = if body.is_null() {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    let response = router(state.clone())
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let code = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (code, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn networks_bots_and_tournaments() {
    let dir = data_dir("arena");
    std::fs::write(dir.join("runs/main.json"), PARAMS).unwrap();
    std::fs::write(dir.join("runs/exp.json"), PARAMS).unwrap();
    std::fs::write(
        dir.join("checkpoints/main/state.json"),
        r#"{"superbatch":35,"step":7000,"positions":1}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("weights/main/best.json"),
        r#"[{"superbatch":35,"elo":100.0,"eloError":83.0,"games":64,"measuredAt":1}]"#,
    )
    .unwrap();
    std::fs::write(dir.join("weights/main.nnue"), b"").unwrap();
    std::fs::write(dir.join("bots/champion.nnue"), b"").unwrap();
    std::fs::write(
        dir.join("bots/champion.json"),
        r#"{"name":"champion","network":"main","superbatch":35,"elo":100.0,"eloError":83.0,"params":null,"createdAt":5}"#,
    )
    .unwrap();
    for (id, state) in [("t1", "running"), ("t2", "running")] {
        std::fs::write(
            dir.join(format!("tournaments/{id}.json")),
            format!(
                r#"{{"id":"{id}","state":"{state}","error":null,"createdAt":1,"updatedAt":1,"bots":["classic","champion"],"gamesPerPair":2,"limit":{{"kind":"time","value":100}},"pairings":[{{"a":"classic","b":"champion","wins":0,"draws":1,"losses":1,"elo":-191.0,"eloError":300.0}}],"standings":[]}}"#
            ),
        )
        .unwrap();
    }
    let state = AppState {
        engine: EngineService::new(1, 1, None),
        data_dir: dir.clone(),
        trainer_url: Some(fake_daemon().await),
        http: reqwest::Client::new(),
        review: None,
        nnue: None,
    };

    // Networks, from the files.
    let (code, view) = call(&state, "GET", "/api/training?network=exp", Value::Null).await;
    assert_eq!(code, StatusCode::OK);
    let view: TrainingView = serde_json::from_value(view).unwrap();
    assert_eq!(view.network.as_deref(), Some("exp"));
    let names: Vec<&str> = view.networks.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["exp", "main"]);
    let main = &view.networks[1];
    assert_eq!(main.superbatch, Some(35));
    assert_eq!(main.best.len(), 1);
    assert!(main.has_weights && main.params.is_some());
    assert!(!view.networks[0].has_weights);
    let (_, view) = call(&state, "GET", "/api/training", Value::Null).await;
    assert_eq!(view["network"], "main", "defaults to the daemon's network");

    // Network commands, relayed.
    let (code, defaults) = call(&state, "GET", "/api/networks/defaults", Value::Null).await;
    assert_eq!(
        (code, &defaults["selfplayNodes"]),
        (StatusCode::OK, &json!(5000))
    );
    let create = |name: &str| json!({ "name": name, "params": defaults.clone() });
    assert_eq!(
        call(&state, "POST", "/api/networks", create("exp2"))
            .await
            .0,
        StatusCode::CREATED
    );
    let (code, body) = call(&state, "POST", "/api/networks", create("main")).await;
    assert_eq!(code, StatusCode::CONFLICT);
    assert!(body["error"].as_str().unwrap().contains("already exists"));
    let (code, _) = call(&state, "POST", "/api/networks", create("../etc")).await;
    assert_eq!(code, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        call(&state, "DELETE", "/api/networks/exp", Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );

    // Arena, from the files.
    let (code, arena) = call(&state, "GET", "/api/arena", Value::Null).await;
    assert_eq!(code, StatusCode::OK);
    let arena: ArenaView = serde_json::from_value(arena).unwrap();
    assert_eq!(arena.bots.len(), 1);
    assert_eq!(arena.controller.tournament.as_deref(), Some("t2"));
    assert_eq!(arena.controller.stockfish.as_deref(), Some("Stockfish 19"));
    let states: Vec<_> = arena
        .tournaments
        .iter()
        .map(|t| (t.id.as_str(), t.state))
        .collect();
    assert!(states.contains(&("t2", TournamentState::Running)));
    assert!(
        states.contains(&("t1", TournamentState::Stopped)),
        "no longer played"
    );

    // Arena commands, relayed.
    let bot = json!({ "name": "sb35", "network": "main", "superbatch": 35 });
    let (code, info) = call(&state, "POST", "/api/bots", bot).await;
    assert_eq!(code, StatusCode::CREATED);
    let info: BotInfo = serde_json::from_value(info).unwrap();
    assert_eq!((info.name.as_str(), info.superbatch), ("sb35", Some(35)));
    let (code, body) = call(&state, "DELETE", "/api/bots/x", Value::Null).await;
    assert_eq!(
        (code, body["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("no bot named x"))
    );
    let request = json!({ "bots": ["classic", "champion"], "gamesPerPair": 10, "limit": { "kind": "nodes", "value": 2000 }, "threads": 0, "pairings": [{ "a": "champion", "b": "classic" }] });
    let (code, started) = call(&state, "POST", "/api/tournaments", request).await;
    assert_eq!((code, &started["id"]), (StatusCode::ACCEPTED, &json!("t3")));
}

#[tokio::test]
async fn arena_without_a_daemon() {
    let dir = data_dir("arena-alone");
    let state = AppState {
        engine: EngineService::new(1, 1, None),
        data_dir: dir,
        trainer_url: None,
        http: reqwest::Client::new(),
        review: None,
        nnue: None,
    };
    let (code, arena) = call(&state, "GET", "/api/arena", Value::Null).await;
    assert_eq!(code, StatusCode::OK);
    let arena: ArenaView = serde_json::from_value(arena).unwrap();
    assert!(arena.bots.is_empty() && arena.tournaments.is_empty());
    let request = json!({ "bots": ["classic", "x"], "gamesPerPair": 2, "limit": { "kind": "depth", "value": 4 }, "threads": 0 });
    assert_eq!(
        call(&state, "POST", "/api/tournaments", request).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
