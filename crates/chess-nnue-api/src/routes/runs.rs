//! `/api/v1/training-runs` and `/api/v1/selfplay-runs`: the lab's jobs
//! (and what `/api/v1/evaluations` shares with them).

use std::collections::VecDeque;
use std::convert::Infallible;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderName, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use chess_jobs::{
    EventKind, Job, JobEvent, JobId, JobKind, Progress, TrainingProgress, TrainingRequest,
};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::architectures::created;
use super::{JsonBody, custom_method, parse_id};
use crate::NnueApi;
use crate::pagination::{JobPage, PageQuery, next_cursor};
use crate::problem::{ApiProblem, ApiResult, Problem};

/// Pause between two looks at the event log of a running job.
const EVENT_POLL: Duration = Duration::from_millis(500);

/// One point of a training curve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetricPoint {
    /// When it was reported (Unix milliseconds).
    pub(crate) time: i64,
    /// The progress at that time.
    #[serde(flatten)]
    pub(crate) progress: TrainingProgress,
}

/// The training curve of a run: one point per superbatch (resumed runs
/// may repeat superbatches trained again after a checkpoint).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Metrics {
    /// Points, oldest first.
    pub(crate) points: Vec<MetricPoint>,
}

/// Where an event stream starts.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct EventsQuery {
    /// Sequence number of the last event received: the stream resumes
    /// after it (as the `Last-Event-ID` header does).
    pub(crate) after: Option<i64>,
}

/// Queues a training run. Everything needed to reproduce it is recorded:
/// the normalized architecture, the content hash of each dataset, the
/// hyperparameters and the seed (drawn when not given).
#[utoipa::path(
    post,
    path = "/api/v1/training-runs",
    operation_id = "createTrainingRun",
    tag = "training-runs",
    params(("Idempotency-Key" = Option<String>, Header, description = "Makes retries safe")),
    request_body = TrainingRequest,
    responses(
        (status = 202, description = "Queued", body = Job),
        (status = 422, description = "Invalid request, with every error", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn create_training(
    State(api): State<NnueApi>,
    JsonBody(request): JsonBody<TrainingRequest>,
) -> ApiResult<Response> {
    let job = api.lab.create_training(&request)?;
    let mut response = created(&format!("/api/v1/training-runs/{}", job.id), &job);
    *response.status_mut() = StatusCode::ACCEPTED;
    Ok(response)
}

pub(crate) fn page(api: &NnueApi, kind: JobKind, page: &PageQuery) -> ApiResult<Json<JobPage>> {
    let limit = page.limit()?;
    let items = api.lab.jobs(Some(kind), page.after()?, limit)?;
    Ok(Json(JobPage {
        next_cursor: next_cursor(items.len(), limit, items.last().map(|j| j.id)),
        items,
    }))
}

/// The job `id`, if it is of `kind`.
pub(crate) fn job_of(api: &NnueApi, kind: JobKind, id: &str) -> ApiResult<Job> {
    let what = match kind {
        JobKind::Training => "training run",
        JobKind::Selfplay => "self-play run",
        JobKind::Evaluation => "evaluation",
    };
    let id: JobId = parse_id(id, what)?;
    match api.lab.job(id) {
        Ok(job) if job.spec.kind() == kind => Ok(job),
        Ok(_) => Err(ApiProblem::not_found(format!("{what} {id} not found"))),
        Err(err) => Err(err.into()),
    }
}

/// Training runs.
#[utoipa::path(
    get,
    path = "/api/v1/training-runs",
    operation_id = "listTrainingRuns",
    tag = "training-runs",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of runs", body = JobPage),
        (status = 400, description = "Invalid cursor or limit", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn list_training(
    State(api): State<NnueApi>,
    Query(query): Query<PageQuery>,
) -> ApiResult<Json<JobPage>> {
    page(&api, JobKind::Training, &query)
}

/// A training run, with its latest progress.
#[utoipa::path(
    get,
    path = "/api/v1/training-runs/{id}",
    operation_id = "getTrainingRun",
    tag = "training-runs",
    params(("id" = String, Path, description = "Run id")),
    responses(
        (status = 200, description = "The run", body = Job),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn get_training(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<Json<Job>> {
    Ok(Json(job_of(&api, JobKind::Training, &id)?))
}

/// Stops a training run: a queued run is canceled, a running one saves a
/// checkpoint at the end of its superbatch and stops.
#[utoipa::path(
    post,
    path = "/api/v1/training-runs/{id}:stop",
    operation_id = "stopTrainingRun",
    tag = "training-runs",
    params(("id" = String, Path, description = "Run id")),
    responses(
        (status = 200, description = "`stopping` (or `canceled`)", body = Job),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "Already over", body = Problem, content_type = "application/problem+json"),
    )
)]
#[allow(dead_code)] // Documents a method served by `act_*`.
pub(crate) async fn stop_training_doc() {}

/// Queues a stopped, interrupted or failed training run again: it resumes
/// from its last checkpoint.
#[utoipa::path(
    post,
    path = "/api/v1/training-runs/{id}:resume",
    operation_id = "resumeTrainingRun",
    tag = "training-runs",
    params(("id" = String, Path, description = "Run id")),
    responses(
        (status = 200, description = "Queued", body = Job),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "Not stopped, interrupted or failed", body = Problem, content_type = "application/problem+json"),
    )
)]
#[allow(dead_code)] // Documents a method served by `act_*`.
pub(crate) async fn resume_training_doc() {}

/// `POST /training-runs/{id}:stop` and `:resume` (one route: the router
/// cannot tell a suffix from the id).
pub(crate) async fn act_training(
    State(api): State<NnueApi>,
    Path(segment): Path<String>,
) -> ApiResult<Json<Job>> {
    let (id, method) = custom_method(&segment)?;
    let job = job_of(&api, JobKind::Training, id)?;
    Ok(Json(match method {
        "stop" => api.lab.stop(job.id)?,
        "resume" => api.lab.resume(job.id)?,
        _ => {
            return Err(ApiProblem::not_found(format!(
                "training runs have no method {method:?} (stop, resume)"
            )));
        }
    }))
}

/// The training curve: loss, learning rate and speed per superbatch.
#[utoipa::path(
    get,
    path = "/api/v1/training-runs/{id}/metrics",
    operation_id = "getTrainingMetrics",
    tag = "training-runs",
    params(("id" = String, Path, description = "Run id")),
    responses(
        (status = 200, description = "The curve", body = Metrics),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn metrics(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<Json<Metrics>> {
    let job = job_of(&api, JobKind::Training, &id)?;
    let mut points = Vec::new();
    let mut after = 0;
    loop {
        let events = api.lab.events(job.id, after, 10_000)?;
        let Some(last) = events.last() else {
            break;
        };
        after = last.seq;
        points.extend(events.into_iter().filter_map(|event| {
            let progress = (event.kind == EventKind::Progress)
                .then(|| serde_json::from_value::<Progress>(event.data).ok())??;
            match progress {
                Progress::Training(progress) => Some(MetricPoint {
                    time: event.time,
                    progress,
                }),
                Progress::Selfplay(_) | Progress::Evaluation(_) => None,
            }
        }));
    }
    Ok(Json(Metrics { points }))
}

/// Live events of a training run (Server-Sent Events): `status`,
/// `progress`, `checkpoint`, `model`, `log`. Each event's `id` is its
/// sequence number; reconnecting with `Last-Event-ID` (or `after`)
/// resumes after it. The stream replays the run's history first, and ends
/// once the run is over.
#[utoipa::path(
    get,
    path = "/api/v1/training-runs/{id}/events",
    operation_id = "streamTrainingEvents",
    tag = "training-runs",
    params(
        ("id" = String, Path, description = "Run id"),
        EventsQuery,
        ("Last-Event-ID" = Option<String>, Header, description = "Resume after this event"),
    ),
    responses(
        (status = 200, description = "Event stream; each `data` is a JobEvent (JSON)", body = String, content_type = "text/event-stream"),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn events_training(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
    Query(query): Query<EventsQuery>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let job = job_of(&api, JobKind::Training, &id)?;
    Ok(event_stream(api, job.id, start_after(&query, &headers)))
}

/// Self-play runs.
#[utoipa::path(
    get,
    path = "/api/v1/selfplay-runs",
    operation_id = "listSelfplayRuns",
    tag = "selfplay-runs",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of runs", body = JobPage),
        (status = 400, description = "Invalid cursor or limit", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn list_selfplay(
    State(api): State<NnueApi>,
    Query(query): Query<PageQuery>,
) -> ApiResult<Json<JobPage>> {
    page(&api, JobKind::Selfplay, &query)
}

/// A self-play run (start one with `POST /api/v1/datasets:selfplay`).
#[utoipa::path(
    get,
    path = "/api/v1/selfplay-runs/{id}",
    operation_id = "getSelfplayRun",
    tag = "selfplay-runs",
    params(("id" = String, Path, description = "Run id")),
    responses(
        (status = 200, description = "The run", body = Job),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn get_selfplay(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<Json<Job>> {
    Ok(Json(job_of(&api, JobKind::Selfplay, &id)?))
}

/// Stops a self-play run: its dataset keeps the games already played.
#[utoipa::path(
    post,
    path = "/api/v1/selfplay-runs/{id}:stop",
    operation_id = "stopSelfplayRun",
    tag = "selfplay-runs",
    params(("id" = String, Path, description = "Run id")),
    responses(
        (status = 200, description = "`stopping` (or `canceled`)", body = Job),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "Already over", body = Problem, content_type = "application/problem+json"),
    )
)]
#[allow(dead_code)] // Documents a method served by `act_*`.
pub(crate) async fn stop_selfplay_doc() {}

/// `POST /selfplay-runs/{id}:stop`.
pub(crate) async fn act_selfplay(
    State(api): State<NnueApi>,
    Path(segment): Path<String>,
) -> ApiResult<Json<Job>> {
    let (id, method) = custom_method(&segment)?;
    let job = job_of(&api, JobKind::Selfplay, id)?;
    match method {
        "stop" => Ok(Json(api.lab.stop(job.id)?)),
        _ => Err(ApiProblem::not_found(format!(
            "self-play runs have no method {method:?} (stop)"
        ))),
    }
}

/// Live events of a self-play run (see the training runs' events).
#[utoipa::path(
    get,
    path = "/api/v1/selfplay-runs/{id}/events",
    operation_id = "streamSelfplayEvents",
    tag = "selfplay-runs",
    params(
        ("id" = String, Path, description = "Run id"),
        EventsQuery,
        ("Last-Event-ID" = Option<String>, Header, description = "Resume after this event"),
    ),
    responses(
        (status = 200, description = "Event stream; each `data` is a JobEvent (JSON)", body = String, content_type = "text/event-stream"),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn events_selfplay(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
    Query(query): Query<EventsQuery>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let job = job_of(&api, JobKind::Selfplay, &id)?;
    Ok(event_stream(api, job.id, start_after(&query, &headers)))
}

/// Sequence number to resume after: `Last-Event-ID`, else `after`, else 0.
pub(crate) fn start_after(query: &EventsQuery, headers: &HeaderMap) -> i64 {
    headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .or(query.after)
        .unwrap_or(0)
}

pub(crate) fn event_stream(api: NnueApi, job: JobId, after: i64) -> Response {
    let stream = events(api, job, after);
    (
        // Nginx would otherwise buffer the stream.
        [
            (HeaderName::from_static("x-accel-buffering"), "no"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
        .into_response()
}

/// The events of `job` after `after`, then new ones as they come, until the
/// job is over and every event was sent.
fn events(api: NnueApi, job: JobId, after: i64) -> impl Stream<Item = Result<Event, Infallible>> {
    futures_util::stream::unfold(
        (api, after, VecDeque::<JobEvent>::new()),
        move |(api, mut after, mut pending)| async move {
            loop {
                if let Some(event) = pending.pop_front() {
                    let sse = Event::default()
                        .id(event.seq.to_string())
                        .event(event.kind.as_str())
                        .json_data(&event)
                        .unwrap_or_default();
                    return Some((Ok(sse), (api, after, pending)));
                }
                // Read the status first: events logged after it are not missed.
                let over = api.lab.job(job).map_or(true, |job| job.status.is_final());
                match api.lab.events(job, after, 100) {
                    Ok(events) if !events.is_empty() => {
                        after = events.last().map_or(after, |e| e.seq);
                        pending.extend(events);
                    }
                    Ok(_) if over => return None,
                    Ok(_) => tokio::time::sleep(EVENT_POLL).await,
                    Err(err) => {
                        tracing::warn!(error = %err, "cannot read job events");
                        return None;
                    }
                }
            }
        },
    )
}
