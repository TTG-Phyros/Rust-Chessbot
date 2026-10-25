//! UCI engine binary.
//!
//! Speaks the Universal Chess Interface on stdin/stdout. Logs go to stderr so
//! they never corrupt the protocol stream; set `RUST_LOG=debug` to see them.

use std::io::{self, BufRead, Write};

use anyhow::Result;
use chess_core::Position;
use chess_eval::ClassicEval;
use chess_search::{SearchLimits, Searcher, StopFlag};
use tracing_subscriber::EnvFilter;

const ENGINE_NAME: &str = concat!("chess-bot ", env!("CARGO_PKG_VERSION"));

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .init();

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut engine = Engine::new();

    for line in stdin.lock().lines() {
        let line = line?;
        tracing::debug!(command = %line, "received");
        if !engine.handle(line.trim(), &mut stdout)? {
            break;
        }
        stdout.flush()?;
    }
    Ok(())
}

/// Protocol state shared between commands.
struct Engine {
    position: Position,
    searcher: Searcher<ClassicEval>,
}

impl Engine {
    fn new() -> Self {
        Self {
            position: Position::startpos(),
            searcher: Searcher::new(ClassicEval, StopFlag::new()),
        }
    }

    /// Handles one command line. Returns `false` when the engine must exit.
    fn handle(&mut self, line: &str, out: &mut impl Write) -> Result<bool> {
        let mut tokens = line.split_whitespace();
        match tokens.next() {
            Some("uci") => {
                writeln!(out, "id name {ENGINE_NAME}")?;
                writeln!(out, "id author chess-bot contributors")?;
                writeln!(out, "uciok")?;
            }
            Some("isready") => writeln!(out, "readyok")?,
            Some("ucinewgame") => self.position = Position::startpos(),
            // Full FEN/move parsing lands with move generation in phase 2.
            Some("position") => self.position = Position::startpos(),
            Some("go") => {
                let info = self
                    .searcher
                    .search(&self.position, &SearchLimits::default(), |info| {
                        tracing::debug!(depth = info.depth, score = info.score, "search info");
                    });
                writeln!(
                    out,
                    "info depth {} score cp {} nodes {}",
                    info.depth, info.score, info.nodes
                )?;
                // Null move until the search can produce real moves (phase 3).
                writeln!(out, "bestmove 0000")?;
            }
            Some("stop") => {}
            Some("quit") => return Ok(false),
            Some(other) => tracing::warn!(command = other, "unknown command"),
            None => {}
        }
        Ok(true)
    }
}
