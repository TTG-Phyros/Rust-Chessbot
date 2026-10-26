//! Producer/consumer pipeline: self-play keeps generating positions on the
//! CPU while the network trains (on the GPU when there is one) on every
//! dataset file published so far.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use chess_search::StopFlag;
use serde_json::json;

use crate::config::Config;
use crate::dataset::{dataset_files, record_count};
use crate::device::SelectedDevice;
use crate::evaluation;
use crate::metrics::Metrics;
use crate::selfplay::{self, SelfplayConfig, SelfplayStats};
use crate::trainer::{TrainState, Trainer};

/// Runs self-play and training together until `stop` is raised or the
/// configured number of superbatches is reached.
pub(crate) fn run(
    config: &Config,
    device: &SelectedDevice,
    metrics: &Metrics,
    stop: &StopFlag,
    resume: bool,
) -> Result<TrainState> {
    let stats = Arc::new(SelfplayStats::default());
    let selfplay_stop = StopFlag::new();
    // Leave one core to feed the trainer (batch building, GPU driver).
    let cores = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
    let selfplay_config = SelfplayConfig {
        threads: if config.selfplay.threads == 0 {
            (cores - 1).max(1)
        } else {
            config.selfplay.threads
        },
        games: 0,
        // The games improve with the network: play with the latest export.
        network: Some(config.weights_dir().join(format!("{}.nnue", config.run))),
        ..config.selfplay.clone()
    };
    let (exports_tx, exports_rx) = std::sync::mpsc::channel();

    std::thread::scope(|scope| {
        // Measure each export against the classical evaluation.
        let evaluator = {
            let stop = stop.clone();
            scope.spawn(move || evaluation::run(exports_rx, &config.evaluation, metrics, &stop))
        };

        let producer = {
            let stats = Arc::clone(&stats);
            let stop = selfplay_stop.clone();
            scope.spawn(move || {
                selfplay::run(
                    &selfplay_config,
                    &config.dataset_dir,
                    &config.run,
                    &stop,
                    &stats,
                    |path| {
                        metrics.event(
                            "dataset",
                            json!({ "file": path, "positions": record_count(path) }),
                        );
                    },
                )
            })
        };

        // Mirror the self-play counters into the status file.
        let reporter = {
            let stats = Arc::clone(&stats);
            let stop = selfplay_stop.clone();
            scope.spawn(move || {
                while !stop.is_stopped() {
                    metrics.update(|status| {
                        status.games = stats.games.load(Ordering::Relaxed);
                        status.positions = stats.positions.load(Ordering::Relaxed);
                    });
                    std::thread::sleep(Duration::from_secs(2));
                }
            })
        };

        // Train on this run's files once enough positions exist.
        let (dir, run, min_positions) = (
            config.dataset_dir.clone(),
            config.run.clone(),
            config.min_positions,
        );
        let mut last_report: Option<std::time::Instant> = None;
        let files = Box::new(move || {
            let files = dataset_files(&dir, Some(&run));
            let available: u64 = files.iter().map(|f| record_count(f)).sum();
            if available < min_positions {
                // Called every second while waiting: report every 30 s only.
                if last_report.is_none_or(|at| at.elapsed() >= Duration::from_secs(30)) {
                    tracing::info!(
                        available,
                        needed = min_positions,
                        "waiting for self-play data (positions count once a chunk file is complete)"
                    );
                    last_report = Some(std::time::Instant::now());
                }
                return Vec::new();
            }
            files
        });
        let trainer = Trainer {
            config: config.train.clone(),
            run: config.run.clone(),
            checkpoint_dir: config.checkpoint_dir(),
            weights_dir: config.weights_dir(),
            device,
            metrics,
            stop: stop.clone(),
            exports: Some(exports_tx),
        };
        let trained = trainer.train(files, resume);
        // Closes the export channel: the evaluator finishes its last match.
        drop(trainer);
        let _ = evaluator.join();

        // Training ended (limit reached or stop requested): stop producing.
        selfplay_stop.stop();
        let produced = producer
            .join()
            .map_err(|_| anyhow::anyhow!("self-play panicked"))?;
        let _ = reporter.join();
        metrics.update(|status| {
            status.games = stats.games.load(Ordering::Relaxed);
            status.positions = stats.positions.load(Ordering::Relaxed);
        });
        produced?;
        trained
    })
}
