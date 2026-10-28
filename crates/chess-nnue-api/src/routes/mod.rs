//! The `/api/v1` routes.

pub(crate) mod architectures;
pub(crate) mod datasets;
pub(crate) mod models;
pub(crate) mod runs;

use std::str::FromStr;

use axum::Router;
use axum::body::{Bytes, to_bytes};
use axum::extract::{FromRequest, Request};
use axum::http::header;
use axum::routing::{get, post};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::NnueApi;
use crate::problem::{ApiProblem, FieldError};

/// Largest JSON or YAML body.
const MAX_DOCUMENT: usize = 1 << 20;

pub(crate) fn router() -> Router<NnueApi> {
    Router::new()
        .route(
            "/api/v1/architectures",
            get(architectures::list).post(architectures::create),
        )
        .route(
            "/api/v1/architectures:validate",
            post(architectures::validate),
        )
        .route("/api/v1/architectures/presets", get(architectures::presets))
        .route("/api/v1/architectures/schema", get(architectures::schema))
        .route(
            "/api/v1/architectures/{id}",
            get(architectures::get).delete(architectures::delete),
        )
        .route(
            "/api/v1/datasets",
            get(datasets::list)
                .post(datasets::upload)
                .layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route("/api/v1/datasets:selfplay", post(datasets::selfplay))
        .route(
            "/api/v1/datasets/{id}",
            get(datasets::get).delete(datasets::delete),
        )
        .route("/api/v1/datasets/{id}/stats", get(datasets::stats))
        .route(
            "/api/v1/training-runs",
            get(runs::list_training).post(runs::create_training),
        )
        .route(
            "/api/v1/training-runs/{id}",
            get(runs::get_training).post(runs::act_training),
        )
        .route("/api/v1/training-runs/{id}/metrics", get(runs::metrics))
        .route(
            "/api/v1/training-runs/{id}/events",
            get(runs::events_training),
        )
        .route("/api/v1/selfplay-runs", get(runs::list_selfplay))
        .route(
            "/api/v1/selfplay-runs/{id}",
            get(runs::get_selfplay).post(runs::act_selfplay),
        )
        .route(
            "/api/v1/selfplay-runs/{id}/events",
            get(runs::events_selfplay),
        )
        .route("/api/v1/models", get(models::list))
        .route("/api/v1/models:deactivate", post(models::deactivate))
        .route(
            "/api/v1/models/{id}",
            get(models::get).post(models::act).delete(models::delete),
        )
        .route("/api/v1/models/{id}/file", get(models::file))
}

/// An identifier from the path: a malformed one names nothing (404).
pub(crate) fn parse_id<T: FromStr>(text: &str, what: &str) -> Result<T, ApiProblem> {
    text.parse()
        .map_err(|_| ApiProblem::not_found(format!("{what} {text} not found")))
}

/// `{id}:{method}` path segments: the id and the method.
pub(crate) fn custom_method(segment: &str) -> Result<(&str, &str), ApiProblem> {
    segment
        .split_once(':')
        .ok_or_else(|| ApiProblem::not_found(format!("no route for POST {segment}")))
}

/// Reads a body of at most [`MAX_DOCUMENT`] bytes.
async fn read_body(request: Request) -> Result<(Option<String>, Bytes), ApiProblem> {
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| {
            ct.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });
    let bytes = to_bytes(request.into_body(), MAX_DOCUMENT)
        .await
        .map_err(|_| ApiProblem::too_large(MAX_DOCUMENT as u64))?;
    Ok((content_type, bytes))
}

/// A JSON body, with errors as problem details: 400 when it is not JSON,
/// 422 when it does not have the expected shape.
pub(crate) struct JsonBody<T>(pub(crate) T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for JsonBody<T> {
    type Rejection = ApiProblem;

    async fn from_request(request: Request, _: &S) -> Result<Self, Self::Rejection> {
        let (content_type, bytes) = read_body(request).await?;
        if content_type.as_deref() != Some("application/json") {
            return Err(ApiProblem::unsupported_media_type(
                "send the body as application/json",
            ));
        }
        decode_json(&bytes).map(JsonBody)
    }
}

pub(crate) fn decode_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiProblem> {
    serde_json::from_slice(bytes).map_err(|err| {
        if err.is_data() {
            ApiProblem::validation(vec![FieldError {
                path: String::new(),
                message: err.to_string(),
            }])
        } else {
            ApiProblem::bad_request(format!("invalid JSON: {err}"))
        }
    })
}

/// An architecture document, in JSON or YAML (`application/yaml`).
pub(crate) struct Document(pub(crate) Value);

impl<S: Send + Sync> FromRequest<S> for Document {
    type Rejection = ApiProblem;

    async fn from_request(request: Request, _: &S) -> Result<Self, Self::Rejection> {
        let (content_type, bytes) = read_body(request).await?;
        match content_type.as_deref() {
            Some("application/json") => decode_json(&bytes).map(Document),
            Some("application/yaml" | "application/x-yaml" | "text/yaml") => {
                serde_norway::from_slice(&bytes)
                    .map(Document)
                    .map_err(|err| ApiProblem::bad_request(format!("invalid YAML: {err}")))
            }
            _ => Err(ApiProblem::unsupported_media_type(
                "send the architecture as application/json or application/yaml",
            )),
        }
    }
}
