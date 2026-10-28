//! `/api/v1/datasets`: uploads, self-play generation, statistics.

use std::io::Read;

use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use chess_dataset::DatasetStats;
use chess_jobs::{Dataset, DatasetId, DatasetStatus, Job, SelfplayRequest};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use utoipa::{IntoParams, ToSchema};

use super::architectures::created;
use super::{JsonBody, parse_id};
use crate::NnueApi;
use crate::pagination::{DatasetPage, PageQuery, next_cursor};
use crate::problem::{ApiProblem, ApiResult, Problem};

/// Parameters of an upload.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct UploadQuery {
    /// Name of the dataset (1 to 100 characters).
    pub(crate) name: String,
}

/// A self-play run and the dataset it fills.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelfplayStarted {
    /// The run (see `/api/v1/selfplay-runs/{id}`).
    pub(crate) run: Job,
    /// The dataset, `generating` until the run ends.
    pub(crate) dataset: Dataset,
}

/// Uploads a dataset file (`CBDS`, see the documentation). It is checked
/// record by record while it streams to disk; nothing is kept if it is
/// invalid.
#[utoipa::path(
    post,
    path = "/api/v1/datasets",
    operation_id = "uploadDataset",
    tag = "datasets",
    params(
        UploadQuery,
        ("Idempotency-Key" = Option<String>, Header, description = "Makes retries safe"),
    ),
    request_body(content = inline(crate::openapi::Binary), content_type = "application/octet-stream", description = "The CBDS file"),
    responses(
        (status = 201, description = "Stored, with its hash and statistics", body = Dataset),
        (status = 413, description = "Larger than the server's limit", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Not application/octet-stream", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid name or file", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn upload(
    State(api): State<NnueApi>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
    body: Body,
) -> ApiResult<Response> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("application/octet-stream") {
        return Err(ApiProblem::unsupported_media_type(
            "send the file as application/octet-stream",
        ));
    }
    let limit = api.config.max_upload_bytes;
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|length| length > limit) {
        return Err(ApiProblem::too_large(limit));
    }

    // The import reads on a blocking thread what the body stream receives.
    let (sender, receiver) = mpsc::channel(8);
    let lab = api.lab.clone();
    let import = tokio::task::spawn_blocking(move || {
        lab.import_dataset(&query.name, ChannelReader::new(receiver), limit)
    });
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        // A closed channel: the import stopped (invalid file, limit).
        if sender
            .send(chunk.map_err(std::io::Error::other))
            .await
            .is_err()
        {
            break;
        }
    }
    drop(sender);
    let dataset = import.await.map_err(ApiProblem::internal)??;
    Ok(created(
        &format!("/api/v1/datasets/{}", dataset.id),
        &dataset,
    ))
}

/// A blocking reader of the chunks an async task sends.
struct ChannelReader {
    receiver: mpsc::Receiver<Result<Bytes, std::io::Error>>,
    current: Bytes,
}

impl ChannelReader {
    fn new(receiver: mpsc::Receiver<Result<Bytes, std::io::Error>>) -> Self {
        Self {
            receiver,
            current: Bytes::new(),
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.current.is_empty() {
            match self.receiver.blocking_recv() {
                Some(Ok(chunk)) => self.current = chunk,
                Some(Err(err)) => return Err(err),
                None => return Ok(0),
            }
        }
        let n = buf.len().min(self.current.len());
        buf[..n].copy_from_slice(&self.current.split_to(n));
        Ok(n)
    }
}

/// Starts a self-play run that generates a new dataset.
#[utoipa::path(
    post,
    path = "/api/v1/datasets:selfplay",
    operation_id = "startSelfplay",
    tag = "datasets",
    params(("Idempotency-Key" = Option<String>, Header, description = "Makes retries safe")),
    request_body = SelfplayRequest,
    responses(
        (status = 202, description = "Queued", body = SelfplayStarted),
        (status = 422, description = "Invalid request, with every error", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn selfplay(
    State(api): State<NnueApi>,
    JsonBody(request): JsonBody<SelfplayRequest>,
) -> ApiResult<Response> {
    let (run, dataset) = api.lab.create_selfplay(&request)?;
    let location = format!("/api/v1/selfplay-runs/{}", run.id);
    let mut response = created(&location, &SelfplayStarted { run, dataset });
    *response.status_mut() = StatusCode::ACCEPTED;
    Ok(response)
}

/// Datasets.
#[utoipa::path(
    get,
    path = "/api/v1/datasets",
    operation_id = "listDatasets",
    tag = "datasets",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of datasets", body = DatasetPage),
        (status = 400, description = "Invalid cursor or limit", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn list(
    State(api): State<NnueApi>,
    Query(page): Query<PageQuery>,
) -> ApiResult<Json<DatasetPage>> {
    let limit = page.limit()?;
    let items = api.lab.datasets(page.after()?, limit)?;
    Ok(Json(DatasetPage {
        next_cursor: next_cursor(items.len(), limit, items.last().map(|d| d.id)),
        items,
    }))
}

/// A dataset.
#[utoipa::path(
    get,
    path = "/api/v1/datasets/{id}",
    operation_id = "getDataset",
    tag = "datasets",
    params(("id" = String, Path, description = "Dataset id")),
    responses(
        (status = 200, description = "The dataset", body = Dataset),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn get(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<Json<Dataset>> {
    Ok(Json(api.lab.dataset(parse_id(&id, "dataset")?)?))
}

/// Statistics of a ready dataset.
#[utoipa::path(
    get,
    path = "/api/v1/datasets/{id}/stats",
    operation_id = "getDatasetStats",
    tag = "datasets",
    params(("id" = String, Path, description = "Dataset id")),
    responses(
        (status = 200, description = "Statistics", body = DatasetStats),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "Not ready yet, or failed", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn stats(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<Json<DatasetStats>> {
    let id: DatasetId = parse_id(&id, "dataset")?;
    let dataset = api.lab.dataset(id)?;
    match (dataset.status, dataset.stats) {
        (DatasetStatus::Ready, Some(stats)) => Ok(Json(stats)),
        (status, _) => Err(ApiProblem::conflict(format!(
            "dataset {id} is {}: no statistics",
            status.as_str()
        ))),
    }
}

/// Deletes a dataset and its files.
#[utoipa::path(
    delete,
    path = "/api/v1/datasets/{id}",
    operation_id = "deleteDataset",
    tag = "datasets",
    params(("id" = String, Path, description = "Dataset id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "Being generated, or used by a queued or running training", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn delete(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id: DatasetId = parse_id(&id, "dataset")?;
    let lab = api.lab.clone();
    crate::blocking(move || lab.delete_dataset(id).map_err(ApiProblem::from)).await?;
    Ok(StatusCode::NO_CONTENT)
}
