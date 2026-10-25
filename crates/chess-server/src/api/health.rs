use axum::Json;

use super::types::HealthResponse;

/// Liveness probe, also used by the UI to display the server version.
#[utoipa::path(
    get,
    path = "/api/health",
    tag = "system",
    responses((status = 200, description = "Server is up", body = HealthResponse))
)]
pub(super) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}
