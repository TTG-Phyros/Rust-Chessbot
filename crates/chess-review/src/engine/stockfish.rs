//! Stockfish at full strength, the reference engine of the reviews.

use std::path::Path;
use std::time::Duration;

use chess_core::{Move, Position};
use chess_stockfish::{Stockfish, UciScore};

use super::{AnalysisEngine, Budget, Line, Score, SearchSettings, uci};

/// A Stockfish process.
pub struct StockfishEngine {
    engine: Stockfish,
}

impl StockfishEngine {
    /// Starts `binary` with `threads` threads and a `hash_mb` MiB hash.
    ///
    /// # Errors
    ///
    /// When the binary cannot be started or does not speak UCI.
    pub fn spawn(binary: &Path, threads: usize, hash_mb: usize) -> anyhow::Result<Self> {
        Ok(Self {
            engine: Stockfish::spawn_full(binary, threads, hash_mb)?,
        })
    }
}

impl AnalysisEngine for StockfishEngine {
    fn id(&self) -> &str {
        self.engine.name()
    }

    fn new_game(&mut self) -> anyhow::Result<()> {
        self.engine.new_game()
    }

    fn analyse(
        &mut self,
        start: &Position,
        moves: &[Move],
        settings: &SearchSettings,
    ) -> anyhow::Result<Vec<Line>> {
        let fen = (*start != Position::startpos()).then(|| start.to_fen());
        let budget = match settings.budget {
            Budget::Depth(depth) => chess_stockfish::Budget::Depth(depth),
            Budget::MoveTimeMs(ms) => chess_stockfish::Budget::MoveTime(Duration::from_millis(ms)),
            Budget::Nodes(nodes) => chess_stockfish::Budget::Nodes(nodes),
        };
        let lines =
            self.engine
                .analyse_lines(fen.as_deref(), &uci(moves), budget, settings.multipv)?;
        Ok(lines
            .into_iter()
            .map(|line| Line {
                score: match line.score {
                    UciScore::Centipawns(cp) => Score::Cp(cp),
                    UciScore::Mate(moves) => Score::Mate(moves),
                },
                depth: line.depth,
                pv: line.pv,
            })
            .collect())
    }
}
