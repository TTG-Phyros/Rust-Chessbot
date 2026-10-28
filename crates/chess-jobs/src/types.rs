//! What the lab stores: datasets, jobs and their events, models.

use chess_dataset::DatasetStats;
use chess_nnue_spec::Architecture;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::hyper::Hyperparameters;
use crate::ids::{ArchitectureId, DatasetId, JobId, ModelId};

/// Text values of an enum stored in a column.
macro_rules! text_enum {
    ($name:ident { $($variant:ident = $text:literal),+ $(,)? }) => {
        impl $name {
            /// Name in the database and in JSON.
            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)+
                }
            }

            /// The value named `text`.
            #[must_use]
            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

/// What a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum JobKind {
    /// Trains a network on datasets; produces a model.
    Training,
    /// Plays games to produce a dataset.
    Selfplay,
}

text_enum!(JobKind {
    Training = "training",
    Selfplay = "selfplay",
});

/// Where a job is in its life.
///
/// ```text
/// queued ──claim──▶ running ──▶ succeeded | failed
///   │                 │ stop          │ worker lost (no heartbeat)
///   ▼                 ▼               ▼
/// canceled         stopping ──▶ stopped     interrupted
///                                  └──── resume ───┴──▶ queued
/// ```
///
/// `failed` training jobs can be resumed too (from their last checkpoint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Waiting for a worker.
    Queued,
    /// A worker runs it.
    Running,
    /// Stop requested: the worker saves a checkpoint and stops.
    Stopping,
    /// Stopped on request; training can resume.
    Stopped,
    /// Its worker disappeared; training can resume.
    Interrupted,
    /// Done.
    Succeeded,
    /// Ended with an error.
    Failed,
    /// Stopped before it started.
    Canceled,
}

text_enum!(JobStatus {
    Queued = "queued",
    Running = "running",
    Stopping = "stopping",
    Stopped = "stopped",
    Interrupted = "interrupted",
    Succeeded = "succeeded",
    Failed = "failed",
    Canceled = "canceled",
});

impl JobStatus {
    /// Whether a worker holds the job.
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::Stopping)
    }

    /// Whether nothing will happen to the job unless it is resumed.
    #[must_use]
    pub fn is_final(self) -> bool {
        !matches!(self, Self::Queued | Self::Running | Self::Stopping)
    }
}

/// Where a training job runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Device {
    /// On the GPU of a worker that has one, otherwise on a CPU worker.
    #[default]
    Auto,
    /// Only on a GPU.
    Gpu,
    /// On a CPU, even on a GPU worker.
    Cpu,
}

text_enum!(Device {
    Auto = "auto",
    Gpu = "gpu",
    Cpu = "cpu",
});

/// A dataset a training job reads, as it was when the job was created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DatasetRef {
    /// The dataset.
    pub id: DatasetId,
    /// Its content hash (SHA-256 of the records).
    pub sha256: String,
    /// Its positions.
    pub records: u64,
}

/// A training job, fully resolved: everything needed to reproduce it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TrainingJob {
    /// Architecture, normalized.
    #[cfg_attr(feature = "openapi", schema(schema_with = crate::architecture_schema))]
    pub architecture: Architecture,
    /// Its hash.
    pub architecture_hash: String,
    /// Training data.
    pub datasets: Vec<DatasetRef>,
    /// Hyperparameters.
    pub hyperparameters: Hyperparameters,
    /// Seed of the initialization and of the data order.
    pub seed: u64,
    /// Where it may run.
    pub device: Device,
    /// Trainer backend (`burn`).
    pub trainer: String,
}

/// A self-play job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SelfplayJob {
    /// Dataset filled by the games.
    pub dataset: DatasetId,
    /// Games to play.
    pub games: u64,
    /// Nodes searched per move.
    pub nodes: u64,
    /// Seed of the random openings.
    pub seed: u64,
    /// Network playing the games; `None`: the classical evaluation.
    pub network: Option<ModelId>,
    /// Worker threads (0: all cores).
    pub threads: usize,
}

/// What a job does, with all its parameters.
// One per job, never in hot paths: boxing the large variant buys nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum JobSpec {
    /// See [`TrainingJob`].
    Training(TrainingJob),
    /// See [`SelfplayJob`].
    Selfplay(SelfplayJob),
}

impl JobSpec {
    /// The kind of job.
    #[must_use]
    pub fn kind(&self) -> JobKind {
        match self {
            Self::Training(_) => JobKind::Training,
            Self::Selfplay(_) => JobKind::Selfplay,
        }
    }
}

/// Latest progress of a training job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TrainingProgress {
    /// Superbatches done.
    pub superbatch: u64,
    /// Superbatches to do.
    pub superbatches: u64,
    /// Optimizer steps done.
    pub step: u64,
    /// Positions trained on.
    pub positions: u64,
    /// Mean loss of the last superbatch.
    pub loss: f64,
    /// Learning rate of the last superbatch.
    pub learning_rate: f64,
    /// Speed of the last superbatch.
    pub positions_per_second: f64,
    /// GPU memory in use, when on a GPU.
    pub vram_used_mb: Option<u64>,
}

/// Latest progress of a self-play job.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SelfplayProgress {
    /// Games played.
    pub games: u64,
    /// Games to play.
    pub games_total: u64,
    /// Positions recorded.
    pub positions: u64,
}

/// Latest progress of a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Progress {
    /// See [`TrainingProgress`].
    Training(TrainingProgress),
    /// See [`SelfplayProgress`].
    Selfplay(SelfplayProgress),
}

/// A job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Job {
    /// Identifier.
    pub id: JobId,
    /// Name given at creation.
    pub name: String,
    /// Status.
    pub status: JobStatus,
    /// What it does.
    pub spec: JobSpec,
    /// Latest progress reported by its worker.
    pub progress: Option<Progress>,
    /// Model produced (training jobs that succeeded).
    pub model: Option<ModelId>,
    /// Why it failed.
    pub error: Option<String>,
    /// Worker holding it, or that held it last.
    pub worker: Option<String>,
    /// Times a worker took it (resumes count).
    pub attempt: u32,
    /// Creation time (Unix milliseconds).
    pub created_at: i64,
    /// When a worker first took it.
    pub started_at: Option<i64>,
    /// Last change.
    pub updated_at: i64,
    /// When it reached a final status.
    pub finished_at: Option<i64>,
}

/// Kind of a job event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    /// Status change: `{status, attempt, error?}`.
    Status,
    /// Progress report: a [`Progress`].
    Progress,
    /// Checkpoint saved: `{superbatch}`.
    Checkpoint,
    /// Model produced: `{modelId}`.
    Model,
    /// Message from the worker: `{message}`.
    Log,
}

text_enum!(EventKind {
    Status = "status",
    Progress = "progress",
    Checkpoint = "checkpoint",
    Model = "model",
    Log = "log",
});

/// Something that happened to a job, in order (`seq` grows across jobs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    /// Position in the log of all events.
    pub seq: i64,
    /// The job.
    pub job: JobId,
    /// When (Unix milliseconds).
    pub time: i64,
    /// What.
    pub kind: EventKind,
    /// Details, depending on `kind`.
    pub data: Value,
}

/// Where a dataset comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum DatasetSource {
    /// Uploaded file.
    Upload,
    /// Generated by a self-play job.
    Selfplay,
}

text_enum!(DatasetSource {
    Upload = "upload",
    Selfplay = "selfplay",
});

/// Whether a dataset can be trained on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum DatasetStatus {
    /// Being generated by its self-play job.
    Generating,
    /// Complete.
    Ready,
    /// Its generation failed or was canceled.
    Failed,
}

text_enum!(DatasetStatus {
    Generating = "generating",
    Ready = "ready",
    Failed = "failed",
});

/// A dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    /// Identifier.
    pub id: DatasetId,
    /// Name given at creation.
    pub name: String,
    /// Origin.
    pub source: DatasetSource,
    /// Status.
    pub status: DatasetStatus,
    /// Positions.
    pub records: u64,
    /// Size of its files.
    pub bytes: u64,
    /// Content hash (SHA-256 of the records), once ready.
    pub sha256: Option<String>,
    /// Statistics, once ready.
    pub stats: Option<DatasetStats>,
    /// Self-play job generating it.
    pub job: Option<JobId>,
    /// Why it failed.
    pub error: Option<String>,
    /// Creation time (Unix milliseconds).
    pub created_at: i64,
    /// Last change.
    pub updated_at: i64,
}

/// How a model was made: enough to train it again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    /// Trainer backend.
    pub trainer: String,
    /// Seed.
    pub seed: u64,
    /// Training data, with content hashes.
    pub datasets: Vec<DatasetRef>,
    /// Hyperparameters.
    pub hyperparameters: Hyperparameters,
    /// Superbatches trained.
    pub superbatches: u64,
    /// Positions trained on.
    pub positions: u64,
    /// Loss of the last superbatch.
    pub final_loss: Option<f64>,
    /// Device it was trained on.
    pub device: String,
}

/// A trained network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// Identifier.
    pub id: ModelId,
    /// Name (the training job's).
    pub name: String,
    /// Training job that produced it.
    pub job: Option<JobId>,
    /// Architecture, normalized.
    #[cfg_attr(feature = "openapi", schema(schema_with = crate::architecture_schema))]
    pub architecture: Architecture,
    /// Its hash.
    pub architecture_hash: String,
    /// SHA-256 of the network file.
    pub file_sha256: String,
    /// Size of the network file.
    pub bytes: u64,
    /// How it was made.
    pub provenance: Provenance,
    /// Creation time (Unix milliseconds).
    pub created_at: i64,
}

/// A training job, as a client asks for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingRequest {
    /// Name (default: the architecture's).
    #[serde(default)]
    pub name: Option<String>,
    /// Architecture document (validated like `POST /architectures:validate`).
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(schema_with = crate::architecture_schema))]
    pub architecture: Option<Value>,
    /// A saved architecture, instead of `architecture`.
    #[serde(default)]
    pub architecture_id: Option<ArchitectureId>,
    /// Datasets to train on (ready ones).
    pub datasets: Vec<DatasetId>,
    /// Hyperparameters (defaults for the missing ones).
    #[serde(default)]
    pub hyperparameters: Hyperparameters,
    /// Seed (default: drawn at random, then recorded).
    #[serde(default)]
    pub seed: Option<u64>,
    /// Where it may run.
    #[serde(default)]
    pub device: Device,
    /// Trainer backend.
    #[serde(default = "default_trainer")]
    pub trainer: String,
}

fn default_trainer() -> String {
    BURN.into()
}

/// Name of the built-in trainer.
pub const BURN: &str = "burn";

/// A self-play dataset, as a client asks for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfplayRequest {
    /// Name of the dataset (default: from the parameters).
    #[serde(default)]
    pub name: Option<String>,
    /// Games to play.
    pub games: u64,
    /// Nodes searched per move.
    #[serde(default = "default_nodes")]
    pub nodes: u64,
    /// Seed (default: drawn at random, then recorded).
    #[serde(default)]
    pub seed: Option<u64>,
    /// Model playing the games (default: the classical evaluation).
    #[serde(default)]
    pub network: Option<ModelId>,
    /// Worker threads (0: all cores).
    #[serde(default)]
    pub threads: usize,
}

fn default_nodes() -> u64 {
    5_000
}

/// Report of a finished training, sent with the network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TrainingSummary {
    /// Superbatches trained.
    pub superbatches: u64,
    /// Positions trained on.
    pub positions: u64,
    /// Loss of the last superbatch.
    pub final_loss: Option<f64>,
    /// Device.
    pub device: String,
}

/// An architecture saved for reuse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StoredArchitecture {
    /// Identifier.
    pub id: ArchitectureId,
    /// Name (from its metadata).
    pub name: String,
    /// Hash.
    pub hash: String,
    /// Architecture, normalized (every default written out).
    #[cfg_attr(feature = "openapi", schema(schema_with = crate::architecture_schema))]
    pub architecture: Architecture,
    /// Creation time (Unix milliseconds).
    pub created_at: i64,
}
