//! Measures every exported network against the classical evaluation, to
//! chart the Elo progression of a run.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use chess_eval::EngineEval;
use chess_search::StopFlag;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::arena::{self, ArenaConfig, Contestant};
use crate::metrics::{Metrics, now};
use crate::ranking::{self, Ranked};

/// Checkpoint matches (`[evaluation]` section of the configuration).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct EvaluationConfig {
    /// Measure each export against the classical evaluation.
    pub(crate) enabled: bool,
    /// Games per measurement.
    pub(crate) games: u32,
    /// Nodes per move.
    pub(crate) nodes: u64,
    /// Threads taken from self-play while measuring.
    pub(crate) threads: usize,
    /// Measured snapshots kept per run, the best first (the others are deleted).
    pub(crate) keep_best: usize,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            games: 64,
            nodes: 2_000,
            threads: 2,
            keep_best: 5,
        }
    }
}

/// Consumes `(network file, superbatch)` exports until the channel closes.
/// When exports arrive faster than matches finish, only the latest is measured.
pub(crate) fn run(
    exports: Receiver<(PathBuf, u64)>,
    config: &EvaluationConfig,
    run: &str,
    metrics: &Metrics,
    stop: &StopFlag,
) {
    let arena = ArenaConfig {
        games: config.games,
        nodes: config.nodes,
        threads: config.threads.max(1),
        ..ArenaConfig::default()
    };
    let baseline = Contestant::Engine(EngineEval::default());
    while let Ok(mut export) = exports.recv() {
        while let Ok(newer) = exports.try_recv() {
            export = newer;
        }
        if stop.is_stopped() || !config.enabled {
            continue;
        }
        let (path, superbatch) = export;
        let network = match arena::load_evaluator(&path.display().to_string()) {
            Ok(network) => Contestant::Engine(network),
            Err(err) => {
                tracing::warn!(error = %format!("{err:#}"), "cannot evaluate the export");
                continue;
            }
        };
        let result = match arena::play_match(&network, &baseline, &arena, stop, |_| {}) {
            Ok(result) => result,
            Err(err) => {
                tracing::warn!(error = %format!("{err:#}"), "evaluation match failed");
                continue;
            }
        };
        let (elo, error) = (result.elo(), result.elo_error());
        tracing::info!(
            superbatch,
            elo = format!("{elo:+.0}"),
            error = format!("±{error:.0}"),
            "network vs classical evaluation"
        );
        metrics.event(
            "match",
            json!({
                "run": run, "superbatch": superbatch, "opponent": "classic", "file": path,
                "games": result.games(), "wins": result.wins, "draws": result.draws,
                "losses": result.losses, "elo": elo, "elo_error": error,
            }),
        );
        metrics.update(|status| {
            status.elo = Some(elo);
            status.elo_error = Some(error);
            status.elo_superbatch = Some(superbatch);
        });
        let Some(dir) = path.parent() else { continue };
        let entry = Ranked {
            superbatch,
            elo,
            elo_error: error,
            games: result.games(),
            measured_at: now(),
        };
        match ranking::record(dir, entry, config.keep_best) {
            Ok(ranking) => {
                if let Some(rank) = ranking.iter().position(|r| r.superbatch == superbatch) {
                    tracing::info!(superbatch, rank = rank + 1, "snapshot among the best");
                }
            }
            Err(err) => tracing::warn!(error = %format!("{err:#}"), "cannot rank the snapshot"),
        }
    }
}

#[cfg(test)]
mod tests {
    use chess_eval::nnue::NnueWeights;

    use super::*;
    use crate::metrics::Status;
    use crate::test_dir;

    #[test]
    fn measures_the_latest_export() {
        let dir = test_dir("evaluation");
        let net = dir.join("net.nnue");
        std::fs::write(&net, NnueWeights::zeros().to_bytes()).unwrap();
        let metrics = Metrics::open(&dir, Status::default()).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send((dir.join("missing.nnue"), 1)).unwrap();
        tx.send((net.clone(), 2)).unwrap();
        drop(tx);
        let config = EvaluationConfig {
            games: 2,
            nodes: 100,
            threads: 1,
            enabled: true,
            keep_best: 5,
        };
        run(rx, &config, "test", &metrics, &StopFlag::new());

        // Only the latest export (superbatch 2) was measured.
        let status = metrics.status();
        assert_eq!(status.elo_superbatch, Some(2));
        assert!(status.elo.is_some());
        let events = std::fs::read_to_string(dir.join("metrics/events.jsonl")).unwrap();
        assert_eq!(
            events
                .lines()
                .filter(|l| l.contains("\"kind\":\"match\""))
                .count(),
            1
        );
        // ...and ranked.
        let ranking = ranking::load(&dir).unwrap();
        assert_eq!(ranking.len(), 1);
        assert_eq!((ranking[0].superbatch, ranking[0].games), (2, 2));
    }
}
