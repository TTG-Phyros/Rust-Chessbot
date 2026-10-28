//! The lab's queue over HTTP (`/internal/v1` of `chess-server`): what a
//! worker in another container uses instead of opening the database.
//! Errors come back as the same [`LabError`]s as from a local [`chess_jobs::Lab`].

use std::time::Duration;

use chess_jobs::{
    Broker, Claim, ClaimToken, Dataset, Directive, EvaluationProgress, EventKind, Job, LabError,
    Model, Progress, TrainingSummary, WorkerInfo,
};
use chess_nnue::spec::ValidationError;
use reqwest::StatusCode;
use reqwest::blocking::{Client, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// A [`Broker`] reached over HTTP.
pub(crate) struct HttpBroker {
    base: String,
    token: String,
    client: Client,
}

impl HttpBroker {
    /// The queue of the server at `base` (e.g. `http://server:3000`),
    /// with the workers' `token`.
    pub(crate) fn new(base: &str, token: &str) -> anyhow::Result<Self> {
        Ok(Self {
            base: base.trim_end_matches('/').to_owned(),
            token: token.to_owned(),
            // Uploading a large network takes a while.
            client: Client::builder()
                .timeout(Duration::from_secs(300))
                .build()?,
        })
    }

    fn post(&self, path: &str) -> RequestBuilder {
        self.client
            .post(format!("{}/internal/v1{path}", self.base))
            .bearer_auth(&self.token)
    }

    fn attempt(claim: ClaimToken, action: &str) -> String {
        format!("/jobs/{}/attempts/{}/{action}", claim.job, claim.attempt)
    }

    /// Sends `request`; non-success answers become [`LabError`]s.
    fn send(&self, request: RequestBuilder) -> Result<Response, LabError> {
        let response = request
            .send()
            .map_err(|err| LabError::Unavailable(err.to_string()))?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let problem: Value = response.json().unwrap_or_default();
        let detail = problem["detail"]
            .as_str()
            .or(problem["title"].as_str())
            .unwrap_or("no detail")
            .to_owned();
        let kind = problem["type"]
            .as_str()
            .and_then(|t| t.strip_prefix("urn:chess-bot:problem:"))
            .unwrap_or("");
        Err(match (status, kind) {
            (_, "lost-claim") => match request_job(&problem) {
                Some(job) => LabError::Lost(job),
                None => LabError::Conflict(detail),
            },
            (_, "validation-error") => LabError::Invalid(
                problem["errors"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|e| ValidationError {
                        path: e["path"].as_str().unwrap_or("").to_owned(),
                        message: e["message"].as_str().unwrap_or("").to_owned(),
                    })
                    .collect(),
            ),
            (StatusCode::NOT_FOUND, _) => LabError::NotFound {
                what: "resource",
                id: detail,
            },
            (StatusCode::CONFLICT, _) => LabError::Conflict(detail),
            _ => LabError::Unavailable(format!("{status}: {detail}")),
        })
    }

    fn json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, LabError> {
        self.send(request)?
            .json()
            .map_err(|err| LabError::Unavailable(format!("unexpected answer: {err}")))
    }
}

/// The job a `lost-claim` problem is about (named in its detail).
fn request_job(problem: &Value) -> Option<chess_jobs::JobId> {
    problem["detail"]
        .as_str()?
        .split_whitespace()
        .find_map(|word| word.parse().ok())
}

impl Broker for HttpBroker {
    fn claim(&self, worker: &WorkerInfo) -> Result<Option<Claim>, LabError> {
        let response = self.send(self.post("/claims").json(worker))?;
        if response.status() == StatusCode::NO_CONTENT {
            return Ok(None);
        }
        response
            .json()
            .map(Some)
            .map_err(|err| LabError::Unavailable(format!("unexpected answer: {err}")))
    }

    fn heartbeat(&self, claim: ClaimToken) -> Result<Directive, LabError> {
        self.json(self.post(&Self::attempt(claim, "heartbeat")))
    }

    fn progress(&self, claim: ClaimToken, progress: &Progress) -> Result<Directive, LabError> {
        self.json(self.post(&Self::attempt(claim, "progress")).json(progress))
    }

    fn event(&self, claim: ClaimToken, kind: EventKind, data: Value) -> Result<(), LabError> {
        let body = json!({ "kind": kind, "data": data });
        self.send(self.post(&Self::attempt(claim, "events")).json(&body))
            .map(drop)
    }

    fn complete_training(
        &self,
        claim: ClaimToken,
        network: &[u8],
        summary: &TrainingSummary,
    ) -> Result<Model, LabError> {
        self.json(
            self.post(&Self::attempt(claim, "network"))
                .query(summary)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(network.to_vec()),
        )
    }

    fn complete_selfplay(&self, claim: ClaimToken) -> Result<Dataset, LabError> {
        self.json(self.post(&Self::attempt(claim, "dataset")))
    }

    fn complete_evaluation(
        &self,
        claim: ClaimToken,
        result: &EvaluationProgress,
    ) -> Result<Job, LabError> {
        self.json(self.post(&Self::attempt(claim, "evaluation")).json(result))
    }

    fn stopped(&self, claim: ClaimToken) -> Result<(), LabError> {
        self.send(self.post(&Self::attempt(claim, "stopped")))
            .map(drop)
    }

    fn fail(&self, claim: ClaimToken, message: &str) -> Result<(), LabError> {
        let body = json!({ "message": message });
        self.send(self.post(&Self::attempt(claim, "failure")).json(&body))
            .map(drop)
    }
}
