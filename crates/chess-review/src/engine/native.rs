//! Our own engine: `chess-search` with the NNUE (or classical) evaluation.

use std::hash::Hasher;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use chess_core::{Move, Position};
use chess_eval::EngineEval;
use chess_search::{SearchLimits, Searcher, StopFlag};

use super::{AnalysisEngine, Budget, Line, Score, SearchSettings, uci};

/// A searcher of our own.
pub struct NativeEngine {
    searcher: Searcher<EngineEval>,
    id: String,
}

impl NativeEngine {
    /// Evaluates with the network of `eval_file` if it exists, classically
    /// otherwise, with a `hash_mb` MiB transposition table.
    ///
    /// # Errors
    ///
    /// When the network file exists but cannot be read or is invalid.
    pub fn new(eval_file: Option<&Path>, hash_mb: usize) -> anyhow::Result<Self> {
        let (eval, id) = match eval_file.filter(|path| path.exists()) {
            Some(path) => {
                let bytes = std::fs::read(path)
                    .with_context(|| format!("cannot read {}", path.display()))?;
                let eval = EngineEval::from_nnue_bytes(&bytes)
                    .with_context(|| format!("invalid network {}", path.display()))?;
                // The network is part of the identity: a new export is a new engine.
                let mut hasher = std::hash::DefaultHasher::new();
                hasher.write(&bytes);
                (eval, format!("chess-bot nnue {:016x}", hasher.finish()))
            }
            None => (EngineEval::default(), String::from("chess-bot classic")),
        };
        let mut searcher = Searcher::new(eval, StopFlag::new());
        searcher.set_hash_size(hash_mb.max(1));
        Ok(Self { searcher, id })
    }
}

impl AnalysisEngine for NativeEngine {
    fn id(&self) -> &str {
        &self.id
    }

    fn new_game(&mut self) -> anyhow::Result<()> {
        self.searcher.clear();
        Ok(())
    }

    fn analyse(
        &mut self,
        start: &Position,
        moves: &[Move],
        settings: &SearchSettings,
    ) -> anyhow::Result<Vec<Line>> {
        // Played from the start so that repetitions are known to the search.
        let mut position = start.clone();
        for &mv in moves {
            position.play(mv)?;
        }
        let mut limits = match settings.budget {
            Budget::Depth(depth) => SearchLimits::depth(depth),
            Budget::MoveTimeMs(ms) => SearchLimits::movetime(Duration::from_millis(ms)),
            Budget::Nodes(nodes) => SearchLimits::nodes(nodes),
        };
        limits.multipv = settings.multipv.max(1);
        self.searcher.stop_flag().reset();
        let result = self.searcher.search(&position, &limits, |_| {});
        Ok(result
            .lines
            .into_iter()
            .map(|line| Line {
                score: match line.score {
                    chess_search::Score::Centipawns(cp) => Score::Cp(cp),
                    chess_search::Score::Mate(moves) => Score::Mate(moves),
                },
                depth: result.depth,
                pv: uci(&line.pv),
            })
            .collect())
    }
}
