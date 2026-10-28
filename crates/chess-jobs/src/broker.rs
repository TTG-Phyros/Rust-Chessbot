//! The workers' side of the queue: take a job, report on it, hand over
//! its result. [`crate::Lab`] implements it directly (worker in the same
//! process or on the same database); the server will expose it over HTTP
//! for workers in other containers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::LabError;
use crate::ids::JobId;
use crate::types::{Dataset, EventKind, Job, JobKind, Model, Progress, TrainingSummary};

/// What a worker can run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerInfo {
    /// Name, for the jobs it takes (e.g. its host name).
    pub id: String,
    /// Whether it trains on a GPU.
    pub gpu: bool,
    /// Kinds of jobs it accepts.
    pub kinds: Vec<JobKind>,
    /// Trainer backends it provides.
    pub trainers: Vec<String>,
}

/// A job taken by a worker. `attempt` identifies this run of it: a worker
/// whose job was taken away (lost heartbeat, then resumed elsewhere) can
/// no longer report on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    /// The job, as it was when taken.
    pub job: Job,
    /// Run of the job (1 for the first, then one more per resume).
    pub attempt: u32,
    /// Whether the job holds the GPU.
    pub gpu: bool,
}

impl Claim {
    /// The job and attempt, for reports.
    #[must_use]
    pub fn token(&self) -> ClaimToken {
        ClaimToken {
            job: self.job.id,
            attempt: self.attempt,
        }
    }
}

/// Identifies a claimed job in reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimToken {
    /// The job.
    pub job: JobId,
    /// Its attempt.
    pub attempt: u32,
}

/// What the worker should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Directive {
    /// Keep going.
    Continue,
    /// Save a checkpoint and stop (then call [`Broker::stopped`]).
    Stop,
}

/// The queue, from a worker.
pub trait Broker: Send + Sync {
    /// Takes the oldest queued job this worker can run, if any. A GPU
    /// training job is only given when no other job holds the GPU.
    ///
    /// # Errors
    ///
    /// When the queue cannot be reached.
    fn claim(&self, worker: &WorkerInfo) -> Result<Option<Claim>, LabError>;

    /// The worker is alive; tells it whether to stop.
    ///
    /// # Errors
    ///
    /// [`LabError::Lost`] when the job was taken away.
    fn heartbeat(&self, claim: ClaimToken) -> Result<Directive, LabError>;

    /// Latest progress (also a heartbeat).
    ///
    /// # Errors
    ///
    /// [`LabError::Lost`] when the job was taken away.
    fn progress(&self, claim: ClaimToken, progress: &Progress) -> Result<Directive, LabError>;

    /// Records an event (checkpoint, log).
    ///
    /// # Errors
    ///
    /// [`LabError::Lost`] when the job was taken away.
    fn event(&self, claim: ClaimToken, kind: EventKind, data: Value) -> Result<(), LabError>;

    /// A training job finished: stores the network (`CBNN` version 2, of
    /// the job's architecture) as a new model.
    ///
    /// # Errors
    ///
    /// [`LabError::Invalid`] when the network is not of the job's
    /// architecture, [`LabError::Lost`] when the job was taken away.
    fn complete_training(
        &self,
        claim: ClaimToken,
        network: &[u8],
        summary: &TrainingSummary,
    ) -> Result<Model, LabError>;

    /// A self-play job finished (or stopped on request): its files are in
    /// the dataset's directory.
    ///
    /// # Errors
    ///
    /// When its files are unreadable, [`LabError::Lost`] when the job was
    /// taken away.
    fn complete_selfplay(&self, claim: ClaimToken) -> Result<Dataset, LabError>;

    /// The worker stopped the job, after a checkpoint: `stopping` jobs
    /// become `stopped`, `running` ones (worker shutting down) `interrupted`.
    ///
    /// # Errors
    ///
    /// [`LabError::Lost`] when the job was taken away.
    fn stopped(&self, claim: ClaimToken) -> Result<(), LabError>;

    /// The job failed.
    ///
    /// # Errors
    ///
    /// [`LabError::Lost`] when the job was taken away.
    fn fail(&self, claim: ClaimToken, message: &str) -> Result<(), LabError>;
}
