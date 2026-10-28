//! `/internal/v1`: the [`Broker`] over HTTP, for workers in other
//! processes or containers. Authenticated by the workers' token
//! (`Authorization: Bearer`); the reverse proxy never exposes it.
//!
//! | Route | Broker call |
//! | --- | --- |
//! | `POST /internal/v1/claims` (`WorkerInfo`) | `claim`: 200 `Claim`, or 204 |
//! | `POST …/jobs/{job}/attempts/{n}/heartbeat` | `heartbeat` → `Directive` |
//! | `POST …/progress` (`Progress`) | `progress` → `Directive` |
//! | `POST …/events` (`{kind, data}`) | `event` → 204 |
//! | `POST …/network?superbatches&positions&finalLoss&device` (the file) | `complete_training` → `Model` |
//! | `POST …/dataset` | `complete_selfplay` → `Dataset` |
//! | `POST …/evaluation` (`EvaluationProgress`) | `complete_evaluation` → `Job` |
//! | `POST …/stopped` | `stopped` → 204 |
//! | `POST …/failure` (`{message}`) | `fail` → 204 |
//!
//! A claim that no longer holds answers 409 `lost-claim`.

use axum::body::{Body, to_bytes};
use axum::extract::{Path, Query, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use chess_jobs::{
    Broker, ClaimToken, Directive, EvaluationProgress, EventKind, Job, JobId, Progress,
    TrainingSummary, WorkerInfo,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::NnueApi;
use crate::auth::same_secret;
use crate::problem::{ApiProblem, ApiResult};
use crate::routes::{JsonBody, parse_id};

/// Largest network upload: the largest architecture's weights and a header.
const MAX_NETWORK: usize = chess_nnue_spec::MAX_WEIGHTS_BYTES as usize + (2 << 20);

const ATTEMPT: &str = "/internal/v1/jobs/{job}/attempts/{attempt}";

pub(crate) fn router(api: NnueApi) -> Router<NnueApi> {
    Router::new()
        .route("/internal/v1/claims", post(claim))
        .route(&format!("{ATTEMPT}/heartbeat"), post(heartbeat))
        .route(&format!("{ATTEMPT}/progress"), post(progress))
        .route(&format!("{ATTEMPT}/events"), post(event))
        .route(
            &format!("{ATTEMPT}/network"),
            post(network).layer(axum::extract::DefaultBodyLimit::max(MAX_NETWORK)),
        )
        .route(&format!("{ATTEMPT}/dataset"), post(dataset))
        .route(&format!("{ATTEMPT}/evaluation"), post(evaluation))
        .route(&format!("{ATTEMPT}/stopped"), post(stopped))
        .route(&format!("{ATTEMPT}/failure"), post(failure))
        .layer(from_fn_with_state(api, token))
}

/// Only requests with the workers' token get through.
async fn token(State(api): State<NnueApi>, request: Request, next: Next) -> Response {
    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match (&api.config.worker_token, presented) {
        (Some(expected), Some(presented)) if same_secret(expected, presented) => {
            next.run(request).await
        }
        _ => ApiProblem::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Worker token required",
        )
        .into_response(),
    }
}

/// A claim from the path.
fn claim_token(job: &str, attempt: u32) -> ApiResult<ClaimToken> {
    let job: JobId = parse_id(job, "job")?;
    Ok(ClaimToken { job, attempt })
}

async fn claim(
    State(api): State<NnueApi>,
    JsonBody(worker): JsonBody<WorkerInfo>,
) -> ApiResult<Response> {
    Ok(match api.lab.claim(&worker)? {
        Some(claim) => Json(claim).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

async fn heartbeat(
    State(api): State<NnueApi>,
    Path((job, attempt)): Path<(String, u32)>,
) -> ApiResult<Json<Directive>> {
    Ok(Json(api.lab.heartbeat(claim_token(&job, attempt)?)?))
}

async fn progress(
    State(api): State<NnueApi>,
    Path((job, attempt)): Path<(String, u32)>,
    JsonBody(progress): JsonBody<Progress>,
) -> ApiResult<Json<Directive>> {
    Ok(Json(
        api.lab.progress(claim_token(&job, attempt)?, &progress)?,
    ))
}

/// Body of an event report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EventReport {
    /// Kind of event.
    pub(crate) kind: EventKind,
    /// Its details.
    pub(crate) data: Value,
}

async fn event(
    State(api): State<NnueApi>,
    Path((job, attempt)): Path<(String, u32)>,
    JsonBody(report): JsonBody<EventReport>,
) -> ApiResult<StatusCode> {
    api.lab
        .event(claim_token(&job, attempt)?, report.kind, report.data)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn network(
    State(api): State<NnueApi>,
    Path((job, attempt)): Path<(String, u32)>,
    Query(summary): Query<TrainingSummary>,
    body: Body,
) -> ApiResult<Response> {
    let claim = claim_token(&job, attempt)?;
    let bytes = to_bytes(body, MAX_NETWORK)
        .await
        .map_err(|_| ApiProblem::too_large(MAX_NETWORK as u64))?;
    let lab = api.lab.clone();
    let model = crate::blocking(move || {
        lab.complete_training(claim, &bytes, &summary)
            .map_err(ApiProblem::from)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(model)).into_response())
}

async fn dataset(
    State(api): State<NnueApi>,
    Path((job, attempt)): Path<(String, u32)>,
) -> ApiResult<Response> {
    let claim = claim_token(&job, attempt)?;
    let lab = api.lab.clone();
    let dataset =
        crate::blocking(move || lab.complete_selfplay(claim).map_err(ApiProblem::from)).await?;
    Ok(Json(dataset).into_response())
}

async fn evaluation(
    State(api): State<NnueApi>,
    Path((job, attempt)): Path<(String, u32)>,
    JsonBody(result): JsonBody<EvaluationProgress>,
) -> ApiResult<Json<Job>> {
    Ok(Json(api.lab.complete_evaluation(
        claim_token(&job, attempt)?,
        &result,
    )?))
}

async fn stopped(
    State(api): State<NnueApi>,
    Path((job, attempt)): Path<(String, u32)>,
) -> ApiResult<StatusCode> {
    api.lab.stopped(claim_token(&job, attempt)?)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Body of a failure report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FailureReport {
    /// What went wrong.
    pub(crate) message: String,
}

async fn failure(
    State(api): State<NnueApi>,
    Path((job, attempt)): Path<(String, u32)>,
    JsonBody(report): JsonBody<FailureReport>,
) -> ApiResult<StatusCode> {
    api.lab.fail(claim_token(&job, attempt)?, &report.message)?;
    Ok(StatusCode::NO_CONTENT)
}
