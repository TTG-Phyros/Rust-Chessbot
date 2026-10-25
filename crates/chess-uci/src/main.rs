//! UCI engine binary.
//!
//! Speaks the Universal Chess Interface on stdin/stdout. Logs go to stderr so
//! they never corrupt the protocol stream; set `RUST_LOG=debug` to see them.
//!
//! Besides the standard commands, it understands two debugging extensions
//! found in most engines: `go perft <depth>` and `d` (display the board).

use std::io::{self, BufRead, Write};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use chess_core::{Position, perft_divide};
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
        let (command, args) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match command {
            "uci" => {
                writeln!(out, "id name {ENGINE_NAME}")?;
                writeln!(out, "id author chess-bot contributors")?;
                writeln!(out, "uciok")?;
            }
            "isready" => writeln!(out, "readyok")?,
            "ucinewgame" => self.position = Position::startpos(),
            "position" => match parse_position(args) {
                Ok(position) => self.position = position,
                // UCI has no error reply: report it as information and keep
                // the previous position.
                Err(err) => {
                    tracing::warn!(%err, "invalid position command");
                    writeln!(out, "info string error: {err:#}")?;
                }
            },
            "go" => self.go(args, out)?,
            "d" => writeln!(out, "{}", self.position)?,
            "stop" => {}
            "quit" => return Ok(false),
            "" => {}
            other => tracing::warn!(command = other, "unknown command"),
        }
        Ok(true)
    }

    fn go(&mut self, args: &str, out: &mut impl Write) -> Result<()> {
        let mut tokens = args.split_whitespace();
        if tokens.next() == Some("perft") {
            let depth = match tokens.next().map(str::parse::<u32>) {
                Some(Ok(depth)) if depth > 0 => depth,
                _ => {
                    writeln!(out, "info string error: usage is `go perft <depth>`")?;
                    return Ok(());
                }
            };
            return self.perft(depth, out);
        }

        let result = self
            .searcher
            .search(&self.position, &SearchLimits::depth(4), |info| {
                tracing::debug!(depth = info.depth, score = %info.score, "search info");
            });
        writeln!(
            out,
            "info depth {} score {} nodes {}",
            result.depth, result.score, result.nodes
        )?;
        writeln!(out, "bestmove {}", result.best_move.unwrap_or_default())?;
        Ok(())
    }

    /// Prints the node count of each root move, then the total (Stockfish format).
    fn perft(&mut self, depth: u32, out: &mut impl Write) -> Result<()> {
        let start = Instant::now();
        let divide = perft_divide(&mut self.position, depth);
        let total: u64 = divide.iter().map(|(_, nodes)| nodes).sum();
        for (mv, nodes) in &divide {
            writeln!(out, "{mv}: {nodes}")?;
        }
        let elapsed = start.elapsed();
        writeln!(out, "\nNodes searched: {total}")?;
        tracing::info!(
            depth,
            total,
            ms = elapsed.as_millis(),
            nps = (total as f64 / elapsed.as_secs_f64().max(1e-9)) as u64,
            "perft"
        );
        Ok(())
    }
}

/// Parses the arguments of `position`: `startpos | fen <FEN>` then optionally
/// `moves <uci>...`.
fn parse_position(args: &str) -> Result<Position> {
    let (setup, moves) = match args.split_once("moves") {
        Some((setup, moves)) => (setup.trim(), Some(moves)),
        None => (args.trim(), None),
    };
    let mut position = if setup == "startpos" {
        Position::startpos()
    } else if let Some(fen) = setup.strip_prefix("fen") {
        Position::from_fen(fen.trim()).context("invalid FEN")?
    } else {
        bail!("expected `startpos` or `fen <FEN>`, got {setup:?}");
    };
    for text in moves.unwrap_or_default().split_whitespace() {
        let mv = position
            .parse_uci_move(text)
            .with_context(|| format!("move {text}"))?;
        position.make_move(mv);
    }
    Ok(position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_startpos_with_moves() {
        let pos = parse_position("startpos moves e2e4 c7c5 g1f3").unwrap();
        assert_eq!(
            pos.to_fen(),
            "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2"
        );
        assert_eq!(pos.ply(), 3);
    }

    #[test]
    fn parses_fen_with_and_without_moves() {
        let fen = "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1";
        assert_eq!(parse_position(&format!("fen {fen}")).unwrap().to_fen(), fen);
        let pos = parse_position(&format!("fen {fen} moves e2e4")).unwrap();
        assert_eq!(pos.to_fen(), "4k3/8/8/8/4P3/8/8/4K3 b - - 0 1");
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_position("").is_err());
        assert!(parse_position("fen nonsense").is_err());
        assert!(parse_position("startpos moves e2e5").is_err());
    }
}
