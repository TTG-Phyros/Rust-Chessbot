//! Integration tests of the HTTP API, run against the router in memory.

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use chess_server::AppState;
use chess_server::api::router;
use chess_server::api::types::{
    EngineMoveResponse, ErrorResponse, GameStatus, HealthResponse, PgnExportResponse,
    PgnImportResponse, PositionView, ScoreView, Side,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn call(request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = router(AppState::default()).oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, body.to_vec())
}

async fn get(uri: &str) -> (StatusCode, Vec<u8>) {
    call(Request::get(uri).body(Body::empty()).unwrap()).await
}

async fn post<T: DeserializeOwned>(uri: &str, body: Value) -> (StatusCode, T) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, bytes) = call(request).await;
    let parsed = serde_json::from_slice(&bytes)
        .unwrap_or_else(|err| panic!("{err}: {}", String::from_utf8_lossy(&bytes)));
    (status, parsed)
}

#[tokio::test]
async fn health_returns_version() {
    let (status, body) = get("/api/health").await;
    assert_eq!(status, StatusCode::OK);
    let health: HealthResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(health.status, "ok");
    assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn openapi_documents_every_route() {
    let (status, body) = get("/api/openapi.json").await;
    assert_eq!(status, StatusCode::OK);
    let doc: Value = serde_json::from_slice(&body).unwrap();
    assert!(doc["paths"]["/api/health"]["get"].is_object());
    for path in [
        "/api/position",
        "/api/engine/move",
        "/api/pgn/import",
        "/api/pgn/export",
    ] {
        assert!(doc["paths"][path]["post"].is_object(), "{path}");
    }
    assert!(doc["components"]["schemas"]["PositionView"].is_object());
}

#[tokio::test]
async fn unknown_route_is_404() {
    let (status, _) = get("/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn startpos_has_twenty_legal_moves() {
    let (status, view): (_, PositionView) = post("/api/position", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view.legal_moves.len(), 20);
    assert_eq!(view.side_to_move, Side::White);
    assert_eq!(view.status, GameStatus::Ongoing);
    assert!(view.last_move.is_none());
    let e4 = view.legal_moves.iter().find(|m| m.uci == "e2e4").unwrap();
    assert_eq!(
        (e4.san.as_str(), e4.from.as_str(), e4.to.as_str()),
        ("e4", "e2", "e4")
    );
}

#[tokio::test]
async fn replays_moves_from_a_fen() {
    let body = json!({ "fen": "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1", "moves": ["e2e4", "e8d7"] });
    let (status, view): (_, PositionView) = post("/api/position", body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view.fen, "8/3k4/8/8/4P3/8/8/4K3 w - - 1 2");
    assert_eq!(view.history, ["e4", "Kd7"]);
    assert_eq!(view.last_move.unwrap().uci, "e8d7");
}

#[tokio::test]
async fn illegal_moves_and_bad_fens_are_422() {
    let (status, error): (_, ErrorResponse) =
        post("/api/position", json!({ "moves": ["e2e4", "e7e4"] })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(error.error.starts_with("move 2 (e7e4)"), "{}", error.error);

    let (status, error): (_, ErrorResponse) = post("/api/position", json!({ "fen": "x" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(error.error.contains("FEN"));
}

#[tokio::test]
async fn malformed_json_is_rejected() {
    let request = Request::post("/api/position")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"moves\": 3}"))
        .unwrap();
    let (status, _) = call(request).await;
    assert!(status.is_client_error());
}

#[tokio::test]
async fn engine_finds_the_mate() {
    let body = json!({
        "game": { "fen": "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1" },
        "limits": { "depth": 4 }
    });
    let (status, answer): (_, EngineMoveResponse) = post("/api/engine/move", body).await;
    assert_eq!(status, StatusCode::OK);
    let best = answer.best_move.unwrap();
    assert_eq!((best.uci.as_str(), best.san.as_str()), ("d1d8", "Rd8#"));
    assert_eq!(answer.score, ScoreView::Mate { moves: 1 });
    assert_eq!(answer.pv.first().map(String::as_str), Some("Rd8#"));
}

#[tokio::test]
async fn engine_score_is_from_whites_point_of_view() {
    // Black to move and getting mated: the score is still positive (White mates).
    let body =
        json!({ "game": { "fen": "7k/8/6KQ/8/8/8/8/8 b - - 0 1" }, "limits": { "depth": 5 } });
    let (_, answer): (_, EngineMoveResponse) = post("/api/engine/move", body).await;
    assert_eq!(answer.score, ScoreView::Mate { moves: 1 });
}

#[tokio::test]
async fn engine_has_no_move_when_the_game_is_over() {
    let mated = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
    let (status, answer): (_, EngineMoveResponse) =
        post("/api/engine/move", json!({ "game": { "fen": mated } })).await;
    assert_eq!(status, StatusCode::OK);
    assert!(answer.best_move.is_none());
}

#[tokio::test]
async fn pgn_round_trip() {
    let pgn = "[White \"Morphy\"]\n\n1. e4 e5 2. Nf3 d6 1-0";
    let (status, imported): (_, PgnImportResponse) =
        post("/api/pgn/import", json!({ "pgn": pgn })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(imported.game.moves, ["e2e4", "e7e5", "g1f3", "d7d6"]);
    assert_eq!(imported.game.fen, None);
    assert_eq!(imported.result, "1-0");
    assert_eq!(imported.tags[0].name, "White");

    let body = json!({ "game": imported.game, "tags": imported.tags, "result": imported.result });
    let (status, exported): (_, PgnExportResponse) = post("/api/pgn/export", body).await;
    assert_eq!(status, StatusCode::OK);
    assert!(exported.pgn.contains("[White \"Morphy\"]"));
    assert!(exported.pgn.contains("1. e4 e5 2. Nf3 d6 1-0"));
}

#[tokio::test]
async fn pgn_export_derives_the_result() {
    let moves = ["f2f3", "e7e5", "g2g4", "d8h4"];
    let (_, exported): (_, PgnExportResponse) =
        post("/api/pgn/export", json!({ "game": { "moves": moves } })).await;
    assert!(
        exported.pgn.contains("[Result \"0-1\"]"),
        "{}",
        exported.pgn
    );
    assert!(exported.pgn.contains("2. g4 Qh4# 0-1"));
}

#[tokio::test]
async fn pgn_errors_are_422() {
    let (status, _): (_, ErrorResponse) = post("/api/pgn/import", json!({ "pgn": "1. e5" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _): (_, ErrorResponse) = post("/api/pgn/import", json!({ "pgn": "" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _): (_, ErrorResponse) =
        post("/api/pgn/export", json!({ "game": {}, "result": "2-0" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
