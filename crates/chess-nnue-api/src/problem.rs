//! Errors as RFC 9457 problem details (`application/problem+json`).

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chess_dataset::DatasetError;
use chess_jobs::LabError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Media type of error responses.
pub const PROBLEM_JSON: &str = "application/problem+json";

/// One problem of a request, located by a JSON pointer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct FieldError {
    /// JSON pointer (RFC 6901) to the offending value; `""` is the whole body.
    pub path: String,
    /// What is wrong, and usually how to fix it.
    pub message: String,
}

/// An error (RFC 9457). `type` identifies the kind of problem
/// (`urn:chess-bot:problem:<name>`); validation problems list every error
/// found in `errors`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Problem {
    /// Kind of problem, e.g. `urn:chess-bot:problem:validation-error`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Short summary of the kind of problem.
    pub title: String,
    /// HTTP status.
    pub status: u16,
    /// What went wrong with this request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Every problem of the request body, for validation errors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<FieldError>,
}

/// An error response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiProblem {
    pub(crate) problem: Problem,
    /// Seconds to wait before retrying (rate limiting).
    pub(crate) retry_after: Option<u64>,
}

impl ApiProblem {
    pub(crate) fn new(status: StatusCode, name: &str, title: &str) -> Self {
        Self {
            problem: Problem {
                kind: format!("urn:chess-bot:problem:{name}"),
                title: title.into(),
                status: status.as_u16(),
                detail: None,
                errors: Vec::new(),
            },
            retry_after: None,
        }
    }

    pub(crate) fn detail(mut self, detail: impl Into<String>) -> Self {
        self.problem.detail = Some(detail.into());
        self
    }

    /// 400: the request itself is malformed (bad cursor, header, id).
    pub(crate) fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad-request", "Malformed request").detail(detail)
    }

    /// 404.
    pub(crate) fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not-found", "Not found").detail(detail)
    }

    /// 409.
    pub(crate) fn conflict(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "conflict",
            "Conflict with the current state",
        )
        .detail(detail)
    }

    /// 422 with the errors of the body.
    pub(crate) fn validation(errors: Vec<FieldError>) -> Self {
        let mut problem = Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation-error",
            "The request is invalid",
        )
        .detail(format!("{} error(s), see `errors`", errors.len()));
        problem.problem.errors = errors;
        problem
    }

    /// 415.
    pub(crate) fn unsupported_media_type(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported-media-type",
            "Unsupported media type",
        )
        .detail(detail)
    }

    /// 413.
    pub(crate) fn too_large(limit: u64) -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload-too-large",
            "Request body too large",
        )
        .detail(format!("the limit is {limit} bytes"))
    }

    /// 500, logged; the client gets no internal detail.
    pub(crate) fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(%error, "lab request failed");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "Internal error",
        )
    }

    /// The HTTP status.
    #[must_use]
    pub(crate) fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.problem.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl From<LabError> for ApiProblem {
    fn from(error: LabError) -> Self {
        match error {
            LabError::NotFound { what, id } => Self::not_found(format!("{what} {id} not found")),
            LabError::Invalid(errors) => Self::validation(
                errors
                    .into_iter()
                    .map(|e| FieldError {
                        path: e.path,
                        message: e.message,
                    })
                    .collect(),
            ),
            LabError::Conflict(message) => Self::conflict(message),
            LabError::Lost(job) => Self::new(
                StatusCode::CONFLICT,
                "lost-claim",
                "The job is no longer held by this worker",
            )
            .detail(format!(
                "job {job} was stopped, interrupted or taken by another worker"
            )),
            LabError::Dataset(DatasetError::TooLarge(limit)) => Self::too_large(limit),
            LabError::Dataset(DatasetError::Io(error)) => Self::internal(error),
            LabError::Dataset(error) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid-dataset",
                "The dataset is not a valid CBDS file",
            )
            .detail(error.to_string()),
            LabError::Unavailable(message) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Service unavailable",
            )
            .detail(message),
            error @ (LabError::Database(_) | LabError::Io(_)) => Self::internal(error),
        }
    }
}

impl IntoResponse for ApiProblem {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = serde_json::to_vec(&self.problem).unwrap_or_default();
        let mut response = (status, body).into_response();
        let headers = response.headers_mut();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(PROBLEM_JSON));
        if let Some(seconds) = self.retry_after {
            headers.insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}

/// Result of a handler.
pub(crate) type ApiResult<T> = Result<T, ApiProblem>;
