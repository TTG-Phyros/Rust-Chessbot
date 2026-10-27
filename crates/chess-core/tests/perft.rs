//! Perft against reference node counts.
//!
//! Positions 1-6 come from the Chess Programming Wiki ("Perft Results"); the
//! others are classic edge cases (illegal en passant, castling through check,
//! under-promotion, stalemate traps).
//!
//! `cargo test` runs the fast depths; `just perft` (or
//! `cargo test --release -p chess-core --test perft -- --include-ignored`)
//! also runs the deep ones (about 790 million leaf nodes).

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use chess_core::{Position, perft};

struct Case {
    name: &'static str,
    fen: &'static str,
    /// `counts[d - 1]` is perft(d).
    counts: &'static [u64],
}

const CPW: &[Case] = &[
    Case {
        name: "1. start position",
        fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        counts: &[20, 400, 8_902, 197_281, 4_865_609, 119_060_324],
    },
    Case {
        name: "2. kiwipete",
        fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        counts: &[48, 2_039, 97_862, 4_085_603, 193_690_690],
    },
    Case {
        name: "3. rook endgame",
        fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        counts: &[14, 191, 2_812, 43_238, 674_624, 11_030_083, 178_633_661],
    },
    Case {
        name: "4. promotions and castling",
        fen: "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        counts: &[6, 264, 9_467, 422_333, 15_833_292],
    },
    Case {
        name: "4b. position 4 mirrored",
        fen: "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1",
        counts: &[6, 264, 9_467, 422_333, 15_833_292],
    },
    Case {
        name: "5. discovered checks",
        fen: "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        counts: &[44, 1_486, 62_379, 2_103_487, 89_941_194],
    },
    Case {
        name: "6. middlegame",
        fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        counts: &[46, 2_079, 89_890, 3_894_594, 164_075_551],
    },
];

/// Classic edge cases (from the "perftsuite" of Peter Ellis Jones / TalkChess),
/// each with its published reference depth.
const EDGE_CASES: &[(&str, &str, u32, u64)] = &[
    (
        "illegal en passant 1",
        "3k4/3p4/8/K1P4r/8/8/8/8 b - - 0 1",
        6,
        1_134_888,
    ),
    (
        "illegal en passant 2",
        "8/8/4k3/8/2p5/8/B2P2K1/8 w - - 0 1",
        6,
        1_015_133,
    ),
    (
        "en passant checks opponent",
        "8/8/1k6/2b5/2pP4/8/5K2/8 b - d3 0 1",
        6,
        1_440_467,
    ),
    (
        "short castling gives check",
        "5k2/8/8/8/8/8/8/4K2R w K - 0 1",
        6,
        661_072,
    ),
    (
        "long castling gives check",
        "3k4/8/8/8/8/8/8/R3K3 w Q - 0 1",
        6,
        803_711,
    ),
    (
        "castling rights",
        "r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1",
        4,
        1_274_206,
    ),
    (
        "castling prevented",
        "r3k2r/8/3Q4/8/8/5q2/8/R3K2R b KQkq - 0 1",
        4,
        1_720_476,
    ),
    (
        "promote out of check",
        "2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1",
        6,
        3_821_001,
    ),
    (
        "discovered check",
        "8/8/1P2K3/8/2n5/1q6/8/5k2 b - - 0 1",
        5,
        1_004_658,
    ),
    (
        "promote to give check",
        "4k3/1P6/8/8/8/8/K7/8 w - - 0 1",
        6,
        217_342,
    ),
    (
        "under-promote to give check",
        "8/P1k5/K7/8/8/8/8/8 w - - 0 1",
        6,
        92_683,
    ),
    ("self stalemate", "K1k5/8/P7/8/8/8/8/8 w - - 0 1", 6, 2_217),
    (
        "stalemate and checkmate 1",
        "8/k1P5/8/1K6/8/8/8/8 w - - 0 1",
        7,
        567_584,
    ),
    (
        "stalemate and checkmate 2",
        "8/8/2k5/5q2/5n2/8/5K2/8 b - - 0 1",
        4,
        23_527,
    ),
];

/// Checks every CPW depth whose node count lies in `min_exclusive..=max`.
fn check_cpw(min_exclusive: u64, max: u64) {
    for case in CPW {
        let mut pos = Position::from_fen(case.fen).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let original = pos.clone();
        for (depth, &expected) in (1..).zip(case.counts) {
            if expected > min_exclusive && expected <= max {
                assert_eq!(
                    perft(&mut pos, depth),
                    expected,
                    "{} at depth {depth}",
                    case.name
                );
            }
        }
        assert_eq!(
            pos, original,
            "{}: perft must leave the position unchanged",
            case.name
        );
    }
}

const FAST_LIMIT: u64 = 5_000_000;

#[test]
fn cpw_positions_fast() {
    check_cpw(0, FAST_LIMIT);
}

#[test]
fn edge_cases() {
    for &(name, fen, depth, expected) in EDGE_CASES {
        let mut pos = Position::from_fen(fen).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(perft(&mut pos, depth), expected, "{name} at depth {depth}");
    }
}

#[test]
#[ignore = "deep perft: run with `just perft` (release mode)"]
fn cpw_positions_deep() {
    check_cpw(FAST_LIMIT, u64::MAX);
}
