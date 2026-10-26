//! `chess-train` binary: self-play data generation, NNUE training on GPU
//! (CPU fallback) and export of quantized weights for `chess-eval`.
//!
//! ```text
//! chess-train gpu-check            # which device, and is it working?
//! chess-train selfplay --games N   # generate a dataset
//! chess-train train                # train on the dataset directory
//! chess-train pipeline             # both at once, until stopped
//! chess-train export --out f.nnue  # quantize the latest checkpoint
//! ```
//!
//! Settings come from `chess-train.toml`, `CHESS_TRAIN_*` variables and flags.

mod arena;
mod batch;
mod bots;
mod config;
mod control;
mod dataset;
mod device;
mod evaluation;
mod gpu_check;
mod metrics;
mod model;
mod network;
mod pipeline;
mod ranking;
mod rng;
mod selfplay;
mod stockfish;
mod tournament;
mod trainer;

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result, bail};
use chess_search::StopFlag;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::device::DevicePreference;
use crate::metrics::{Metrics, Status, now};

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Compute device used for training: `auto` picks the GPU when one is detected.
    #[arg(
        long,
        value_enum,
        default_value_t,
        env = "CHESS_TRAIN_DEVICE",
        global = true
    )]
    device: DevicePreference,

    /// Configuration file (default: `chess-train.toml` if present).
    #[arg(long, env = config::CONFIG_PATH_ENV, global = true)]
    config: Option<String>,

    /// Name of the run (overrides the configuration).
    #[arg(long, global = true)]
    run: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Prints the selected device and validates it with a matmul and a backward pass.
    GpuCheck {
        /// Side of the square matrices multiplied during the check.
        #[arg(long, default_value_t = 2048)]
        size: usize,
    },
    /// Generates training positions by self-play.
    Selfplay {
        /// Games to play (0 = until Ctrl+C).
        #[arg(long)]
        games: Option<u64>,
        /// Nodes searched per move.
        #[arg(long)]
        nodes: Option<u64>,
        /// Worker threads (0 = all cores).
        #[arg(long)]
        threads: Option<usize>,
        /// Network playing the games (reloaded when the file changes); default: classical.
        #[arg(long)]
        network: Option<PathBuf>,
    },
    /// Trains the network on the dataset directory.
    Train {
        /// Superbatches to train (0 = until Ctrl+C).
        #[arg(long)]
        superbatches: Option<u64>,
        /// Positions per step.
        #[arg(long)]
        batch_size: Option<usize>,
        /// Start from scratch instead of resuming the run's checkpoint.
        #[arg(long)]
        fresh: bool,
    },
    /// Self-play and training together, until Ctrl+C or the superbatch limit.
    Pipeline {
        /// Superbatches to train (0 = until Ctrl+C).
        #[arg(long)]
        superbatches: Option<u64>,
        /// Start from scratch instead of resuming the run's checkpoint.
        #[arg(long)]
        fresh: bool,
    },
    /// Control daemon: HTTP API to start and stop the pipeline (used by chess-server).
    Serve {
        /// Port of the control API.
        #[arg(long, default_value_t = 4000, env = "CHESS_TRAIN_CONTROL_PORT")]
        port: u16,
        /// Start the pipeline immediately.
        #[arg(long)]
        autostart: bool,
    },
    /// Quantizes the run's latest checkpoint into a `.nnue` file.
    Export {
        /// Output file (default: `<data_dir>/weights/<run>.nnue`).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Plays engine A against engine B to measure their Elo difference.
    Match {
        /// Engine A: `classic`, `stockfish-1` to `stockfish-10`, or a `.nnue` file.
        a: String,
        /// Engine B: `classic`, `stockfish-1` to `stockfish-10`, or a `.nnue` file.
        b: String,
        /// Maximum number of games.
        #[arg(long)]
        games: Option<u32>,
        /// Nodes per move.
        #[arg(long)]
        nodes: Option<u64>,
        /// Worker threads (0 = all cores).
        #[arg(long)]
        threads: Option<usize>,
        /// Stop as soon as the SPRT (elo0 vs elo1) concludes.
        #[arg(long)]
        sprt: bool,
        /// SPRT null hypothesis: A is this many Elo stronger.
        #[arg(long, allow_negative_numbers = true)]
        elo0: Option<f64>,
        /// SPRT alternative hypothesis.
        #[arg(long, allow_negative_numbers = true)]
        elo1: Option<f64>,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        // Dependencies stay at `warn`: cubecl's compiler logs whole IR dumps at `info`.
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn,chess_train=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let mut config = Config::load(cli.config.as_deref()).context("invalid configuration")?;
    if let Some(run) = cli.run {
        config.run = run;
    }

    match cli.command {
        Command::GpuCheck { size } => {
            let selected = device::select(cli.device)?;
            gpu_check::run(&selected, size)
        }
        Command::Selfplay {
            games,
            nodes,
            threads,
            network,
        } => {
            if network.is_some() {
                config.selfplay.network = network;
            }
            if let Some(games) = games {
                config.selfplay.games = games;
            }
            if let Some(nodes) = nodes {
                config.selfplay.nodes = nodes;
            }
            if let Some(threads) = threads {
                config.selfplay.threads = threads;
            }
            run_selfplay(&config)
        }
        Command::Train {
            superbatches,
            batch_size,
            fresh,
        } => {
            if let Some(superbatches) = superbatches {
                config.train.superbatches = superbatches;
            }
            if let Some(batch_size) = batch_size {
                config.train.batch_size = batch_size;
            }
            run_train(&config, cli.device, !fresh)
        }
        Command::Pipeline {
            superbatches,
            fresh,
        } => {
            if let Some(superbatches) = superbatches {
                config.train.superbatches = superbatches;
            }
            run_pipeline(&config, cli.device, !fresh)
        }
        Command::Serve { port, autostart } => control::serve(config, cli.device, port, autostart),
        Command::Export { out } => {
            let out =
                out.unwrap_or_else(|| config.weights_dir().join(format!("{}.nnue", config.run)));
            trainer::export_checkpoint(&config.checkpoint_dir(), &config.run, &out)?;
            println!("exported {}", out.display());
            Ok(())
        }
        Command::Match {
            a,
            b,
            games,
            nodes,
            threads,
            sprt,
            elo0,
            elo1,
        } => {
            let arena = &mut config.arena;
            arena.games = games.unwrap_or(arena.games);
            arena.nodes = nodes.unwrap_or(arena.nodes);
            arena.threads = threads.unwrap_or(arena.threads);
            arena.sprt |= sprt;
            arena.elo0 = elo0.unwrap_or(arena.elo0);
            arena.elo1 = elo1.unwrap_or(arena.elo1);
            run_match(&a, &b, &config.arena)
        }
    }
}

fn run_match(a: &str, b: &str, config: &arena::ArenaConfig) -> Result<()> {
    let stop = stop_on_signal()?;
    let engine_a = arena::Contestant::load(a, config)?;
    let engine_b = arena::Contestant::load(b, config)?;
    println!(
        "{a} ({}) vs {b} ({}), {} nodes per move",
        engine_a.name(),
        engine_b.name(),
        config.nodes
    );
    let report = |result: arena::MatchResult| {
        let mut line = format!(
            "games {:>4}  +{} ={} -{}  score {:.1}%  elo {:+.1} ± {:.1}  LOS {:.1}%",
            result.games(),
            result.wins,
            result.draws,
            result.losses,
            100.0 * result.score(),
            result.elo(),
            result.elo_error(),
            100.0 * result.los(),
        );
        if config.sprt {
            line.push_str(&format!(
                "  LLR {:.2} [{:.2}, {:.2}]",
                result.llr(config.elo0, config.elo1),
                (config.beta / (1.0 - config.alpha)).ln(),
                ((1.0 - config.beta) / config.alpha).ln()
            ));
        }
        line
    };
    let result = arena::play_match(&engine_a, &engine_b, config, &stop, |r| {
        println!("{}", report(r))
    })?;
    println!("final: {}", report(result));
    if config.sprt {
        let verdict = match result.verdict(config) {
            arena::Verdict::Pass => "H1 accepted: A is stronger",
            arena::Verdict::Fail => "H0 accepted: A is not stronger",
            arena::Verdict::Inconclusive => "inconclusive (more games needed)",
        };
        println!("SPRT elo0={} elo1={}: {verdict}", config.elo0, config.elo1);
    }
    Ok(())
}

/// A flag raised by Ctrl+C or SIGTERM (`docker stop`), for a clean shutdown
/// that writes a final checkpoint.
fn stop_on_signal() -> Result<StopFlag> {
    let stop = StopFlag::new();
    let flag = stop.clone();
    ctrlc::set_handler(move || {
        tracing::warn!("stop requested: finishing the current step");
        flag.stop();
    })
    .context("cannot install the signal handler")?;
    Ok(stop)
}

fn open_metrics(config: &Config, phase: &str, device: String) -> Result<Metrics> {
    let status = Status {
        run: config.run.clone(),
        phase: phase.into(),
        device,
        started_at: now(),
        ..Status::default()
    };
    Metrics::open(&config.data_dir, status)
}

fn run_selfplay(config: &Config) -> Result<()> {
    let stop = stop_on_signal()?;
    let metrics = open_metrics(config, "selfplay", "CPU".into())?;
    let stats = selfplay::SelfplayStats::default();
    selfplay::run(
        &config.selfplay,
        &config.dataset_dir,
        &config.run,
        &stop,
        &stats,
        |path| {
            metrics.event("dataset", serde_json::json!({ "file": path }));
            metrics.update(|status| {
                status.games = stats.games.load(Ordering::Relaxed);
                status.positions = stats.positions.load(Ordering::Relaxed);
            });
        },
    )?;
    metrics.update(|status| {
        status.phase = "finished".into();
        status.games = stats.games.load(Ordering::Relaxed);
        status.positions = stats.positions.load(Ordering::Relaxed);
    });
    Ok(())
}

fn run_train(config: &Config, preference: DevicePreference, resume: bool) -> Result<()> {
    let stop = stop_on_signal()?;
    let selected = device::select(preference)?;
    println!("device: {}", selected.info);
    let metrics = open_metrics(config, "training", selected.info.to_string())?;
    let files = dataset::dataset_files(&config.dataset_dir, None);
    if files.is_empty() {
        bail!(
            "no dataset in {} (run `chess-train selfplay` first)",
            config.dataset_dir.display()
        );
    }
    let positions: u64 = files.iter().map(|f| dataset::record_count(f)).sum();
    tracing::info!(files = files.len(), positions, "dataset found");
    let (exports_tx, exports_rx) = std::sync::mpsc::channel();
    let trainer = trainer::Trainer {
        config: config.train.clone(),
        run: config.run.clone(),
        checkpoint_dir: config.checkpoint_dir(),
        weights_dir: config.weights_dir(),
        device: &selected,
        metrics: &metrics,
        stop: stop.clone(),
        exports: Some(exports_tx),
    };
    let state = std::thread::scope(|scope| {
        let (metrics, stop) = (&metrics, &stop);
        let evaluator = scope.spawn(move || {
            evaluation::run(exports_rx, &config.evaluation, &config.run, metrics, stop)
        });
        let state = trainer.train(Box::new(move || files.clone()), resume);
        drop(trainer);
        let _ = evaluator.join();
        state
    })?;
    metrics.update(|status| status.phase = "finished".into());
    println!(
        "trained {} superbatches ({} positions)",
        state.superbatch, state.positions
    );
    Ok(())
}

fn run_pipeline(config: &Config, preference: DevicePreference, resume: bool) -> Result<()> {
    let stop = stop_on_signal()?;
    run_pipeline_with(config, preference, resume, &stop)
}

/// The pipeline with an external stop flag (signal handler or control API).
pub(crate) fn run_pipeline_with(
    config: &Config,
    preference: DevicePreference,
    resume: bool,
    stop: &StopFlag,
) -> Result<()> {
    let selected = device::select(preference)?;
    println!("device: {}", selected.info);
    let metrics = open_metrics(config, "pipeline", selected.info.to_string())?;
    let state = pipeline::run(config, &selected, &metrics, stop, resume);
    metrics.update(|status| status.phase = "finished".into());
    let state = state?;
    println!(
        "trained {} superbatches ({} positions)",
        state.superbatch, state.positions
    );
    Ok(())
}

/// A fresh, empty directory for a test.
#[cfg(test)]
pub(crate) fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("chess-train-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_default();
    dir
}
