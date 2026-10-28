//! `chess-nnue`: inspect and convert network files.
//!
//! - `chess-nnue info <file>`: format version, architecture, hash, size
//! - `chess-nnue convert <in> <out>`: rewrites any readable network (e.g. a
//!   legacy version 1 file) in the current format; evaluations are unchanged
//! - `chess-nnue random <architecture> <out>`: an untrained network of an
//!   architecture (preset id or JSON/YAML file), for tests and benchmarks

use std::path::PathBuf;

use anyhow::{Context, Result};
use chess_nnue::spec::{Format, parse, presets};
use chess_nnue::{Network, inspect};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "Inspect and convert chess-bot NNUE network files")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Prints the header of a network file and checks its weights.
    Info {
        /// Network file.
        file: PathBuf,
    },
    /// Rewrites a network in the current format.
    Convert {
        /// Network to read (any supported version).
        input: PathBuf,
        /// File to write.
        output: PathBuf,
    },
    /// Writes a network of pseudo-random weights (for tests and benchmarks).
    Random {
        /// Preset id, or architecture file (`.json`, `.yaml`).
        architecture: String,
        /// File to write.
        output: PathBuf,
        /// Seed of the weights.
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
}

fn read(path: &PathBuf) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Info { file } => {
            let bytes = read(&file)?;
            let info =
                inspect(&bytes).with_context(|| format!("{} is not a network", file.display()))?;
            let network = Network::from_bytes(&bytes)
                .with_context(|| format!("{} has invalid weights", file.display()))?;
            let estimate = info.architecture.estimate();
            println!("file          {}", file.display());
            println!("format        version {}", info.version);
            println!("name          {}", info.architecture.metadata.name);
            println!("hash          {}", info.hash);
            println!("size          {} bytes", info.bytes);
            println!("inputs        {}", estimate.input_features);
            println!("parameters    {}", estimate.parameters);
            println!("kernel        {}", network.kernel());
            println!(
                "architecture  {}",
                serde_json::to_string_pretty(&info.architecture)?
            );
        }
        Command::Convert { input, output } => {
            let network = Network::from_bytes(&read(&input)?)
                .with_context(|| format!("{} is not a valid network", input.display()))?;
            let bytes = network.to_bytes();
            // Never write a file that would not load back identically.
            let back = Network::from_bytes(&bytes).context("converted network does not load")?;
            anyhow::ensure!(back.to_bytes() == bytes, "conversion is not stable");
            std::fs::write(&output, &bytes)
                .with_context(|| format!("cannot write {}", output.display()))?;
            println!(
                "{} → {} (format version {}, hash {})",
                input.display(),
                output.display(),
                chess_nnue::FORMAT_VERSION,
                network.hash()
            );
        }
        Command::Random {
            architecture,
            output,
            seed,
        } => {
            let architecture = match presets::get(&architecture) {
                Some(preset) => preset.architecture,
                None => {
                    let path = PathBuf::from(&architecture);
                    let format = match path.extension().and_then(|e| e.to_str()) {
                        Some("yaml" | "yml") => Format::Yaml,
                        _ => Format::Json,
                    };
                    let text = String::from_utf8(read(&path)?)?;
                    parse(&text, format).map_err(|errors| {
                        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
                        anyhow::anyhow!(
                            "invalid architecture:
  {}",
                            lines.join(
                                "
  "
                            )
                        )
                    })?
                }
            };
            let network = Network::pseudo_random(&architecture, seed)?;
            std::fs::write(&output, network.to_bytes())
                .with_context(|| format!("cannot write {}", output.display()))?;
            println!(
                "{} ({}, hash {})",
                output.display(),
                architecture.metadata.name,
                network.hash()
            );
        }
    }
    Ok(())
}
