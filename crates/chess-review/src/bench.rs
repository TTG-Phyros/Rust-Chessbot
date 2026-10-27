//! Speed of an engine with given settings, to estimate how long a review
//! will take before any game was analysed with them.

use std::time::Instant;

use chess_core::Position;

use crate::engine::{AnalysisEngine, SearchSettings};

/// Consecutive middlegame positions, searched in a row like those of a
/// game (the hash table helps from one to the next): the plies of the
/// "Opera game" (Morphy, 1858) after moves 6 to 13.
const MOVES: [&str; 26] = [
    "e2e4", "e7e5", "g1f3", "d7d6", "d2d4", "c8g4", "d4e5", "g4f3", "d1f3", "d6e5", "f1c4", "g8f6",
    "f3b3", "d8e7", "b1c3", "c7c6", "c1g5", "b7b5", "c3b5", "c6b5", "c4b5", "b8d7", "e1c1", "a8d8",
    "d1d7", "d8d7",
];
/// Plies played before the first position measured.
const FIRST: usize = 10;
/// Positions measured.
pub const POSITIONS: usize = 8;

/// Average time, in seconds, `engine` takes per position with `settings`
/// on a few typical positions (it starts a new game first).
///
/// # Errors
///
/// When the engine fails.
pub fn seconds_per_position(
    engine: &mut dyn AnalysisEngine,
    settings: &SearchSettings,
) -> anyhow::Result<f64> {
    let start = Position::startpos();
    let mut position = start.clone();
    let mut moves = Vec::with_capacity(MOVES.len());
    for text in MOVES {
        let mv = position.parse_uci_move(text)?;
        position.make_move(mv);
        moves.push(mv);
    }
    engine.new_game()?;
    let clock = Instant::now();
    for ply in FIRST..FIRST + POSITIONS {
        engine.analyse(&start, &moves[..ply], settings)?;
    }
    #[allow(clippy::cast_precision_loss)] // A handful of positions.
    Ok(clock.elapsed().as_secs_f64() / POSITIONS as f64)
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;
    use crate::Budget;
    use crate::engine::NativeEngine;

    #[test]
    fn deeper_searches_cost_more() {
        let mut engine = NativeEngine::new(None, 4).unwrap();
        let shallow = SearchSettings {
            budget: Budget::Depth(2),
            multipv: 1,
        };
        let deep = SearchSettings {
            budget: Budget::Depth(7),
            multipv: 2,
        };
        let fast = seconds_per_position(&mut engine, &shallow).unwrap();
        let slow = seconds_per_position(&mut engine, &deep).unwrap();
        assert!(fast > 0.0);
        assert!(slow > fast, "{slow} <= {fast}");
    }
}
