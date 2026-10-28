//! The lab: SQLite store, client operations and the workers' broker.
//!
//! Operations are short (a few rows) and run under one mutex; the slow
//! parts — reading an upload, hashing a dataset, writing a network — run
//! outside it, and the job's claim is checked again before committing.

use std::hash::{BuildHasher, RandomState};
use std::io::Read;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use chess_nnue_spec::ValidationError;
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::LabError;
use crate::broker::{Broker, Claim, ClaimToken, Directive, WorkerInfo};
use crate::ids::{DatasetId, JobId, ModelId};
use crate::paths::LabPaths;
use crate::types::{
    BURN, Dataset, DatasetRef, DatasetSource, DatasetStatus, Device, EventKind, Job, JobEvent,
    JobKind, JobSpec, JobStatus, Model, Progress, Provenance, SelfplayJob, SelfplayRequest,
    TrainingJob, TrainingRequest, TrainingSummary,
};

/// Schema, one entry per version (`PRAGMA user_version`).
const MIGRATIONS: &[&str] = &["CREATE TABLE datasets (
        id         TEXT PRIMARY KEY,
        name       TEXT NOT NULL,
        source     TEXT NOT NULL,      -- upload | selfplay
        status     TEXT NOT NULL,      -- generating | ready | failed
        records    INTEGER NOT NULL DEFAULT 0,
        bytes      INTEGER NOT NULL DEFAULT 0,
        sha256     TEXT,
        stats      TEXT,               -- chess_dataset::DatasetStats (JSON)
        job_id     TEXT,
        error      TEXT,
        created_at INTEGER NOT NULL,   -- Unix milliseconds
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE jobs (
        id           TEXT PRIMARY KEY,
        kind         TEXT NOT NULL,    -- training | selfplay
        name         TEXT NOT NULL,
        status       TEXT NOT NULL,
        spec         TEXT NOT NULL,    -- JobSpec (JSON)
        device       TEXT NOT NULL,
        gpu          INTEGER NOT NULL DEFAULT 0,  -- holds the GPU slot
        progress     TEXT,             -- Progress (JSON)
        model_id     TEXT,
        error        TEXT,
        worker       TEXT,
        attempt      INTEGER NOT NULL DEFAULT 0,
        created_at   INTEGER NOT NULL,
        started_at   INTEGER,
        updated_at   INTEGER NOT NULL,
        finished_at  INTEGER,
        heartbeat_at INTEGER
    );
    CREATE INDEX jobs_by_status ON jobs (status, id);
    CREATE TABLE job_events (
        seq    INTEGER PRIMARY KEY AUTOINCREMENT,
        job_id TEXT NOT NULL,
        time   INTEGER NOT NULL,
        kind   TEXT NOT NULL,
        data   TEXT NOT NULL
    );
    CREATE INDEX job_events_by_job ON job_events (job_id, seq);
    CREATE TABLE models (
        id                TEXT PRIMARY KEY,
        name              TEXT NOT NULL,
        job_id            TEXT,
        architecture      TEXT NOT NULL,   -- normalized architecture (JSON)
        architecture_hash TEXT NOT NULL,
        file_sha256       TEXT NOT NULL,
        bytes             INTEGER NOT NULL,
        provenance        TEXT NOT NULL,   -- Provenance (JSON)
        created_at        INTEGER NOT NULL
    );"];

/// Most items a list returns.
pub const MAX_PAGE: usize = 200;

const JOB_COLUMNS: &str = "id, name, status, spec, progress, model_id, error, worker, attempt,
    created_at, started_at, updated_at, finished_at";
const DATASET_COLUMNS: &str = "id, name, source, status, records, bytes, sha256, stats, job_id, error, created_at, updated_at";
const MODEL_COLUMNS: &str = "id, name, job_id, architecture, architecture_hash, file_sha256, bytes,
    provenance, created_at";

/// Current time, Unix milliseconds.
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn random_seed() -> u64 {
    RandomState::new().hash_one(now())
}

/// The lab's database and files.
#[derive(Debug)]
pub struct Lab {
    connection: Mutex<Connection>,
    paths: LabPaths,
    trainers: Vec<String>,
}

impl Lab {
    /// Opens (or creates) the database at `db`, its files under `paths`.
    ///
    /// # Errors
    ///
    /// When the database cannot be opened or migrated.
    pub fn open(db: &Path, paths: LabPaths) -> Result<Self, LabError> {
        if let Some(parent) = db.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(db)?;
        // WAL: readers do not wait for the writer.
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Self::with_connection(connection, paths)
    }

    /// An empty database in memory (tests), its files under `paths`.
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn in_memory(paths: LabPaths) -> Result<Self, LabError> {
        Self::with_connection(Connection::open_in_memory()?, paths)
    }

    fn with_connection(mut connection: Connection, paths: LabPaths) -> Result<Self, LabError> {
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let transaction = connection.transaction()?;
        for (version, migration) in (1..).zip(MIGRATIONS).skip_while(|(v, _)| *v <= version) {
            transaction.execute_batch(migration)?;
            transaction.pragma_update(None, "user_version", version)?;
        }
        transaction.commit()?;
        Ok(Self {
            connection: Mutex::new(connection),
            paths,
            trainers: vec![BURN.into()],
        })
    }

    /// Trainer backends jobs may ask for (default: `burn`).
    #[must_use]
    pub fn with_trainers(mut self, trainers: Vec<String>) -> Self {
        self.trainers = trainers;
        self
    }

    /// Where the files are.
    #[must_use]
    pub fn paths(&self) -> &LabPaths {
        &self.paths
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    // --- Datasets ---------------------------------------------------------

    /// Stores an uploaded dataset after checking every record (at most
    /// `max_bytes`).
    ///
    /// # Errors
    ///
    /// [`LabError::Invalid`] for a bad name, [`LabError::Dataset`] for a bad
    /// file (nothing is kept).
    pub fn import_dataset(
        &self,
        name: &str,
        source: impl Read,
        max_bytes: u64,
    ) -> Result<Dataset, LabError> {
        let name = check_name(name, "/name").map_err(|err| LabError::Invalid(vec![err]))?;
        let id = DatasetId::generate();
        let dir = self.paths.dataset_dir(id);
        let imported = chess_dataset::import(source, &dir.join("upload.bin"), max_bytes);
        let imported = match imported {
            Ok(imported) => imported,
            Err(err) => {
                let _ = std::fs::remove_dir(&dir);
                return Err(err.into());
            }
        };
        let now = now();
        let stats = serde_json::to_string(&imported.stats).map_err(json_error)?;
        self.lock().execute(
            "INSERT INTO datasets (id, name, source, status, records, bytes, sha256, stats,
                created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id.to_string(),
                name,
                DatasetSource::Upload.as_str(),
                DatasetStatus::Ready.as_str(),
                imported.records as i64,
                imported.bytes as i64,
                imported.sha256,
                stats,
                now
            ],
        )?;
        self.dataset(id)
    }

    /// A dataset.
    ///
    /// # Errors
    ///
    /// [`LabError::NotFound`].
    pub fn dataset(&self, id: DatasetId) -> Result<Dataset, LabError> {
        dataset_in(&self.lock(), id)
    }

    /// Datasets, newest first, older than `after` (a page of `limit`).
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn datasets(
        &self,
        after: Option<DatasetId>,
        limit: usize,
    ) -> Result<Vec<Dataset>, LabError> {
        list(
            &self.lock(),
            "datasets",
            DATASET_COLUMNS,
            after.map(|id| id.to_string()),
            limit,
            dataset_from_row,
        )
    }

    // --- Jobs -------------------------------------------------------------

    /// Queues a training job, after checking the architecture, datasets
    /// and hyperparameters; the job records everything needed to reproduce
    /// it (normalized architecture, dataset hashes, seed).
    ///
    /// # Errors
    ///
    /// [`LabError::Invalid`] with every problem found.
    pub fn create_training(&self, request: &TrainingRequest) -> Result<Job, LabError> {
        let mut errors = Vec::new();
        let architecture = match chess_nnue_spec::from_value(&request.architecture) {
            Ok(architecture) => Some(architecture.normalized()),
            Err(found) => {
                errors.extend(found.into_iter().map(|e| ValidationError {
                    path: format!("/architecture{}", e.path),
                    message: e.message,
                }));
                None
            }
        };
        errors.extend(request.hyperparameters.validate("/hyperparameters"));
        if !self.trainers.contains(&request.trainer) {
            errors.push(ValidationError {
                path: "/trainer".into(),
                message: format!(
                    "unknown trainer {:?}; available: {}",
                    request.trainer,
                    self.trainers.join(", ")
                ),
            });
        }
        // Without a name nor a valid architecture, the architecture's errors say it all.
        let name = request
            .name
            .clone()
            .or_else(|| architecture.as_ref().map(|a| a.metadata.name.clone()))
            .and_then(|name| {
                check_name(&name, "/name")
                    .map_err(|err| errors.push(err))
                    .ok()
            });

        let connection = self.lock();
        let datasets = resolve_datasets(&connection, &request.datasets, &mut errors);
        let positions: u64 = datasets.iter().map(|d| d.records).sum();
        if !datasets.is_empty() && positions < request.hyperparameters.batch_size as u64 {
            errors.push(ValidationError {
                path: "/hyperparameters/batchSize".into(),
                message: format!("larger than the {positions} positions of the datasets"),
            });
        }
        let (Some(architecture), Some(name), true) = (architecture, name, errors.is_empty()) else {
            return Err(LabError::Invalid(errors));
        };
        let spec = JobSpec::Training(TrainingJob {
            architecture_hash: architecture.hash().to_string(),
            architecture,
            datasets,
            hyperparameters: request.hyperparameters.clone(),
            seed: request.seed.unwrap_or_else(random_seed),
            device: request.device,
            trainer: request.trainer.clone(),
        });
        let id = insert_job(&connection, &name, &spec, request.device)?;
        job_in(&connection, id)
    }

    /// Queues a self-play job and creates the dataset it fills.
    ///
    /// # Errors
    ///
    /// [`LabError::Invalid`] with every problem found.
    pub fn create_selfplay(&self, request: &SelfplayRequest) -> Result<(Job, Dataset), LabError> {
        let mut errors = Vec::new();
        let mut check = |ok: bool, path: &str, message: &str| {
            if !ok {
                errors.push(ValidationError {
                    path: path.into(),
                    message: message.into(),
                });
            }
        };
        check(
            (1..=10_000_000).contains(&request.games),
            "/games",
            "must be between 1 and 10000000",
        );
        check(
            (1..=10_000_000).contains(&request.nodes),
            "/nodes",
            "must be between 1 and 10000000",
        );
        check(request.threads <= 256, "/threads", "must be at most 256");
        let name = request.name.clone().unwrap_or_else(|| {
            format!(
                "self-play, {} games at {} nodes",
                request.games, request.nodes
            )
        });
        let name = check_name(&name, "/name")
            .map_err(|err| errors.push(err))
            .ok();

        let mut connection = self.lock();
        if let Some(model) = request.network
            && model_in(&connection, model).is_err()
        {
            errors.push(ValidationError {
                path: "/network".into(),
                message: format!("no model {model}"),
            });
        }
        let (Some(name), true) = (name, errors.is_empty()) else {
            return Err(LabError::Invalid(errors));
        };
        let dataset = DatasetId::generate();
        let spec = JobSpec::Selfplay(SelfplayJob {
            dataset,
            games: request.games,
            nodes: request.nodes,
            seed: request.seed.unwrap_or_else(random_seed),
            network: request.network,
            threads: request.threads,
        });
        let transaction = connection.transaction()?;
        let job = insert_job(&transaction, &name, &spec, Device::Cpu)?;
        let now = now();
        transaction.execute(
            "INSERT INTO datasets (id, name, source, status, job_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![
                dataset.to_string(),
                name,
                DatasetSource::Selfplay.as_str(),
                DatasetStatus::Generating.as_str(),
                job.to_string(),
                now
            ],
        )?;
        transaction.commit()?;
        Ok((job_in(&connection, job)?, dataset_in(&connection, dataset)?))
    }

    /// A job.
    ///
    /// # Errors
    ///
    /// [`LabError::NotFound`].
    pub fn job(&self, id: JobId) -> Result<Job, LabError> {
        job_in(&self.lock(), id)
    }

    /// Jobs, newest first, older than `after` (a page of `limit`).
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn jobs(&self, after: Option<JobId>, limit: usize) -> Result<Vec<Job>, LabError> {
        list(
            &self.lock(),
            "jobs",
            JOB_COLUMNS,
            after.map(|id| id.to_string()),
            limit,
            job_from_row,
        )
    }

    /// Events of a job after `after_seq` (0: from the start), oldest first.
    ///
    /// # Errors
    ///
    /// [`LabError::NotFound`] for an unknown job.
    pub fn events(
        &self,
        id: JobId,
        after_seq: i64,
        limit: usize,
    ) -> Result<Vec<JobEvent>, LabError> {
        let connection = self.lock();
        job_in(&connection, id)?;
        let mut statement = connection.prepare_cached(
            "SELECT seq, job_id, time, kind, data FROM job_events
             WHERE job_id = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
        )?;
        let events = statement
            .query_map(
                params![id.to_string(), after_seq, limit.min(10_000) as i64],
                |row| {
                    Ok(JobEvent {
                        seq: row.get(0)?,
                        job: parse_id(row, 1)?,
                        time: row.get(2)?,
                        kind: parse_enum(row, 3, EventKind::parse)?,
                        data: parse_json(row, 4)?,
                    })
                },
            )?
            .collect::<Result<_, _>>()?;
        Ok(events)
    }

    /// Stops a job: a queued one is canceled, a running one asked to save
    /// a checkpoint and stop.
    ///
    /// # Errors
    ///
    /// [`LabError::Conflict`] when the job is not queued, running or stopping.
    pub fn stop(&self, id: JobId) -> Result<Job, LabError> {
        let mut connection = self.lock();
        let job = job_in(&connection, id)?;
        let now = now();
        let transaction = connection.transaction()?;
        match job.status {
            JobStatus::Queued => {
                set_status(&transaction, &job, JobStatus::Canceled, None, now)?;
                fail_dataset(&transaction, &job.spec, "canceled", now)?;
            }
            JobStatus::Running => set_status(&transaction, &job, JobStatus::Stopping, None, now)?,
            JobStatus::Stopping => {}
            status => {
                return Err(LabError::Conflict(format!(
                    "job {id} is {}: only queued or running jobs can be stopped",
                    status.as_str()
                )));
            }
        }
        transaction.commit()?;
        job_in(&connection, id)
    }

    /// Queues a stopped, interrupted or failed training job again; its
    /// worker resumes from the last checkpoint.
    ///
    /// # Errors
    ///
    /// [`LabError::Conflict`] for self-play jobs and jobs in other states.
    pub fn resume(&self, id: JobId) -> Result<Job, LabError> {
        let mut connection = self.lock();
        let job = job_in(&connection, id)?;
        if job.spec.kind() != JobKind::Training {
            return Err(LabError::Conflict(format!(
                "job {id} is a self-play job: they cannot be resumed, create a new one"
            )));
        }
        if !matches!(
            job.status,
            JobStatus::Stopped | JobStatus::Interrupted | JobStatus::Failed
        ) {
            return Err(LabError::Conflict(format!(
                "job {id} is {}: only stopped, interrupted or failed jobs can be resumed",
                job.status.as_str()
            )));
        }
        let transaction = connection.transaction()?;
        set_status(&transaction, &job, JobStatus::Queued, None, now())?;
        transaction.commit()?;
        job_in(&connection, id)
    }

    /// Marks `interrupted` the jobs whose worker has not reported since
    /// `stale_before` (Unix milliseconds); returns them.
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn reap(&self, stale_before: i64) -> Result<Vec<JobId>, LabError> {
        let mut connection = self.lock();
        let stale: Vec<Job> = {
            let mut statement = connection.prepare_cached(&format!(
                "SELECT {JOB_COLUMNS} FROM jobs
                 WHERE status IN ('running', 'stopping') AND heartbeat_at < ?1"
            ))?;
            statement
                .query_map([stale_before], job_from_row)?
                .collect::<Result<_, _>>()?
        };
        let now = now();
        let transaction = connection.transaction()?;
        for job in &stale {
            set_status(
                &transaction,
                job,
                JobStatus::Interrupted,
                Some("worker stopped reporting"),
                now,
            )?;
            fail_dataset(&transaction, &job.spec, "interrupted", now)?;
        }
        transaction.commit()?;
        Ok(stale.iter().map(|job| job.id).collect())
    }

    /// [`Lab::reap`] of the jobs whose worker has been silent for `silence`.
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn reap_silent(&self, silence: std::time::Duration) -> Result<Vec<JobId>, LabError> {
        self.reap(now() - silence.as_millis() as i64)
    }

    // --- Models -----------------------------------------------------------

    /// A model.
    ///
    /// # Errors
    ///
    /// [`LabError::NotFound`].
    pub fn model(&self, id: ModelId) -> Result<Model, LabError> {
        model_in(&self.lock(), id)
    }

    /// Models, newest first, older than `after` (a page of `limit`).
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn models(&self, after: Option<ModelId>, limit: usize) -> Result<Vec<Model>, LabError> {
        list(
            &self.lock(),
            "models",
            MODEL_COLUMNS,
            after.map(|id| id.to_string()),
            limit,
            model_from_row,
        )
    }

    /// The job behind `claim`, if the claim still holds.
    fn claimed(&self, claim: ClaimToken) -> Result<Job, LabError> {
        claimed_in(&self.lock(), claim)
    }
}

impl Broker for Lab {
    fn claim(&self, worker: &WorkerInfo) -> Result<Option<Claim>, LabError> {
        let mut connection = self.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let gpu_busy: bool = transaction.query_row(
            "SELECT EXISTS (SELECT 1 FROM jobs WHERE gpu = 1 AND status IN ('running', 'stopping'))",
            [],
            |row| row.get(0),
        )?;
        let queued: Vec<Job> = {
            let mut statement = transaction.prepare_cached(&format!(
                "SELECT {JOB_COLUMNS} FROM jobs WHERE status = 'queued' ORDER BY id"
            ))?;
            statement
                .query_map([], job_from_row)?
                .collect::<Result<_, _>>()?
        };
        let chosen = queued.into_iter().find_map(|job| {
            if !worker.kinds.contains(&job.spec.kind()) {
                return None;
            }
            match &job.spec {
                JobSpec::Training(training) => {
                    let gpu = worker.gpu && training.device != Device::Cpu;
                    let runnable = worker.trainers.contains(&training.trainer)
                        && (worker.gpu || training.device != Device::Gpu)
                        && !(gpu && gpu_busy);
                    runnable.then_some((job, gpu))
                }
                JobSpec::Selfplay(_) => Some((job, false)),
            }
        });
        let Some((job, gpu)) = chosen else {
            return Ok(None);
        };
        let now = now();
        let attempt = job.attempt + 1;
        transaction.execute(
            "UPDATE jobs SET status = 'running', attempt = ?2, worker = ?3, gpu = ?4,
                started_at = COALESCE(started_at, ?5), heartbeat_at = ?5, updated_at = ?5,
                error = NULL
             WHERE id = ?1",
            params![job.id.to_string(), attempt, worker.id, gpu, now],
        )?;
        append_event(
            &transaction,
            job.id,
            EventKind::Status,
            &json!({ "status": "running", "attempt": attempt, "worker": worker.id, "gpu": gpu }),
            now,
        )?;
        transaction.commit()?;
        Ok(Some(Claim {
            job: job_in(&connection, job.id)?,
            attempt,
            gpu,
        }))
    }

    fn heartbeat(&self, claim: ClaimToken) -> Result<Directive, LabError> {
        let connection = self.lock();
        let job = claimed_in(&connection, claim)?;
        connection.execute(
            "UPDATE jobs SET heartbeat_at = ?2 WHERE id = ?1",
            params![job.id.to_string(), now()],
        )?;
        Ok(directive(job.status))
    }

    fn progress(&self, claim: ClaimToken, progress: &Progress) -> Result<Directive, LabError> {
        let mut connection = self.lock();
        let job = claimed_in(&connection, claim)?;
        let now = now();
        let data = serde_json::to_value(progress).map_err(json_error)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE jobs SET progress = ?2, heartbeat_at = ?3, updated_at = ?3 WHERE id = ?1",
            params![job.id.to_string(), data.to_string(), now],
        )?;
        append_event(&transaction, job.id, EventKind::Progress, &data, now)?;
        transaction.commit()?;
        Ok(directive(job.status))
    }

    fn event(&self, claim: ClaimToken, kind: EventKind, data: Value) -> Result<(), LabError> {
        let connection = self.lock();
        let job = claimed_in(&connection, claim)?;
        append_event(&connection, job.id, kind, &data, now())
    }

    fn complete_training(
        &self,
        claim: ClaimToken,
        network: &[u8],
        summary: &TrainingSummary,
    ) -> Result<Model, LabError> {
        let job = self.claimed(claim)?;
        let JobSpec::Training(training) = &job.spec else {
            return Err(LabError::Conflict(format!(
                "job {} is not a training job",
                job.id
            )));
        };
        check_network(network, &training.architecture_hash)?;

        // Write the file first, outside the lock: a model row always has its file.
        let id = ModelId::generate();
        let file = self.paths.model_file(id);
        write_atomically(&file, network)?;
        let file_sha256 = hex(&Sha256::digest(network));
        let provenance = Provenance {
            trainer: training.trainer.clone(),
            seed: training.seed,
            datasets: training.datasets.clone(),
            hyperparameters: training.hyperparameters.clone(),
            superbatches: summary.superbatches,
            positions: summary.positions,
            final_loss: summary.final_loss,
            device: summary.device.clone(),
        };

        let commit = || -> Result<Model, LabError> {
            let mut connection = self.lock();
            let job = claimed_in(&connection, claim)?;
            let now = now();
            let transaction = connection.transaction()?;
            transaction.execute(
                &format!("INSERT INTO models ({MODEL_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"),
                params![
                    id.to_string(),
                    job.name,
                    job.id.to_string(),
                    to_json(&training.architecture)?,
                    training.architecture_hash,
                    file_sha256,
                    network.len() as i64,
                    to_json(&provenance)?,
                    now
                ],
            )?;
            transaction.execute(
                "UPDATE jobs SET model_id = ?2 WHERE id = ?1",
                params![job.id.to_string(), id.to_string()],
            )?;
            append_event(
                &transaction,
                job.id,
                EventKind::Model,
                &json!({ "modelId": id }),
                now,
            )?;
            set_status(&transaction, &job, JobStatus::Succeeded, None, now)?;
            transaction.commit()?;
            model_in(&connection, id)
        };
        commit().inspect_err(|_| {
            let _ = std::fs::remove_file(&file);
        })
    }

    fn complete_selfplay(&self, claim: ClaimToken) -> Result<Dataset, LabError> {
        let job = self.claimed(claim)?;
        let JobSpec::Selfplay(selfplay) = &job.spec else {
            return Err(LabError::Conflict(format!(
                "job {} is not a self-play job",
                job.id
            )));
        };
        // Hash the files outside the lock (a large dataset takes seconds).
        let scanned = chess_dataset::scan(&self.paths.dataset_files(selfplay.dataset))?;

        let mut connection = self.lock();
        let job = claimed_in(&connection, claim)?;
        let now = now();
        let transaction = connection.transaction()?;
        if scanned.records > 0 {
            transaction.execute(
                "UPDATE datasets SET status = 'ready', records = ?2, bytes = ?3, sha256 = ?4,
                    stats = ?5, updated_at = ?6
                 WHERE id = ?1",
                params![
                    selfplay.dataset.to_string(),
                    scanned.records as i64,
                    scanned.bytes as i64,
                    scanned.sha256,
                    to_json(&scanned.stats)?,
                    now
                ],
            )?;
        } else {
            fail_dataset(&transaction, &job.spec, "no position was generated", now)?;
        }
        let status = if job.status == JobStatus::Stopping {
            JobStatus::Stopped
        } else {
            JobStatus::Succeeded
        };
        set_status(&transaction, &job, status, None, now)?;
        transaction.commit()?;
        dataset_in(&connection, selfplay.dataset)
    }

    fn stopped(&self, claim: ClaimToken) -> Result<(), LabError> {
        let mut connection = self.lock();
        let job = claimed_in(&connection, claim)?;
        let status = if job.status == JobStatus::Stopping {
            JobStatus::Stopped
        } else {
            JobStatus::Interrupted
        };
        let now = now();
        let transaction = connection.transaction()?;
        set_status(&transaction, &job, status, None, now)?;
        fail_dataset(&transaction, &job.spec, "stopped before completion", now)?;
        transaction.commit()?;
        Ok(())
    }

    fn fail(&self, claim: ClaimToken, message: &str) -> Result<(), LabError> {
        let mut connection = self.lock();
        let job = claimed_in(&connection, claim)?;
        let now = now();
        let transaction = connection.transaction()?;
        set_status(&transaction, &job, JobStatus::Failed, Some(message), now)?;
        fail_dataset(&transaction, &job.spec, message, now)?;
        transaction.commit()?;
        Ok(())
    }
}

fn directive(status: JobStatus) -> Directive {
    if status == JobStatus::Stopping {
        Directive::Stop
    } else {
        Directive::Continue
    }
}

/// Checks that `network` is a valid version 2 network of the architecture
/// whose hash is `expected`.
fn check_network(network: &[u8], expected: &str) -> Result<(), LabError> {
    let info = chess_nnue::inspect(network)
        .map_err(|err| LabError::invalid("/network", err.to_string()))?;
    if info.version != chess_nnue::FORMAT_VERSION {
        return Err(LabError::invalid(
            "/network",
            format!(
                "network format version {}, expected {}",
                info.version,
                chess_nnue::FORMAT_VERSION
            ),
        ));
    }
    let network = chess_nnue::Network::from_bytes(network)
        .map_err(|err| LabError::invalid("/network", err.to_string()))?;
    if network.hash().to_string() != expected {
        return Err(LabError::invalid(
            "/network",
            format!(
                "network of architecture {}, the job trains {expected}",
                network.hash()
            ),
        ));
    }
    Ok(())
}

/// Checks a name: 1 to 100 characters once trimmed, no control character.
fn check_name(name: &str, path: &str) -> Result<String, ValidationError> {
    let name = name.trim();
    let error = |message: &str| ValidationError {
        path: path.into(),
        message: message.into(),
    };
    if name.is_empty() || name.chars().count() > 100 {
        return Err(error("must have 1 to 100 characters"));
    }
    if name.chars().any(char::is_control) {
        return Err(error("must not contain control characters"));
    }
    Ok(name.to_owned())
}

/// The datasets of a training request, as references; problems go to `errors`.
fn resolve_datasets(
    connection: &Connection,
    ids: &[DatasetId],
    errors: &mut Vec<ValidationError>,
) -> Vec<DatasetRef> {
    let mut error = |path: String, message: String| errors.push(ValidationError { path, message });
    if ids.is_empty() || ids.len() > 64 {
        error("/datasets".into(), "must list 1 to 64 datasets".into());
    }
    let mut refs: Vec<DatasetRef> = Vec::with_capacity(ids.len());
    for (i, &id) in ids.iter().enumerate() {
        let path = format!("/datasets/{i}");
        if refs.iter().any(|r| r.id == id) {
            error(path, format!("dataset {id} is listed twice"));
            continue;
        }
        match dataset_in(connection, id) {
            Ok(Dataset {
                status: DatasetStatus::Ready,
                sha256: Some(sha256),
                records,
                ..
            }) => refs.push(DatasetRef {
                id,
                sha256,
                records,
            }),
            Ok(dataset) => error(
                path,
                format!("dataset {id} is {}, not ready", dataset.status.as_str()),
            ),
            Err(_) => error(path, format!("no dataset {id}")),
        }
    }
    refs
}

fn insert_job(
    connection: &Connection,
    name: &str,
    spec: &JobSpec,
    device: Device,
) -> Result<JobId, LabError> {
    let id = JobId::generate();
    let now = now();
    connection.execute(
        "INSERT INTO jobs (id, kind, name, status, spec, device, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6, ?6)",
        params![
            id.to_string(),
            spec.kind().as_str(),
            name,
            to_json(spec)?,
            device.as_str(),
            now
        ],
    )?;
    append_event(
        connection,
        id,
        EventKind::Status,
        &json!({ "status": "queued", "attempt": 0 }),
        now,
    )?;
    Ok(id)
}

/// Moves `job` to `status`, releasing the GPU slot unless it stays active,
/// and records the change.
fn set_status(
    connection: &Connection,
    job: &Job,
    status: JobStatus,
    error: Option<&str>,
    now: i64,
) -> Result<(), LabError> {
    connection.execute(
        "UPDATE jobs SET status = ?2, error = ?3, updated_at = ?4,
            gpu = CASE WHEN ?5 THEN gpu ELSE 0 END,
            finished_at = CASE WHEN ?6 THEN ?4 ELSE NULL END
         WHERE id = ?1",
        params![
            job.id.to_string(),
            status.as_str(),
            error,
            now,
            status.is_active(),
            status.is_final()
        ],
    )?;
    let mut data = json!({ "status": status.as_str(), "attempt": job.attempt });
    if let Some(error) = error {
        data["error"] = error.into();
    }
    append_event(connection, job.id, EventKind::Status, &data, now)
}

/// Marks the dataset of a self-play job as failed (no-op for training jobs).
fn fail_dataset(
    connection: &Connection,
    spec: &JobSpec,
    reason: &str,
    now: i64,
) -> Result<(), LabError> {
    if let JobSpec::Selfplay(selfplay) = spec {
        connection.execute(
            "UPDATE datasets SET status = 'failed', error = ?2, updated_at = ?3
             WHERE id = ?1 AND status = 'generating'",
            params![selfplay.dataset.to_string(), reason, now],
        )?;
    }
    Ok(())
}

fn append_event(
    connection: &Connection,
    job: JobId,
    kind: EventKind,
    data: &Value,
    now: i64,
) -> Result<(), LabError> {
    connection.execute(
        "INSERT INTO job_events (job_id, time, kind, data) VALUES (?1, ?2, ?3, ?4)",
        params![job.to_string(), now, kind.as_str(), data.to_string()],
    )?;
    Ok(())
}

fn claimed_in(connection: &Connection, claim: ClaimToken) -> Result<Job, LabError> {
    match job_in(connection, claim.job) {
        Ok(job) if job.status.is_active() && job.attempt == claim.attempt => Ok(job),
        Ok(_) | Err(LabError::NotFound { .. }) => Err(LabError::Lost(claim.job)),
        Err(err) => Err(err),
    }
}

fn job_in(connection: &Connection, id: JobId) -> Result<Job, LabError> {
    get(
        connection,
        "jobs",
        JOB_COLUMNS,
        "job",
        &id.to_string(),
        job_from_row,
    )
}

fn dataset_in(connection: &Connection, id: DatasetId) -> Result<Dataset, LabError> {
    get(
        connection,
        "datasets",
        DATASET_COLUMNS,
        "dataset",
        &id.to_string(),
        dataset_from_row,
    )
}

fn model_in(connection: &Connection, id: ModelId) -> Result<Model, LabError> {
    get(
        connection,
        "models",
        MODEL_COLUMNS,
        "model",
        &id.to_string(),
        model_from_row,
    )
}

fn get<T>(
    connection: &Connection,
    table: &str,
    columns: &str,
    what: &'static str,
    id: &str,
    decode: fn(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<T, LabError> {
    connection
        .prepare_cached(&format!("SELECT {columns} FROM {table} WHERE id = ?1"))?
        .query_row([id], decode)
        .optional()?
        .ok_or_else(|| LabError::NotFound {
            what,
            id: id.to_owned(),
        })
}

/// A page of `table`, newest first, older than `after`.
fn list<T>(
    connection: &Connection,
    table: &str,
    columns: &str,
    after: Option<String>,
    limit: usize,
    decode: fn(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, LabError> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT {columns} FROM {table} WHERE ?1 IS NULL OR id < ?1 ORDER BY id DESC LIMIT ?2"
    ))?;
    let items = statement
        .query_map(params![after, limit.clamp(1, MAX_PAGE) as i64], decode)?
        .collect::<Result<_, _>>()?;
    Ok(items)
}

fn job_from_row(row: &Row<'_>) -> rusqlite::Result<Job> {
    Ok(Job {
        id: parse_id(row, 0)?,
        name: row.get(1)?,
        status: parse_enum(row, 2, JobStatus::parse)?,
        spec: parse_json(row, 3)?,
        progress: parse_optional_json(row, 4)?,
        model: parse_optional_id(row, 5)?,
        error: row.get(6)?,
        worker: row.get(7)?,
        attempt: row.get(8)?,
        created_at: row.get(9)?,
        started_at: row.get(10)?,
        updated_at: row.get(11)?,
        finished_at: row.get(12)?,
    })
}

fn dataset_from_row(row: &Row<'_>) -> rusqlite::Result<Dataset> {
    Ok(Dataset {
        id: parse_id(row, 0)?,
        name: row.get(1)?,
        source: parse_enum(row, 2, DatasetSource::parse)?,
        status: parse_enum(row, 3, DatasetStatus::parse)?,
        records: row.get::<_, i64>(4)? as u64,
        bytes: row.get::<_, i64>(5)? as u64,
        sha256: row.get(6)?,
        stats: parse_optional_json(row, 7)?,
        job: parse_optional_id(row, 8)?,
        error: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn model_from_row(row: &Row<'_>) -> rusqlite::Result<Model> {
    Ok(Model {
        id: parse_id(row, 0)?,
        name: row.get(1)?,
        job: parse_optional_id(row, 2)?,
        architecture: parse_json(row, 3)?,
        architecture_hash: row.get(4)?,
        file_sha256: row.get(5)?,
        bytes: row.get::<_, i64>(6)? as u64,
        provenance: parse_json(row, 7)?,
        created_at: row.get(8)?,
    })
}

fn conversion_error(
    index: usize,
    err: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(err))
}

fn parse_id<T: std::str::FromStr<Err = uuid::Error>>(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<T> {
    row.get::<_, String>(index)?
        .parse()
        .map_err(|err| conversion_error(index, err))
}

fn parse_optional_id<T: std::str::FromStr<Err = uuid::Error>>(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<T>> {
    row.get::<_, Option<String>>(index)?
        .map(|text| text.parse().map_err(|err| conversion_error(index, err)))
        .transpose()
}

fn parse_enum<T>(row: &Row<'_>, index: usize, parse: fn(&str) -> Option<T>) -> rusqlite::Result<T> {
    let text: String = row.get(index)?;
    parse(&text).ok_or_else(|| {
        conversion_error(
            index,
            std::io::Error::other(format!("unknown value {text:?}")),
        )
    })
}

fn parse_json<T: DeserializeOwned>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    serde_json::from_str(&row.get::<_, String>(index)?).map_err(|err| conversion_error(index, err))
}

fn parse_optional_json<T: DeserializeOwned>(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<T>> {
    row.get::<_, Option<String>>(index)?
        .map(|text| serde_json::from_str(&text).map_err(|err| conversion_error(index, err)))
        .transpose()
}

fn to_json(value: &impl Serialize) -> Result<String, LabError> {
    serde_json::to_string(value).map_err(json_error)
}

fn json_error(err: serde_json::Error) -> LabError {
    LabError::Io(std::io::Error::other(err))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes `bytes` to `path` through a temporary file.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), LabError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    Ok(())
}
