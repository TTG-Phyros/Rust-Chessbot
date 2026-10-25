//! Fixed-depth search over a set of positions: a quick speed measurement
//! whose node count also acts as a signature (any change to the search or
//! the evaluation changes it).

use std::time::{Duration, Instant};

use chess_core::Position;
use chess_eval::Evaluator;

use crate::{SearchLimits, Searcher};

/// Positions searched by [`bench`]: openings, middlegames, endgames.
pub const BENCH_POSITIONS: [&str; 12] = [
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
    "r2q1rk1/pp2bppp/2n1pn2/3p4/2PP4/2N1PN2/PP3PPP/R2QKB1R w KQ - 0 9",
    "2r3k1/pp3ppp/4p3/3pP3/3P1P2/P5P1/1P5P/2R3K1 w - - 0 25",
    "8/8/4k3/3p4/3P4/4K3/8/8 w - - 0 1",
    "6k1/5ppp/8/8/8/8/1q3PPP/3R2K1 w - - 0 1",
    "8/5pk1/6p1/8/3B4/6P1/5PK1/8 b - - 0 40",
];

/// Totals of a [`bench`] run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BenchResult {
    /// Nodes searched over all positions.
    pub nodes: u64,
    /// Wall-clock time.
    pub elapsed: Duration,
}

impl BenchResult {
    /// Nodes per second.
    #[must_use]
    pub fn nps(&self) -> u64 {
        (self.nodes as f64 / self.elapsed.as_secs_f64().max(1e-6)) as u64
    }
}

/// Searches every [`BENCH_POSITIONS`] entry to `depth` from a cleared state,
/// so the node count is deterministic.
pub fn bench<E: Evaluator>(searcher: &mut Searcher<E>, depth: u32) -> BenchResult {
    let start = Instant::now();
    let mut nodes = 0;
    for fen in BENCH_POSITIONS {
        let Ok(pos) = Position::from_fen(fen) else {
            continue;
        };
        searcher.clear();
        nodes += searcher
            .search(&pos, &SearchLimits::depth(depth), |_| {})
            .nodes;
    }
    BenchResult {
        nodes,
        elapsed: start.elapsed(),
    }
}
