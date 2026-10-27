//! Analysis engines behind one trait.

#[cfg(feature = "native")]
mod native;
#[cfg(feature = "stockfish")]
mod stockfish;

use std::fmt;
use std::path::PathBuf;

use chess_core::{Move, Position};
use serde::{Deserialize, Serialize};

#[cfg(feature = "native")]
pub use native::NativeEngine;
#[cfg(feature = "stockfish")]
pub use stockfish::StockfishEngine;

/// Score for the side to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Score {
    /// Advantage in centipawns.
    Cp(i32),
    /// Mate in this many moves: positive if the side to move mates,
    /// negative if it gets mated; `0` when it is checkmated already.
    Mate(i32),
}

impl Score {
    /// The same score seen by the other side.
    #[must_use]
    pub const fn negate(self) -> Self {
        match self {
            Self::Cp(cp) => Self::Cp(-cp),
            Self::Mate(moves) => Self::Mate(-moves),
        }
    }
}

/// One of the best lines of a position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// Score for the side to move.
    pub score: Score,
    /// Depth of the search that found it.
    pub depth: u32,
    /// The moves, in UCI notation, starting with the candidate move.
    pub pv: Vec<String>,
}

/// How long to search each position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Budget {
    /// A fixed depth, in plies: reproducible.
    Depth(u32),
    /// A fixed time, in milliseconds.
    MoveTimeMs(u64),
    /// A fixed number of nodes: reproducible, and independent of the machine.
    Nodes(u64),
}

impl fmt::Display for Budget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Depth(depth) => write!(f, "depth {depth}"),
            Self::MoveTimeMs(ms) => write!(f, "{ms} ms"),
            Self::Nodes(nodes) => write!(f, "{nodes} nodes"),
        }
    }
}

/// Settings of each search of a review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSettings {
    /// Depth, time or nodes per position.
    pub budget: Budget,
    /// Number of best lines (`MultiPV`); at least 2 to recognise the only
    /// good move of a position.
    pub multipv: usize,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            budget: Budget::Depth(16),
            multipv: 3,
        }
    }
}

/// An engine able to list the best lines of a position.
pub trait AnalysisEngine: Send {
    /// Name and version, part of the cache key of its analyses (e.g.
    /// `Stockfish 19`, or our engine with a hash of its network).
    fn id(&self) -> &str;

    /// Forgets what previous searches learned (hash table), so that the
    /// analysis of a game does not depend on the games before it.
    ///
    /// # Errors
    ///
    /// When the engine stopped working.
    fn new_game(&mut self) -> anyhow::Result<()>;

    /// The best lines (up to `settings.multipv`), best first, of the
    /// position reached by `moves` from `start`; empty when the side to
    /// move has no legal move.
    ///
    /// # Errors
    ///
    /// When the engine stopped working.
    fn analyse(
        &mut self,
        start: &Position,
        moves: &[Move],
        settings: &SearchSettings,
    ) -> anyhow::Result<Vec<Line>>;
}

/// Which engine to start, and how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "kind")]
pub enum EngineSpec {
    /// Stockfish over UCI.
    Stockfish {
        /// The binary (looked up on the `PATH` without a directory).
        binary: PathBuf,
        /// Search threads per engine (one engine per worker).
        threads: usize,
        /// Hash table, in MiB.
        hash_mb: usize,
    },
    /// Our engine: the network of `eval_file` if it exists, the classical
    /// evaluation otherwise.
    Native {
        /// Network file (`.nnue`).
        eval_file: Option<PathBuf>,
        /// Transposition table, in MiB.
        hash_mb: usize,
    },
}

impl EngineSpec {
    /// Starts the engine.
    ///
    /// # Errors
    ///
    /// When it cannot be started (missing binary, invalid network), or was
    /// left out of this build.
    pub fn spawn(&self) -> anyhow::Result<Box<dyn AnalysisEngine>> {
        match self {
            #[cfg(feature = "stockfish")]
            Self::Stockfish {
                binary,
                threads,
                hash_mb,
            } => Ok(Box::new(StockfishEngine::spawn(
                binary, *threads, *hash_mb,
            )?)),
            #[cfg(feature = "native")]
            Self::Native { eval_file, hash_mb } => {
                Ok(Box::new(NativeEngine::new(eval_file.as_deref(), *hash_mb)?))
            }
            #[allow(unreachable_patterns)] // Every variant is handled with all features on.
            _ => anyhow::bail!("this engine was not compiled in"),
        }
    }
}

/// UCI notation of `moves`.
fn uci(moves: &[Move]) -> Vec<String> {
    moves.iter().map(ToString::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_serialize_compactly() {
        assert_eq!(
            serde_json::to_string(&Score::Cp(-35)).unwrap(),
            r#"{"cp":-35}"#
        );
        assert_eq!(
            serde_json::to_string(&Score::Mate(2)).unwrap(),
            r#"{"mate":2}"#
        );
        assert_eq!(Score::Mate(3).negate(), Score::Mate(-3));
        let settings: SearchSettings =
            serde_json::from_str(r#"{"budget":{"moveTimeMs":200},"multipv":2}"#).unwrap();
        assert_eq!(settings.budget, Budget::MoveTimeMs(200));
        assert_eq!(settings.budget.to_string(), "200 ms");
    }
}
