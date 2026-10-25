//! `chess-train` binary: self-play, dataset generation, NNUE training and export.
//!
//! Phase 1 implements `gpu-check` only; the other subcommands are wired so the
//! CLI shape is stable, and are filled in during phases 5 and 6.

mod device;
mod gpu_check;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use crate::device::DevicePreference;

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
    /// Generates training positions by self-play (phase 5).
    Selfplay,
    /// Trains the NNUE on a dataset (phase 5).
    Train,
    /// Exports quantized weights for `chess-eval` (phase 5).
    Export,
    /// Plays a match between two networks to estimate the Elo gain (phase 6).
    Match,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        // Dependencies stay at `warn`: cubecl's compiler logs whole IR dumps at `info`.
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn,chess_train=info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::GpuCheck { size } => {
            let selected = device::select(cli.device)?;
            gpu_check::run(&selected, size)
        }
        Command::Selfplay | Command::Train | Command::Export | Command::Match => {
            bail!("`{:?}` is not implemented yet", cli.command)
        }
    }
}
