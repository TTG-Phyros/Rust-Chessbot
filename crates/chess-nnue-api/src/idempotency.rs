//! `Idempotency-Key` on `POST` requests: a retry with the same key and
//! content gets the first response again instead of creating twice; the
//! same key with another content is refused. Keys are kept 24 hours, per
//! API key.

use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chess_jobs::Idempotency;
use sha2::{Digest, Sha256};

use crate::NnueApi;
use crate::auth::Principal;
use crate::problem::ApiProblem;

pub(crate) const IDEMPOTENCY_KEY: &str = "idempotency-key";
/// Set on replayed responses.
pub(crate) const REPLAYED: &str = "idempotent-replayed";
/// Largest JSON body read to fingerprint a request, and largest response
/// stored (uploads are fingerprinted by their size, not read twice).
const MAX_BUFFERED: usize = 4 << 20;

pub(crate) async fn layer(State(api): State<NnueApi>, request: Request, next: Next) -> Response {
    if request.method() != Method::POST {
        return next.run(request).await;
    }
    let Some(key) = request.headers().get(IDEMPOTENCY_KEY) else {
        return next.run(request).await;
    };
    let key = match key.to_str() {
        Ok(key) if (1..=255).contains(&key.len()) && key.bytes().all(|b| b.is_ascii_graphic()) => {
            key.to_owned()
        }
        _ => {
            return ApiProblem::bad_request(
                "Idempotency-Key must be 1 to 255 visible ASCII characters",
            )
            .into_response();
        }
    };
    let principal = request
        .extensions()
        .get::<Principal>()
        .map_or("anonymous", |p| p.0.as_str())
        .to_owned();
    let scope = format!("{principal} POST {}", request.uri().path());

    // Fingerprint: query, media type and body (its length only for uploads).
    let (parts, body) = request.into_parts();
    let content_type = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let mut sha = Sha256::new();
    sha.update(parts.uri.query().unwrap_or("").as_bytes());
    sha.update(b"\n");
    sha.update(content_type.as_bytes());
    sha.update(b"\n");
    let body = if content_type.starts_with("application/octet-stream") {
        let length = parts
            .headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        sha.update(length.as_bytes());
        body
    } else {
        match to_bytes(body, MAX_BUFFERED).await {
            Ok(bytes) => {
                sha.update(&bytes);
                Body::from(bytes)
            }
            Err(_) => return ApiProblem::too_large(MAX_BUFFERED as u64).into_response(),
        }
    };
    let fingerprint: String = sha.finalize().iter().map(|b| format!("{b:02x}")).collect();

    match api.lab.idempotency_begin(&scope, &key, &fingerprint) {
        Err(err) => ApiProblem::from(err).into_response(),
        Ok(Idempotency::Mismatch) => ApiProblem::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "idempotency-key-reused",
            "Idempotency-Key already used for another request",
        )
        .detail("send a new key for a different request")
        .into_response(),
        Ok(Idempotency::InProgress) => ApiProblem::new(
            StatusCode::CONFLICT,
            "idempotency-in-progress",
            "A request with this Idempotency-Key is still running",
        )
        .detail("retry once it has finished")
        .into_response(),
        Ok(Idempotency::Replay {
            status,
            content_type,
            body,
        }) => {
            let status = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
            let mut response = (status, body).into_response();
            if let Some(value) = content_type.and_then(|ct| HeaderValue::from_str(&ct).ok()) {
                response.headers_mut().insert(header::CONTENT_TYPE, value);
            }
            response
                .headers_mut()
                .insert(REPLAYED, HeaderValue::from_static("true"));
            response
        }
        Ok(Idempotency::New) => {
            let response = next.run(Request::from_parts(parts, body)).await;
            let status = response.status();
            // Failures worth retrying (server errors, throttling) free the key.
            if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                if let Err(err) = api.lab.idempotency_abandon(&scope, &key) {
                    tracing::warn!(error = %err, "cannot release an idempotency key");
                }
                return response;
            }
            let (parts, body) = response.into_parts();
            let Ok(bytes) = to_bytes(body, MAX_BUFFERED).await else {
                let _ = api.lab.idempotency_abandon(&scope, &key);
                return ApiProblem::internal("response too large to store").into_response();
            };
            let content_type = parts
                .headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok());
            if let Err(err) =
                api.lab
                    .idempotency_finish(&scope, &key, status.as_u16(), content_type, &bytes)
            {
                tracing::warn!(error = %err, "cannot store an idempotent response");
            }
            Response::from_parts(parts, Body::from(bytes))
        }
    }
}
