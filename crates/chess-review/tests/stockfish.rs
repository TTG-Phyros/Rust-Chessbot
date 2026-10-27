//! Famous moves classified from real Stockfish evaluations. Needs a
//! Stockfish: `STOCKFISH` names the binary, `stockfish` on the `PATH`
//! otherwise; skipped without one.
#![cfg(feature = "stockfish")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use chess_core::{Move, Position};
use chess_review::{
    AnalysisEngine, Budget, EngineSpec, MoveClass, SearchSettings, Thresholds, analyse_game,
    review_game,
};

fn stockfish() -> Option<Box<dyn AnalysisEngine>> {
    let binary =
        std::env::var_os("STOCKFISH").map_or_else(|| PathBuf::from("stockfish"), PathBuf::from);
    chess_stockfish::probe(&binary)?;
    let spec = EngineSpec::Stockfish {
        binary,
        threads: 1,
        hash_mb: 64,
    };
    Some(spec.spawn().unwrap())
}

/// The position after `moves` (SAN) from the start.
fn after(moves: &str) -> Position {
    let mut position = Position::startpos();
    for san in moves.split_whitespace() {
        let mv = position.parse_san(san).unwrap();
        position.make_move(mv);
    }
    position.without_history()
}

/// Classes of `moves` (SAN) played from `start`, Stockfish at depth 18.
fn classes(engine: &mut dyn AnalysisEngine, start: &Position, moves: &str) -> Vec<MoveClass> {
    let mut position = start.clone();
    let moves: Vec<Move> = moves
        .split_whitespace()
        .map(|san| {
            let mv = position.parse_san(san).unwrap();
            position.make_move(mv);
            mv
        })
        .collect();
    let settings = SearchSettings {
        budget: Budget::Depth(18),
        multipv: 3,
    };
    let analysis = analyse_game(engine, start, &moves, &settings, |_, _| true)
        .unwrap()
        .unwrap();
    review_game(
        start,
        &moves,
        &analysis.positions,
        0,
        &Thresholds::default(),
    )
    .unwrap()
    .moves
    .iter()
    .map(|review| review.class)
    .collect()
}

#[test]
fn famous_moves() {
    let Some(mut engine) = stockfish() else {
        eprintln!("no Stockfish: skipped");
        return;
    };
    let engine = engine.as_mut();

    // Legal's mate: 5. Nxe5!! gives the queen to the pinning bishop.
    let legal = after("e4 e5 Nf3 d6 Bc4 Bg4 Nc3 g6");
    assert_eq!(classes(engine, &legal, "Nxe5"), [MoveClass::Brilliant]);
    // Taking the queen loses: 5... Bxd1?? 6. Bxf7+ Ke7 7. Nd5#.
    let taken = after("e4 e5 Nf3 d6 Bc4 Bg4 Nc3 g6 Nxe5");
    assert_eq!(classes(engine, &taken, "Bxd1")[0], MoveClass::Blunder);

    // The Saavedra position: only 1. c7! wins.
    let saavedra = Position::from_fen("8/8/1KP5/3r4/8/8/8/k7 w - - 0 1").unwrap();
    assert_eq!(classes(engine, &saavedra, "c7"), [MoveClass::Great]);

    // 3... Nf6?? allows the Scholar's mate.
    let scholar = after("e4 e5 Bc4 Nc6 Qh5");
    assert_eq!(classes(engine, &scholar, "Nf6"), [MoveClass::Blunder]);
    // 2... Qh4?? leaves the queen to the knight, and 3. d3? does not take it.
    let open = after("e4 e5 Nf3");
    assert_eq!(
        classes(engine, &open, "Qh4 d3"),
        [MoveClass::Blunder, MoveClass::Miss]
    );
}
