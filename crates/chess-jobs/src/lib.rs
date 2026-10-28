//! The NNUE lab's store and job queue.
//!
//! A [`Lab`] keeps datasets, jobs (training, self-play, evaluation), their events and
//! the models they produce in one SQLite database, owned by the server,
//! and their files in [`LabPaths`]. Workers take jobs through the
//! [`Broker`] trait: one job at a time each, at most one job on the GPU,
//! with heartbeats so that a job whose worker disappeared is marked
//! `interrupted` and can resume from its last checkpoint.
//!
//! Every identifier is a UUIDv7 generated here, and every file path is
//! derived from one: nothing a client sends is used as a path.

mod broker;
mod hyper;
mod ids;
mod lab;
mod paths;
mod stats;
mod types;

use chess_nnue_spec::ValidationError;
use thiserror::Error;

pub use broker::{Broker, Claim, ClaimToken, Directive, WorkerInfo};
pub use hyper::Hyperparameters;
pub use ids::{ArchitectureId, DatasetId, JobId, ModelId};
pub use lab::{IDEMPOTENCY_TTL, Idempotency, Lab, MAX_PAGE};
pub use paths::LabPaths;
pub use stats::{MatchResult, Sprt, Verdict, ratings};
pub use types::{
    BURN, CLASSIC, Contestant, DEFAULT_MATCH_NODES, Dataset, DatasetRef, DatasetSource,
    DatasetStatus, Device, EvaluationJob, EvaluationProgress, EvaluationRequest, EventKind, Job,
    JobEvent, JobKind, JobSpec, JobStatus, Model, Progress, Provenance, SelfplayJob,
    SelfplayProgress, SelfplayRequest, SprtStatus, StoredArchitecture, TrainingJob,
    TrainingProgress, TrainingRequest, TrainingSummary,
};

/// Why a lab operation failed.
#[derive(Debug, Error)]
pub enum LabError {
    /// No such resource.
    #[error("{what} {id} not found")]
    NotFound {
        /// Kind of resource (`job`, `dataset`, `model`).
        what: &'static str,
        /// Its identifier.
        id: String,
    },
    /// The request is invalid; every problem has a JSON pointer.
    #[error("invalid request: {}", summary(.0))]
    Invalid(Vec<ValidationError>),
    /// The operation does not apply to the resource in its current state.
    #[error("{0}")]
    Conflict(String),
    /// The worker's claim on the job is no longer valid (stopped,
    /// interrupted, or taken by another worker).
    #[error("job {0} is no longer held by this worker")]
    Lost(JobId),
    /// An uploaded dataset is invalid.
    #[error(transparent)]
    Dataset(#[from] chess_dataset::DatasetError),
    /// The database failed.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// A file could not be read or written.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The queue could not be reached (remote brokers).
    #[error("lab unavailable: {0}")]
    Unavailable(String),
}

/// Reference to the architecture schema, which the API adds to its
/// components (it comes from `chess_nnue_spec::json_schema`).
#[cfg(feature = "openapi")]
fn architecture_schema() -> utoipa::openapi::Ref {
    utoipa::openapi::Ref::from_schema_name("Architecture")
}

fn summary(errors: &[ValidationError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

impl LabError {
    fn invalid(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Invalid(vec![ValidationError {
            path: path.into(),
            message: message.into(),
        }])
    }
}
