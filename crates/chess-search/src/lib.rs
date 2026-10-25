//! Game-tree search.
//!
//! The searcher is generic over [`Evaluator`] and never names a concrete
//! evaluation: the caller injects it. Negamax alpha-beta, iterative deepening,
//! transposition table and pruning arrive in phase 3, once move generation exists.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chess_core::Position;
use chess_eval::{Evaluator, Score};

/// Limits of a search, mirroring the UCI `go` parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchLimits {
    /// Maximum depth in plies.
    pub depth: Option<u32>,
    /// Maximum number of nodes.
    pub nodes: Option<u64>,
    /// Fixed time per move, in milliseconds.
    pub movetime_ms: Option<u64>,
}

/// Progress report emitted while searching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInfo {
    /// Completed depth.
    pub depth: u32,
    /// Score in centipawns from the side to move's point of view.
    pub score: Score,
    /// Nodes visited so far.
    pub nodes: u64,
}

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

/// A searcher bound to an injected evaluator.
#[derive(Debug)]
pub struct Searcher<E> {
    evaluator: E,
    stop: StopFlag,
}

impl<E: Evaluator> Searcher<E> {
    /// Creates a searcher using `evaluator`, cancellable through `stop`.
    pub const fn new(evaluator: E, stop: StopFlag) -> Self {
        Self { evaluator, stop }
    }

    /// Searches `pos` within `limits`, calling `on_info` after each iteration.
    ///
    /// Phase 1 placeholder: returns the static evaluation at depth 0.
    pub fn search(
        &mut self,
        pos: &Position,
        _limits: &SearchLimits,
        mut on_info: impl FnMut(&SearchInfo),
    ) -> SearchInfo {
        let info = SearchInfo {
            depth: 0,
            score: self.evaluator.evaluate(pos),
            nodes: 1,
        };
        if !self.stop.is_stopped() {
            on_info(&info);
        }
        info
    }
}

#[cfg(test)]
mod tests {
    use chess_eval::ClassicEval;

    use super::*;

    #[test]
    fn search_reports_static_eval() {
        let mut searcher = Searcher::new(ClassicEval, StopFlag::new());
        let mut reports = 0;
        let info = searcher.search(&Position::startpos(), &SearchLimits::default(), |_| {
            reports += 1;
        });
        assert_eq!(info.score, 0);
        assert_eq!(reports, 1);
    }

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
