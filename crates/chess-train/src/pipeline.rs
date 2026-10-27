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
use crate::ranking;
use crate::selfplay::{self, SelfplayConfig, SelfplayStats};
use crate::trainer::{TrainState, Trainer};

/// Decides, at the start of each epoch, whether the trainer may go through
/// the dataset again.
struct DataGate {
    min_positions: u64,
    max_reuse: u64,
    /// Positions handed to the trainer so far (one full dataset per epoch).
    served: u64,
}

#[derive(Debug, PartialEq, Eq)]
enum Gate {
    Open,
    /// Fewer than `min_positions` positions so far.
    NotEnoughData,
    /// Another epoch would train on each position more than `max_reuse`
    /// times on average.
    TooMuchReuse,
}

impl DataGate {
    fn new(min_positions: u64, max_reuse: u64) -> Self {
        Self {
            min_positions,
            max_reuse,
            served: 0,
        }
    }

    fn admit(&mut self, available: u64) -> Gate {
        if available < self.min_positions.max(1) {
            return Gate::NotEnoughData;
        }
        if self.max_reuse > 0 && self.served + available > available.saturating_mul(self.max_reuse)
        {
            return Gate::TooMuchReuse;
        }
        self.served += available;
        Gate::Open
    }
}

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
    // A network trained before rankings existed keeps its best snapshots.
    if let Err(err) = ranking::seed_from_events(
        &config.weights_dir().join(&config.run),
        &config.data_dir.join("metrics").join("events.jsonl"),
        &config.run,
        config.evaluation.keep_best,
    ) {
        tracing::warn!(error = %format!("{err:#}"), "cannot rebuild the snapshot ranking");
    }

    std::thread::scope(|scope| {
        // Measure each export against the classical evaluation.
        let evaluator = {
            let stop = stop.clone();
            scope.spawn(move || {
                evaluation::run(exports_rx, &config.evaluation, &config.run, metrics, &stop)
            })
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

        // Mirror the self-play counters into the status file, and the
        // boards of the workers for the UI.
        let reporter = {
            let stats = Arc::clone(&stats);
            let stop = selfplay_stop.clone();
            scope.spawn(move || {
                let mut tick: u64 = 0;
                while !stop.is_stopped() {
                    metrics.write_boards(&stats.boards());
                    if tick.is_multiple_of(2) {
                        metrics.update(|status| {
                            status.games = stats.games.load(Ordering::Relaxed);
                            status.positions = stats.positions.load(Ordering::Relaxed);
                        });
                    }
                    tick += 1;
                    std::thread::sleep(Duration::from_secs(1));
                }
            })
        };

        // Train on this run's files (and those it borrows) once enough positions exist.
        let dir = config.dataset_dir.clone();
        let mut runs = vec![config.run.clone()];
        runs.extend(
            config
                .datasets_from
                .iter()
                .filter(|run| **run != config.run)
                .cloned(),
        );
        let mut gate = DataGate::new(config.min_positions, config.max_reuse);
        let mut last_report: Option<std::time::Instant> = None;
        let files = Box::new(move || {
            let files: Vec<_> = runs
                .iter()
                .flat_map(|run| dataset_files(&dir, Some(run)))
                .collect();
            let available: u64 = files.iter().map(|f| record_count(f)).sum();
            match gate.admit(available) {
                Gate::Open => files,
                wait => {
                    // Called every second while waiting: report every 30 s only.
                    if last_report.is_none_or(|at| at.elapsed() >= Duration::from_secs(30)) {
                        match wait {
                            Gate::NotEnoughData => tracing::info!(
                                available,
                                needed = gate.min_positions,
                                "waiting for self-play data (positions count once a chunk file is complete)"
                            ),
                            _ => tracing::info!(
                                available,
                                max_reuse = gate.max_reuse,
                                "training is ahead of self-play: waiting for new games"
                            ),
                        }
                        last_report = Some(std::time::Instant::now());
                    }
                    Vec::new()
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_waits_for_data_then_bounds_reuse() {
        let mut gate = DataGate::new(100, 3);
        assert_eq!(gate.admit(50), Gate::NotEnoughData);
        // 100 positions: three epochs, then each was seen three times.
        for _ in 0..3 {
            assert_eq!(gate.admit(100), Gate::Open);
        }
        assert_eq!(gate.admit(100), Gate::TooMuchReuse);
        // New games make room again: 500 served for 200 available (≤ 3 × 200).
        assert_eq!(gate.admit(200), Gate::Open);
        assert_eq!(gate.admit(200), Gate::TooMuchReuse);

        let mut unlimited = DataGate::new(1, 0);
        for _ in 0..1000 {
            assert_eq!(unlimited.admit(10), Gate::Open);
        }
    }
}
