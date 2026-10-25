//! Game-tree search.
//!
//! [`Searcher`] is generic over [`chess_eval::Evaluator`] and never names a
//! concrete evaluation: the caller injects it. It implements negamax
//! alpha-beta with iterative deepening, aspiration windows, a transposition
//! table, quiescence search, null-move pruning, reverse futility pruning,
//! late move reductions, killer moves, history heuristic and MVV-LVA.
//!
//! The search runs on the caller's thread and can be cancelled from any
//! other thread through a [`StopFlag`].

mod bench;
mod limits;
mod ordering;
mod score;
mod search;
mod tt;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use bench::{BENCH_POSITIONS, BenchResult, bench};
pub use limits::SearchLimits;
pub use score::{MATE, MAX_PLY, Score};
pub use search::{SearchInfo, SearchResult, Searcher};
pub use tt::{DEFAULT_HASH_MB, MAX_HASH_MB};

/// Shared cancellation flag, set by UCI `stop` or by the UI.
#[derive(Debug, Clone, Default)]
pub struct StopFlag(Arc<AtomicBool>);

impl StopFlag {
    /// Creates a cleared flag.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks every search holding this flag to stop as soon as possible.
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Clears the flag before a new search.
    pub fn reset(&self) {
        self.0.store(false, Ordering::Relaxed);
    }

    /// Whether a stop was requested.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_flag_round_trip() {
        let flag = StopFlag::new();
        let clone = flag.clone();
        clone.stop();
        assert!(flag.is_stopped());
        flag.reset();
        assert!(!clone.is_stopped());
    }
}
