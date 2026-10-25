//! Integration tests of the HTTP API, run against the router in memory.

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use chess_server::api::{router, types::HealthResponse};
use tower::ServiceExt;

async fn get(uri: &str) -> (StatusCode, Vec<u8>) {
    let request = Request::get(uri).body(Body::empty()).unwrap();
    let response = router().oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, body.to_vec())
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
async fn openapi_documents_health() {
    let (status, body) = get("/api/openapi.json").await;
    assert_eq!(status, StatusCode::OK);
    let doc: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(doc["paths"]["/api/health"]["get"].is_object());
}

#[tokio::test]
async fn unknown_route_is_404() {
    let (status, _) = get("/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
