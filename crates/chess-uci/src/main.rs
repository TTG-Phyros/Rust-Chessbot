//! UCI engine binary.
//!
//! Speaks the Universal Chess Interface on stdin/stdout. Logs go to stderr so
//! they never corrupt the protocol stream; set `RUST_LOG=debug` to see them.
//!
//! Besides the standard commands, it understands the usual debugging
//! extensions: `go perft <depth>`, `d` (display the board) and
//! `bench [depth]`. `chess-uci bench [depth]` runs the bench and exits.

mod command;
mod engine;

use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tracing_subscriber::EnvFilter;

use crate::engine::{DEFAULT_BENCH_DEPTH, Engine, Output};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .init();

    let out: Output = Arc::new(Mutex::new(io::stdout()));
    let mut engine = Engine::new(out);

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("bench") {
        let depth = args
            .get(1)
            .map_or(DEFAULT_BENCH_DEPTH.to_string(), Clone::clone);
        engine.handle(&format!("bench {depth}"));
        return Ok(());
    }

    for line in io::stdin().lock().lines() {
        let line = line?;
        tracing::debug!(command = %line, "received");
        if !engine.handle(&line) {
            return Ok(());
        }
    }
    engine.finish_input();
    Ok(())
}
