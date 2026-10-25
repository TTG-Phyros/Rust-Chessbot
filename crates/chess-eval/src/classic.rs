use chess_core::{Color, PieceKind, Position};

use crate::{Evaluator, Score};

/// Material values in centipawns, indexed by [`PieceKind::index`].
const MATERIAL: [Score; 6] = [100, 320, 330, 500, 900, 0];

/// Hand-crafted evaluation.
///
/// Phase 1 counts material only; tapered piece-square tables come in phase 3.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClassicEval;

impl Evaluator for ClassicEval {
    fn evaluate(&self, pos: &Position) -> Score {
        let white_minus_black: Score = PieceKind::ALL
            .iter()
            .map(|&kind| {
                let white = pos.pieces(Color::White, kind).count() as Score;
                let black = pos.pieces(Color::Black, kind).count() as Score;
                MATERIAL[kind.index()] * (white - black)
            })
            .sum();
        match pos.side_to_move() {
            Color::White => white_minus_black,
            Color::Black => -white_minus_black,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_is_balanced() {
        assert_eq!(ClassicEval.evaluate(&Position::startpos()), 0);
    }
}
