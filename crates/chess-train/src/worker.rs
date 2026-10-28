//! Lab worker: takes the jobs of the NNUE lab's queue (a
//! [`chess_jobs::Broker`]) one at a time and runs them — training with a
//! [`Trainer`], self-play into the job's dataset, evaluation matches in the
//! arena — reporting progress and heartbeats, stopping (after a
//! checkpoint) when asked to.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use chess_eval::EngineEval;
use chess_jobs::{
    Broker, Claim, ClaimToken, Contestant, DEFAULT_MATCH_NODES, Directive, EvaluationJob,
    EvaluationProgress, EventKind, JobId, JobSpec, LabError, LabPaths, MatchResult, Progress,
    SelfplayJob, SelfplayProgress, TrainingJob, TrainingProgress, TrainingSummary, WorkerInfo,
};
use chess_search::StopFlag;
use serde_json::json;

use crate::arena::{self, ArenaConfig, MoveLimit};
use crate::device::{self, DevicePreference, SelectedDevice};
use crate::selfplay::{self, SelfplayConfig, SelfplayStats};
use crate::trainer::{
    BurnTrainer, Observer, SuperbatchReport, TrainConfig, TrainState, Trainer, TrainingRun,
};

/// A worker and what it runs jobs with.
pub(crate) struct Worker<'a> {
    pub(crate) broker: &'a dyn Broker,
    pub(crate) paths: LabPaths,
    pub(crate) info: WorkerInfo,
    /// The GPU, when the worker has one.
    pub(crate) gpu: Option<&'a SelectedDevice>,
    /// Time between two heartbeats (and self-play progress reports).
    pub(crate) heartbeat: Duration,
    /// Self-play settings the jobs do not choose (hash, chunk size, openings).
    pub(crate) selfplay: SelfplayConfig,
}

impl Worker<'_> {
    /// Runs jobs until `stop` is raised, polling every `poll` when the queue
    /// is empty (`idle` runs then) or unreachable (a restarting server). With
    /// `once`, returns after one job, or at once when none is queued.
    pub(crate) fn run(
        &self,
        stop: &StopFlag,
        poll: Duration,
        once: bool,
        mut idle: impl FnMut(),
    ) -> Result<()> {
        tracing::info!(worker = %self.info.id, gpu = self.info.gpu, "lab worker started");
        while !stop.is_stopped() {
            match self.run_one(stop) {
                Ok(Some(_)) if once => return Ok(()),
                Ok(Some(_)) => continue,
                Ok(None) if once => return Ok(()),
                Ok(None) => idle(),
                Err(err) if once => return Err(err),
                Err(err) => {
                    tracing::warn!(error = %format!("{err:#}"), "queue unreachable, retrying")
                }
            }
            let until = Instant::now() + poll;
            while !stop.is_stopped() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        Ok(())
    }

    /// Takes the next job this worker can run and runs it.
    ///
    /// # Errors
    ///
    /// Only when the queue cannot be reached: a failing job is reported as
    /// failed and the worker goes on.
    pub(crate) fn run_one(&self, stop: &StopFlag) -> Result<Option<JobId>> {
        let Some(claim) = self.broker.claim(&self.info)? else {
            return Ok(None);
        };
        let token = claim.token();
        tracing::info!(job = %token.job, attempt = token.attempt, gpu = claim.gpu, name = %claim.job.name, "job started");
        let job = JobRun {
            broker: self.broker,
            token,
            stop: StopFlag::new(),
            requested: AtomicBool::new(false),
            lost: AtomicBool::new(false),
        };
        let done = StopFlag::new();
        let stats = SelfplayStats::default();
        let result = std::thread::scope(|scope| {
            let heartbeats = scope.spawn(|| self.heartbeats(&job, stop, &done, &claim, &stats));
            let result = match &claim.job.spec {
                JobSpec::Training(training) => self.train(&claim, training, &job),
                JobSpec::Selfplay(selfplay) => self.selfplay(selfplay, &job, &stats),
                JobSpec::Evaluation(evaluation) => self.evaluate(evaluation, &job),
            };
            done.stop();
            let _ = heartbeats.join();
            result
        });
        match result {
            Ok(()) => tracing::info!(job = %token.job, "job done"),
            Err(_) if job.lost() => {
                tracing::warn!(job = %token.job, "job taken away from this worker");
            }
            Err(err) => {
                let message = format!("{err:#}");
                tracing::error!(job = %token.job, error = %message, "job failed");
                if let Err(err) = self.broker.fail(token, &message) {
                    tracing::warn!(job = %token.job, error = %err, "cannot report the failure");
                }
            }
        }
        Ok(Some(token.job))
    }

    /// Heartbeats (with progress, for self-play) until `done`; raises the
    /// job's stop flag when the worker stops, the job is stopped or lost.
    fn heartbeats(
        &self,
        job: &JobRun<'_>,
        stop: &StopFlag,
        done: &StopFlag,
        claim: &Claim,
        stats: &SelfplayStats,
    ) {
        let mut last = Instant::now();
        while !done.is_stopped() {
            if stop.is_stopped() {
                job.stop.stop();
            }
            if last.elapsed() >= self.heartbeat {
                last = Instant::now();
                let directive = match &claim.job.spec {
                    JobSpec::Selfplay(selfplay) => self
                        .broker
                        .progress(job.token, &selfplay_progress(selfplay, stats)),
                    JobSpec::Training(_) | JobSpec::Evaluation(_) => {
                        self.broker.heartbeat(job.token)
                    }
                };
                job.obey(directive);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn train(&self, claim: &Claim, job: &TrainingJob, run: &JobRun<'_>) -> Result<()> {
        let cpu;
        let device = if claim.gpu {
            self.gpu
                .context("job given the GPU, but this worker has none")?
        } else {
            cpu = device::select(DevicePreference::Cpu)?;
            &cpu
        };
        let files: Vec<PathBuf> = job
            .datasets
            .iter()
            .flat_map(|dataset| self.paths.dataset_files(dataset.id))
            .collect();
        ensure!(!files.is_empty(), "the job's datasets have no file");
        let h = &job.hyperparameters;
        let config = TrainConfig {
            architecture: None,
            batch_size: h.batch_size,
            batches_per_superbatch: h.batches_per_superbatch,
            superbatches: h.superbatches,
            learning_rate: h.learning_rate,
            lr_drop_every: h.lr_drop_every,
            lr_drop_factor: h.lr_drop_factor,
            wdl: h.wdl,
            score_scale: h.score_scale,
            shuffle_records: h.shuffle_records,
            export_every: 0,
            seed: job.seed,
        };
        let training = TrainingRun {
            architecture: job.architecture.clone(),
            config,
            files: Box::new(move || files.clone()),
            checkpoint: self.paths.run_dir(claim.job.id).join("checkpoint"),
            resume: true,
            stop: run.stop.clone(),
        };
        let trainer = BurnTrainer { device };
        ensure!(
            trainer.id() == job.trainer,
            "this worker has no {} trainer",
            job.trainer
        );
        let outcome = trainer.train(training, &JobObserver { run })?;
        if run.lost() {
            return Ok(());
        }
        match outcome.network {
            Some(network) if outcome.finished => {
                let summary = TrainingSummary {
                    superbatches: outcome.state.superbatch,
                    positions: outcome.state.positions,
                    final_loss: outcome.loss,
                    device: device.info.to_string(),
                };
                let model = self
                    .broker
                    .complete_training(run.token, &network, &summary)?;
                tracing::info!(job = %run.token.job, model = %model.id, "model stored");
            }
            // Stopped (by request or shutdown) after its last checkpoint.
            _ => self.broker.stopped(run.token)?,
        }
        Ok(())
    }

    fn selfplay(&self, job: &SelfplayJob, run: &JobRun<'_>, stats: &SelfplayStats) -> Result<()> {
        let network = job.network.map(|model| self.paths.model_file(model));
        if let Some(path) = &network {
            ensure!(path.exists(), "network {} not found", path.display());
        }
        let config = SelfplayConfig {
            games: job.games,
            nodes: job.nodes,
            threads: job.threads,
            seed: job.seed,
            network,
            ..self.selfplay.clone()
        };
        let dir = self.paths.dataset_dir(job.dataset);
        selfplay::run(&config, &dir, "selfplay", &run.stop, stats, |_| {})?;
        if run.lost() {
            return Ok(());
        }
        if run.stop.is_stopped() && !run.requested.load(Ordering::Relaxed) {
            // The worker is shutting down: the games are not all played.
            self.broker.stopped(run.token)?;
            return Ok(());
        }
        run.obey(
            self.broker
                .progress(run.token, &selfplay_progress(job, stats)),
        );
        let dataset = self.broker.complete_selfplay(run.token)?;
        tracing::info!(dataset = %dataset.id, positions = dataset.records, "dataset ready");
        Ok(())
    }
}

impl Worker<'_> {
    /// Plays the match of an evaluation job, after measuring the speed of
    /// both engines on this machine; reports the result after each batch
    /// of games.
    fn evaluate(&self, job: &EvaluationJob, run: &JobRun<'_>) -> Result<()> {
        let load = |contestant: &Contestant| -> Result<EngineEval> {
            let Some(model) = contestant.model else {
                return Ok(EngineEval::default());
            };
            let path = self.paths.model_file(model);
            let bytes = std::fs::read(&path)
                .with_context(|| format!("network {} not found", path.display()))?;
            EngineEval::from_nnue_bytes(&bytes)
                .with_context(|| format!("model {model} is not a valid network"))
        };
        let (candidate, baseline) = (load(&job.candidate)?, load(&job.baseline)?);
        // Before the match, which takes every core it is given.
        let candidate_nps = arena::measure_nps(&candidate, NPS_NODES);
        let baseline_nps = arena::measure_nps(&baseline, NPS_NODES);
        tracing::info!(job = %run.token.job, candidate_nps, baseline_nps, "speeds measured");
        let progress = |result: MatchResult| EvaluationProgress {
            candidate_nps: Some(candidate_nps),
            baseline_nps: Some(baseline_nps),
            ..EvaluationProgress::new(result, job.games, job.sprt.as_ref())
        };
        let report = |result: MatchResult| {
            run.obey(
                self.broker
                    .progress(run.token, &Progress::Evaluation(progress(result))),
            );
        };
        report(MatchResult::default());

        let sprt = job.sprt.unwrap_or_default();
        let config = ArenaConfig {
            games: job.games,
            limit: Some(match job.move_time_ms {
                Some(ms) => MoveLimit::Time(ms),
                None => MoveLimit::Nodes(job.nodes.unwrap_or(DEFAULT_MATCH_NODES)),
            }),
            threads: job.threads,
            seed: job.seed,
            sprt: job.sprt.is_some(),
            elo0: sprt.elo0,
            elo1: sprt.elo1,
            alpha: sprt.alpha,
            beta: sprt.beta,
            ..ArenaConfig::default()
        };
        let result = arena::play_match(
            &arena::Contestant::Engine(candidate),
            &arena::Contestant::Engine(baseline),
            &config,
            &run.stop,
            report,
        )?;
        if run.lost() {
            return Ok(());
        }
        if run.stop.is_stopped() && !run.requested.load(Ordering::Relaxed) {
            // The worker is shutting down: the games are not all played.
            self.broker.stopped(run.token)?;
            return Ok(());
        }
        let done = self
            .broker
            .complete_evaluation(run.token, &progress(result))?;
        tracing::info!(
            job = %done.id,
            games = result.games(),
            elo = format!("{:+.1}", result.elo()),
            error = format!("±{:.1}", result.elo_error()),
            "evaluation done"
        );
        Ok(())
    }
}

/// Nodes per bench position when measuring the speed of an engine.
const NPS_NODES: u64 = 20_000;

fn selfplay_progress(job: &SelfplayJob, stats: &SelfplayStats) -> Progress {
    Progress::Selfplay(SelfplayProgress {
        games: stats.games.load(Ordering::Relaxed),
        games_total: job.games,
        positions: stats.positions.load(Ordering::Relaxed),
    })
}

/// A job being run: its claim and how it ends early.
struct JobRun<'a> {
    broker: &'a dyn Broker,
    token: ClaimToken,
    /// Raised to stop the job (worker stopping, stop requested, job lost).
    stop: StopFlag,
    /// The queue asked for the stop.
    requested: AtomicBool,
    /// The job was taken away: nothing more is reported.
    lost: AtomicBool,
}

impl JobRun<'_> {
    fn lost(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
    }

    /// Follows the queue's answer to a report.
    fn obey(&self, answer: Result<Directive, LabError>) {
        match answer {
            Ok(Directive::Continue) => {}
            Ok(Directive::Stop) => {
                self.requested.store(true, Ordering::Relaxed);
                self.stop.stop();
            }
            Err(LabError::Lost(_)) => {
                self.lost.store(true, Ordering::Relaxed);
                self.stop.stop();
            }
            // The queue is unreachable for now: keep going, report later.
            Err(err) => tracing::warn!(job = %self.token.job, error = %err, "cannot report"),
        }
    }
}

/// Reports a training to the queue.
struct JobObserver<'a, 'b> {
    run: &'a JobRun<'b>,
}

impl Observer for JobObserver<'_, '_> {
    fn started(&self, resumed: Option<TrainState>) {
        if let Some(state) = resumed {
            let message = format!(
                "resumed from the checkpoint of superbatch {}",
                state.superbatch
            );
            self.event(EventKind::Log, json!({ "message": message }));
        }
    }

    fn superbatch(&self, report: &SuperbatchReport) {
        let state = report.state;
        let progress = Progress::Training(TrainingProgress {
            superbatch: state.superbatch,
            superbatches: report.superbatches,
            step: state.step,
            positions: state.positions,
            loss: report.loss,
            learning_rate: report.learning_rate,
            positions_per_second: report.positions_per_second,
            vram_used_mb: report.vram_used_mb,
        });
        self.run
            .obey(self.run.broker.progress(self.run.token, &progress));
    }

    fn checkpoint(&self, state: TrainState) {
        self.event(
            EventKind::Checkpoint,
            json!({ "superbatch": state.superbatch, "positions": state.positions }),
        );
    }
}

impl JobObserver<'_, '_> {
    fn event(&self, kind: EventKind, data: serde_json::Value) {
        let answer = self
            .run
            .broker
            .event(self.run.token, kind, data)
            .map(|()| Directive::Continue);
        self.run.obey(answer);
    }
}

#[cfg(test)]
mod tests {
    use chess_core::Position;
    use chess_jobs::{
        Device, Hyperparameters, JobKind, JobStatus, Lab, SelfplayRequest, TrainingRequest,
    };
    use chess_nnue::Network;
    use chess_nnue::spec::presets;

    use super::*;
    use crate::model::FloatWeights;
    use crate::test_dir;

    fn lab(name: &str) -> (Lab, PathBuf) {
        let dir = test_dir(name);
        let lab = Lab::open(&dir.join("lab.sqlite"), LabPaths::under(&dir)).unwrap();
        (lab, dir)
    }

    fn worker<'a>(lab: &'a Lab) -> Worker<'a> {
        Worker {
            broker: lab,
            paths: lab.paths().clone(),
            info: WorkerInfo {
                id: "test".into(),
                gpu: false,
                kinds: vec![JobKind::Training, JobKind::Selfplay, JobKind::Evaluation],
                trainers: vec!["burn".into()],
            },
            gpu: None,
            heartbeat: Duration::from_millis(100),
            selfplay: SelfplayConfig {
                hash_mb: 1,
                ..SelfplayConfig::default()
            },
        }
    }

    /// Positions whose score is pure material: something learnable.
    fn upload(lab: &Lab, dir: &std::path::Path) -> chess_jobs::Dataset {
        let path = crate::trainer::tests::write_dataset(dir);
        let file = std::fs::File::open(path).unwrap();
        lab.import_dataset("material", file, 1 << 20).unwrap()
    }

    /// A small architecture with every feature of the generic path: king
    /// buckets with mirroring, pairwise activation, a hidden layer, output
    /// buckets on every layer.
    fn architecture() -> serde_json::Value {
        let mut arch = presets::get("multilayer").unwrap().architecture.clone();
        arch.accumulator.size = 64;
        arch.hidden_layers[0].size = 8;
        arch.hidden_layers[1].size = 8;
        serde_json::to_value(arch).unwrap()
    }

    fn request(dataset: chess_jobs::DatasetId, superbatches: u64) -> TrainingRequest {
        TrainingRequest {
            name: Some("round trip".into()),
            architecture: Some(architecture()),
            architecture_id: None,
            datasets: vec![dataset],
            hyperparameters: Hyperparameters {
                superbatches,
                batch_size: 64,
                batches_per_superbatch: 8,
                learning_rate: 2e-2,
                lr_drop_every: 100,
                shuffle_records: 256,
                ..Hyperparameters::default()
            },
            seed: Some(42),
            device: Device::Auto,
            trainer: "burn".into(),
        }
    }

    /// The whole chain: a job trains a network, the model's file loads in
    /// the engine with the job's architecture, and the engine evaluates
    /// like the trainer's float network (up to quantization).
    #[test]
    fn trained_models_load_in_the_engine_and_agree_with_the_trainer() {
        let (lab, dir) = lab("worker-round-trip");
        let dataset = upload(&lab, &dir);
        let job = lab.create_training(&request(dataset.id, 6)).unwrap();
        let worker = worker(&lab);
        assert_eq!(worker.run_one(&StopFlag::new()).unwrap(), Some(job.id));

        let job = lab.job(job.id).unwrap();
        assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
        let model = lab.model(job.model.unwrap()).unwrap();
        assert_eq!(model.provenance.seed, 42);
        assert_eq!(model.provenance.superbatches, 6);
        assert_eq!(model.provenance.datasets[0].sha256, dataset.sha256.unwrap());

        let bytes = std::fs::read(lab.paths().model_file(model.id)).unwrap();
        let network = Network::from_bytes(&bytes).unwrap();
        let JobSpec::Training(spec) = &job.spec else {
            panic!("not a training job");
        };
        assert_eq!(network.hash().to_string(), spec.architecture_hash);
        assert_eq!(network.hash().to_string(), model.architecture_hash);

        // The engine and the trainer's float network agree.
        let checkpoint = lab.paths().run_dir(job.id).join("checkpoint/weights.cbfw");
        let float = FloatWeights::from_bytes(&std::fs::read(checkpoint).unwrap()).unwrap();
        let scale = spec.architecture.quantization.eval_scale as f32;
        for fen in [
            "4k3/8/8/8/8/8/8/3QK3 w - - 0 1",
            "3qk3/8/8/8/8/8/8/4K3 w - - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        ] {
            let pos = Position::from_fen(fen).unwrap();
            let (engine, trainer) = (network.evaluate(&pos) as f32, float.evaluate(&pos));
            assert!(
                (engine - trainer).abs() <= 0.03 * scale + 2.0,
                "{fen}: engine {engine}, trainer {trainer}"
            );
        }
        // It learned: a queen up is good for the side to move.
        let queen_up = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        assert!(
            network.evaluate(&queen_up) > 50,
            "{} {:?}",
            network.evaluate(&queen_up),
            model.provenance.final_loss
        );

        // The model loads where the engine loads networks.
        assert!(chess_eval::EngineEval::from_nnue_bytes(&bytes).is_ok());
    }

    #[test]
    fn stopped_jobs_resume_from_their_checkpoint() {
        let (lab, dir) = lab("worker-resume");
        let dataset = upload(&lab, &dir);
        let job = lab.create_training(&request(dataset.id, 4)).unwrap();
        let worker = worker(&lab);

        // Stop as soon as the first superbatch is reported.
        let stop = StopFlag::new();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                while lab.job(job.id).unwrap().progress.is_none() {
                    std::thread::sleep(Duration::from_millis(10));
                }
                lab.stop(job.id).unwrap();
            });
            worker.run_one(&stop).unwrap();
        });
        let stopped = lab.job(job.id).unwrap();
        assert_eq!(stopped.status, JobStatus::Stopped, "{:?}", stopped.error);
        let Some(Progress::Training(progress)) = stopped.progress else {
            panic!("no progress");
        };
        assert!(progress.superbatch < 4, "{progress:?}");

        lab.resume(job.id).unwrap();
        worker.run_one(&stop).unwrap();
        let done = lab.job(job.id).unwrap();
        assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
        let model = lab.model(done.model.unwrap()).unwrap();
        assert_eq!(model.provenance.superbatches, 4);
        let events = lab.events(job.id, 0, 1000).unwrap();
        assert!(
            events.iter().any(|e| e.data["message"]
                .as_str()
                .is_some_and(|m| m.contains("resumed"))),
            "{events:?}"
        );
        // Resumed, not restarted: 4 superbatches of 8 steps in all.
        let Some(Progress::Training(progress)) = done.progress else {
            panic!("no progress");
        };
        assert_eq!(progress.step, 32);
    }

    /// Serves the internal API of `lab` (workers' token `token`) on a
    /// local port, for as long as the test runs.
    fn serve(lab: &std::sync::Arc<Lab>) -> std::net::SocketAddr {
        let api = chess_nnue_api::NnueApi::new(
            std::sync::Arc::clone(lab),
            chess_nnue_api::ApiConfig {
                worker_token: Some("token".into()),
                ..chess_nnue_api::ApiConfig::default()
            },
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            runtime.block_on(async move {
                let _ = axum::serve(listener, api.router()).await;
            });
        });
        address
    }

    /// The same chain through the server's internal API, as a worker in
    /// another container runs it.
    #[test]
    fn workers_train_over_http() {
        let (lab, dir) = lab("worker-http");
        let lab = std::sync::Arc::new(lab);
        let dataset = upload(&lab, &dir);
        let job = lab.create_training(&request(dataset.id, 2)).unwrap();

        let address = serve(&lab);
        let http = crate::remote::HttpBroker::new(&format!("http://{address}"), "token").unwrap();
        let worker = Worker {
            broker: &http,
            ..worker(&lab)
        };
        assert_eq!(worker.run_one(&StopFlag::new()).unwrap(), Some(job.id));
        let done = lab.job(job.id).unwrap();
        assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
        let model = lab.model(done.model.unwrap()).unwrap();
        Network::from_bytes(&std::fs::read(lab.paths().model_file(model.id)).unwrap()).unwrap();
        assert_eq!(
            lab.events(job.id, 0, 100).unwrap().len(),
            8,
            "{:?}",
            lab.events(job.id, 0, 100)
        );

        // A worker with a wrong token is turned away.
        let intruder =
            crate::remote::HttpBroker::new(&format!("http://{address}"), "nope").unwrap();
        assert!(matches!(
            intruder.claim(&worker.info),
            Err(LabError::Unavailable(message)) if message.starts_with("401")
        ));
    }

    /// An evaluation match between a trained model and the classical
    /// evaluation, played by a worker over HTTP: the result, the speeds
    /// and the SPRT status are reported, and the same seed replays the
    /// same games.
    #[test]
    fn evaluations_play_matches_over_http() {
        let (lab, dir) = lab("worker-evaluation");
        let lab = std::sync::Arc::new(lab);
        let dataset = upload(&lab, &dir);
        let job = lab.create_training(&request(dataset.id, 1)).unwrap();
        worker(&lab).run_one(&StopFlag::new()).unwrap();
        let model = lab.job(job.id).unwrap().model.unwrap();

        let address = serve(&lab);
        let http = crate::remote::HttpBroker::new(&format!("http://{address}"), "token").unwrap();
        let worker = Worker {
            broker: &http,
            ..worker(&lab)
        };
        let evaluation = chess_jobs::EvaluationRequest {
            name: None,
            candidate: model,
            baseline: None,
            games: 6,
            nodes: Some(300),
            move_time_ms: None,
            sprt: Some(chess_jobs::Sprt::default()),
            seed: Some(3),
            threads: 2,
        };
        let mut results = Vec::new();
        for _ in 0..2 {
            let job = lab.create_evaluation(&evaluation).unwrap();
            assert_eq!(worker.run_one(&StopFlag::new()).unwrap(), Some(job.id));
            let done = lab.job(job.id).unwrap();
            assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
            let Some(Progress::Evaluation(result)) = done.progress else {
                panic!("no result: {done:?}");
            };
            assert_eq!(result.games, 6);
            assert_eq!(result.wins + result.draws + result.losses, 6);
            assert!(result.candidate_nps.unwrap() > 0 && result.baseline_nps.unwrap() > 0);
            let sprt = result.sprt.unwrap();
            assert_eq!(sprt.verdict, chess_jobs::Verdict::Inconclusive);
            assert!(sprt.lower_bound < sprt.llr && sprt.llr < sprt.upper_bound);
            // One progress report before the games, one per batch of pairs.
            let reports = lab
                .events(job.id, 0, 100)
                .unwrap()
                .into_iter()
                .filter(|e| e.kind == EventKind::Progress)
                .count();
            assert!(reports >= 3, "{reports}");
            results.push(result.result());
        }
        assert_eq!(results[0], results[1]);
    }

    #[test]
    fn failures_are_reported() {
        let (lab, dir) = lab("worker-failure");
        let dataset = upload(&lab, &dir);
        let job = lab.create_training(&request(dataset.id, 1)).unwrap();
        // The dataset's files disappeared.
        std::fs::remove_dir_all(lab.paths().dataset_dir(dataset.id)).unwrap();
        worker(&lab).run_one(&StopFlag::new()).unwrap();
        let failed = lab.job(job.id).unwrap();
        assert_eq!(failed.status, JobStatus::Failed);
        assert!(failed.error.unwrap().contains("no file"));
    }

    #[test]
    fn a_worker_shutting_down_interrupts_its_job() {
        let (lab, _) = lab("worker-shutdown");
        let (job, dataset) = lab
            .create_selfplay(&SelfplayRequest {
                name: None,
                games: 1_000,
                nodes: 200,
                seed: Some(1),
                network: None,
                threads: 1,
            })
            .unwrap();
        let stop = StopFlag::new();
        stop.stop();
        worker(&lab).run_one(&stop).unwrap();
        assert_eq!(lab.job(job.id).unwrap().status, JobStatus::Interrupted);
        assert_eq!(
            lab.dataset(dataset.id).unwrap().status,
            chess_jobs::DatasetStatus::Failed
        );
    }

    #[test]
    fn selfplay_jobs_produce_datasets() {
        let (lab, _) = lab("worker-selfplay");
        let (job, dataset) = lab
            .create_selfplay(&SelfplayRequest {
                name: None,
                games: 2,
                nodes: 200,
                seed: Some(1),
                network: None,
                threads: 1,
            })
            .unwrap();
        worker(&lab).run_one(&StopFlag::new()).unwrap();
        let job = lab.job(job.id).unwrap();
        assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
        let dataset = lab.dataset(dataset.id).unwrap();
        assert_eq!(dataset.status, chess_jobs::DatasetStatus::Ready);
        assert!(dataset.records > 10, "{}", dataset.records);
        assert_eq!(
            job.progress,
            Some(Progress::Selfplay(SelfplayProgress {
                games: 2,
                games_total: 2,
                positions: dataset.records,
            }))
        );
        // Positions of a dataset are valid records of real games.
        let files = lab.paths().dataset_files(dataset.id);
        let mut reader = chess_dataset::DatasetReader::open(&files[0]).unwrap();
        for record in reader.read_chunk(1000).unwrap() {
            record.to_position().unwrap();
        }
    }
}
