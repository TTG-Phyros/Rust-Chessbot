//! `chess-train` binary: self-play data generation, NNUE training on GPU
//! (CPU fallback) and export of quantized weights for `chess-eval`.
//!
//! ```text
//! chess-train gpu-check            # which device, and is it working?
//! chess-train selfplay --games N   # generate a dataset
//! chess-train train                # train on the dataset directory
//! chess-train pipeline             # both at once, until stopped
//! chess-train export --out f.nnue  # quantize the latest checkpoint
//! chess-train worker               # run the NNUE lab's queued jobs
//! chess-train lab ...              # lab datasets, training jobs, models
//! ```
//!
//! Settings come from `chess-train.toml`, `CHESS_TRAIN_*` variables and flags.

mod arena;
mod batch;
mod bots;
mod config;
mod control;
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
mod timer;
mod tournament;
mod trainer;
mod worker;

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chess_jobs::{
    DatasetId, Hyperparameters, JobId, JobKind, Lab, LabPaths, ModelId, SelfplayRequest,
    TrainingRequest, WorkerInfo,
};
use chess_search::StopFlag;
use clap::{Parser, Subcommand};
use serde::Serialize;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::device::{DeviceInfo, DevicePreference};
use crate::metrics::{Metrics, Status, now};
use crate::trainer::{BurnTrainer, RunObserver, Trainer, TrainingRun};

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
        /// Stop cleanly after this many minutes.
        #[arg(long)]
        max_minutes: Option<u64>,
    },
    /// Self-play and training together, until Ctrl+C or the superbatch limit.
    Pipeline {
        /// Superbatches to train (0 = until Ctrl+C).
        #[arg(long)]
        superbatches: Option<u64>,
        /// Start from scratch instead of resuming the run's checkpoint.
        #[arg(long)]
        fresh: bool,
        /// Stop cleanly after this many minutes.
        #[arg(long)]
        max_minutes: Option<u64>,
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
        /// Nodes per move of our engine.
        #[arg(long, conflicts_with_all = ["depth", "movetime"])]
        nodes: Option<u64>,
        /// Depth per move of our engine, instead of nodes.
        #[arg(long, conflicts_with = "movetime")]
        depth: Option<u32>,
        /// Milliseconds per move of our engine, instead of nodes.
        #[arg(long)]
        movetime: Option<u64>,
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
    /// NNUE lab worker: runs the queued training and self-play jobs, one at a time.
    Worker {
        /// Lab directory: database, datasets, models, checkpoints.
        #[arg(long, env = "CHESS_LAB_DIR", default_value = "data/lab")]
        lab: PathBuf,
        /// Run one job (or none if the queue is empty), then exit.
        #[arg(long)]
        once: bool,
        /// Seconds between two looks at an empty queue.
        #[arg(long, default_value_t = 2)]
        poll_seconds: u64,
    },
    /// NNUE lab: datasets, training jobs and models (until the REST API
    /// exists). `--device` says where training jobs may run.
    Lab {
        /// Lab directory: database, datasets, models, checkpoints.
        #[arg(long, env = "CHESS_LAB_DIR", default_value = "data/lab")]
        lab: PathBuf,
        #[command(subcommand)]
        command: LabCommand,
    },
}

#[derive(Debug, Subcommand)]
enum LabCommand {
    /// Imports a dataset file (`CBDS`), checking every record.
    Import {
        file: PathBuf,
        /// Name (default: the file name).
        #[arg(long)]
        name: Option<String>,
    },
    /// Queues a self-play job that generates a dataset.
    Selfplay {
        /// Games to play.
        #[arg(long)]
        games: u64,
        /// Nodes searched per move.
        #[arg(long, default_value_t = 5_000)]
        nodes: u64,
        /// Model playing the games (default: the classical evaluation).
        #[arg(long)]
        network: Option<ModelId>,
        /// Worker threads (0 = all cores).
        #[arg(long, default_value_t = 0)]
        threads: usize,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Queues a training job.
    Train {
        /// Architecture: a preset id or a JSON/YAML file.
        #[arg(long)]
        arch: String,
        /// Dataset to train on (repeatable).
        #[arg(long = "dataset", required = true)]
        datasets: Vec<DatasetId>,
        #[arg(long)]
        superbatches: Option<u64>,
        #[arg(long)]
        batch_size: Option<usize>,
        #[arg(long)]
        batches_per_superbatch: Option<usize>,
        #[arg(long)]
        learning_rate: Option<f64>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Lists the datasets.
    Datasets,
    /// Lists the jobs.
    Jobs,
    /// Lists the models.
    Models,
    /// Shows a job and its events.
    Job { id: JobId },
    /// Stops a job (cancels a queued one).
    Stop { id: JobId },
    /// Queues a stopped, interrupted or failed training job again.
    Resume { id: JobId },
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
            max_minutes,
        } => {
            if let Some(superbatches) = superbatches {
                config.train.superbatches = superbatches;
            }
            if let Some(batch_size) = batch_size {
                config.train.batch_size = batch_size;
            }
            run_train(&config, cli.device, !fresh, max_minutes)
        }
        Command::Pipeline {
            superbatches,
            fresh,
            max_minutes,
        } => {
            if let Some(superbatches) = superbatches {
                config.train.superbatches = superbatches;
            }
            run_pipeline(&config, cli.device, !fresh, max_minutes)
        }
        Command::Serve { port, autostart } => control::serve(config, cli.device, port, autostart),
        Command::Worker {
            lab,
            once,
            poll_seconds,
        } => run_worker(&config, cli.device, &lab, once, poll_seconds),
        Command::Lab { lab, command } => run_lab(&open_lab(&lab)?, command, cli.device),
        Command::Export { out } => {
            let out =
                out.unwrap_or_else(|| config.weights_dir().join(format!("{}.nnue", config.run)));
            trainer::export_checkpoint(&config.checkpoint_dir().join(&config.run), &out)?;
            println!("exported {}", out.display());
            Ok(())
        }
        Command::Match {
            a,
            b,
            games,
            nodes,
            depth,
            movetime,
            threads,
            sprt,
            elo0,
            elo1,
        } => {
            let arena = &mut config.arena;
            arena.games = games.unwrap_or(arena.games);
            arena.nodes = nodes.unwrap_or(arena.nodes);
            if let Some(limit) = depth
                .map(arena::MoveLimit::Depth)
                .or(movetime.map(arena::MoveLimit::Time))
            {
                arena.limit = Some(limit);
            }
            arena.move_limit().validate().map_err(anyhow::Error::msg)?;
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
        "{a} ({}) vs {b} ({}), {}",
        engine_a.name(),
        engine_b.name(),
        config.move_limit()
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
    let done = StopFlag::new();
    std::thread::scope(|scope| {
        // The boards of the workers, for the UI, until self-play ends.
        scope.spawn(|| {
            while !done.is_stopped() {
                metrics.write_boards(&stats.boards());
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });
        let played = selfplay::run(
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
        );
        done.stop();
        played
    })?;
    metrics.update(|status| {
        status.phase = "finished".into();
        status.games = stats.games.load(Ordering::Relaxed);
        status.positions = stats.positions.load(Ordering::Relaxed);
    });
    Ok(())
}

/// Stops cleanly after `minutes`, if given (the watcher ends with the run).
fn time_limit(stop: &StopFlag, minutes: Option<u64>) -> Result<()> {
    if let Some(minutes) = minutes {
        timer::Timer::validate(minutes).map_err(anyhow::Error::msg)?;
        let timer = timer::Timer::default();
        timer.set(Some(minutes));
        drop(timer.watch(stop.clone()));
    }
    Ok(())
}

fn run_train(
    config: &Config,
    preference: DevicePreference,
    resume: bool,
    max_minutes: Option<u64>,
) -> Result<()> {
    let stop = stop_on_signal()?;
    time_limit(&stop, max_minutes)?;
    let selected = device::select(preference)?;
    println!("device: {}", selected.info);
    let metrics = open_metrics(config, "training", selected.info.to_string())?;
    let files = chess_dataset::dataset_files(&config.dataset_dir, None);
    if files.is_empty() {
        bail!(
            "no dataset in {} (run `chess-train selfplay` first)",
            config.dataset_dir.display()
        );
    }
    let positions: u64 = files.iter().map(|f| chess_dataset::record_count(f)).sum();
    tracing::info!(files = files.len(), positions, "dataset found");
    let (exports_tx, exports_rx) = std::sync::mpsc::channel();
    let observer = RunObserver {
        run: config.run.clone(),
        weights_dir: config.weights_dir(),
        metrics: &metrics,
        exports: Some(exports_tx),
    };
    let run = TrainingRun {
        architecture: config.train.architecture()?,
        config: config.train.clone(),
        files: Box::new(move || files.clone()),
        checkpoint: config.checkpoint_dir().join(&config.run),
        resume,
        stop: stop.clone(),
    };
    let state = std::thread::scope(|scope| {
        let (metrics, stop) = (&metrics, &stop);
        let evaluator = scope.spawn(move || {
            evaluation::run(exports_rx, &config.evaluation, &config.run, metrics, stop)
        });
        let state = BurnTrainer { device: &selected }
            .train(run, &observer)
            .map(|outcome| outcome.state);
        // Closes the export channel: the evaluator finishes its last match.
        drop(observer);
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

fn run_pipeline(
    config: &Config,
    preference: DevicePreference,
    resume: bool,
    max_minutes: Option<u64>,
) -> Result<()> {
    let stop = stop_on_signal()?;
    time_limit(&stop, max_minutes)?;
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

fn open_lab(dir: &Path) -> Result<Lab> {
    Lab::open(&dir.join("lab.sqlite"), LabPaths::under(dir))
        .with_context(|| format!("cannot open the lab in {}", dir.display()))
}

/// Jobs whose worker has been silent this long are marked interrupted.
const WORKER_SILENCE: Duration = Duration::from_secs(120);

fn run_worker(
    config: &Config,
    preference: DevicePreference,
    dir: &Path,
    once: bool,
    poll_seconds: u64,
) -> Result<()> {
    let stop = stop_on_signal()?;
    let lab = open_lab(dir)?;
    let selected = device::select(preference)?;
    let gpu = matches!(selected.info, DeviceInfo::Cuda(_)).then_some(&selected);
    println!("device: {}", selected.info);
    let host = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "worker".into());
    let worker = worker::Worker {
        broker: &lab,
        paths: lab.paths().clone(),
        info: WorkerInfo {
            id: format!("{host}-{}", std::process::id()),
            gpu: gpu.is_some(),
            kinds: vec![JobKind::Training, JobKind::Selfplay],
            trainers: vec![BurnTrainer { device: &selected }.id().into()],
        },
        gpu,
        heartbeat: Duration::from_secs(10),
        selfplay: config.selfplay.clone(),
    };
    worker.run(
        &stop,
        Duration::from_secs(poll_seconds.max(1)),
        once,
        || {
            // Until the server does it: jobs of crashed workers become resumable.
            if let Err(err) = lab.reap_silent(WORKER_SILENCE) {
                tracing::warn!(error = %err, "cannot check for silent workers");
            }
        },
    )
}

fn run_lab(lab: &Lab, command: LabCommand, device: DevicePreference) -> Result<()> {
    fn print(value: &impl Serialize) -> Result<()> {
        println!("{}", serde_json::to_string_pretty(value)?);
        Ok(())
    }
    lab.reap_silent(WORKER_SILENCE)?;
    match command {
        LabCommand::Import { file, name } => {
            let name = name.unwrap_or_else(|| {
                file.file_stem()
                    .map_or_else(|| "dataset".into(), |s| s.to_string_lossy().into_owned())
            });
            let source = std::fs::File::open(&file)
                .with_context(|| format!("cannot open {}", file.display()))?;
            print(&lab.import_dataset(&name, std::io::BufReader::new(source), u64::MAX)?)
        }
        LabCommand::Selfplay {
            games,
            nodes,
            network,
            threads,
            seed,
            name,
        } => {
            let (job, dataset) = lab.create_selfplay(&SelfplayRequest {
                name,
                games,
                nodes,
                seed,
                network,
                threads,
            })?;
            print(&serde_json::json!({ "job": job, "dataset": dataset }))
        }
        LabCommand::Train {
            arch,
            datasets,
            superbatches,
            batch_size,
            batches_per_superbatch,
            learning_rate,
            seed,
            name,
        } => {
            let architecture = trainer::TrainConfig {
                architecture: Some(arch),
                ..trainer::TrainConfig::default()
            }
            .architecture()?;
            let defaults = Hyperparameters::default();
            let hyperparameters = Hyperparameters {
                superbatches: superbatches.unwrap_or(defaults.superbatches),
                batch_size: batch_size.unwrap_or(defaults.batch_size),
                batches_per_superbatch: batches_per_superbatch
                    .unwrap_or(defaults.batches_per_superbatch),
                learning_rate: learning_rate.unwrap_or(defaults.learning_rate),
                ..defaults
            };
            print(&lab.create_training(&TrainingRequest {
                name,
                architecture: serde_json::to_value(architecture)?,
                datasets,
                hyperparameters,
                seed,
                device: match device {
                    DevicePreference::Auto => chess_jobs::Device::Auto,
                    DevicePreference::Cuda => chess_jobs::Device::Gpu,
                    DevicePreference::Cpu => chess_jobs::Device::Cpu,
                },
                trainer: chess_jobs::BURN.into(),
            })?)
        }
        LabCommand::Datasets => print(&lab.datasets(None, chess_jobs::MAX_PAGE)?),
        LabCommand::Jobs => print(&lab.jobs(None, chess_jobs::MAX_PAGE)?),
        LabCommand::Models => print(&lab.models(None, chess_jobs::MAX_PAGE)?),
        LabCommand::Job { id } => print(&serde_json::json!({
            "job": lab.job(id)?,
            "events": lab.events(id, 0, 10_000)?,
        })),
        LabCommand::Stop { id } => print(&lab.stop(id)?),
        LabCommand::Resume { id } => print(&lab.resume(id)?),
    }
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

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn command_line_is_consistent() {
        super::Cli::command().debug_assert();
    }
}
