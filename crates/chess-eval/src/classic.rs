//! Hand-crafted evaluation: material and piece-square tables, tapered between
//! middlegame and endgame by the amount of material left.
//!
//! Values are those of Ronald Friederich's PeSTO (as published on the Chess
//! Programming Wiki), tuned by Texel's method; they make a solid baseline for
//! a material+PST evaluation.

use chess_core::{Color, PieceKind, Position, Square};

use crate::{Evaluator, Score};

/// Material + PeSTO piece-square tables, tapered by game phase.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClassicEval;

impl Evaluator for ClassicEval {
    fn evaluate(&self, pos: &Position) -> Score {
        let mut mg = [0; 2];
        let mut eg = [0; 2];
        let mut phase = 0;
        for color in Color::ALL {
            for kind in PieceKind::ALL {
                for square in pos.pieces(color, kind) {
                    let index = table_index(color, square);
                    mg[color.index()] += TABLES.mg[kind.index()][index];
                    eg[color.index()] += TABLES.eg[kind.index()][index];
                    phase += PHASE_WEIGHT[kind.index()];
                }
            }
        }
        let us = pos.side_to_move().index();
        let them = 1 - us;
        let phase = phase.min(MAX_PHASE);
        let mg_score = mg[us] - mg[them];
        let eg_score = eg[us] - eg[them];
        (mg_score * phase + eg_score * (MAX_PHASE - phase)) / MAX_PHASE
    }
}

/// Middlegame and endgame material values (pawn, knight, bishop, rook, queen, king).
pub(crate) const MG_VALUE: [Score; 6] = [82, 337, 365, 477, 1025, 0];
/// See [`MG_VALUE`].
pub(crate) const EG_VALUE: [Score; 6] = [94, 281, 297, 512, 936, 0];

/// Contribution of each piece kind to the game phase: 24 with all pieces on
/// the board (pure middlegame), 0 with only kings and pawns (pure endgame).
const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

/// Tables are written rank 8 first (as seen by white), so a white piece on
/// `square` reads entry `square ^ 56` and a black one reads `square`.
fn table_index(color: Color, square: Square) -> usize {
    match color {
        Color::White => square.flip_rank().index(),
        Color::Black => square.index(),
    }
}

struct Tables {
    mg: [[Score; 64]; 6],
    eg: [[Score; 64]; 6],
}

/// Piece-square tables with the material value folded in.
static TABLES: Tables = {
    let mut mg = [MG_PAWN, MG_KNIGHT, MG_BISHOP, MG_ROOK, MG_QUEEN, MG_KING];
    let mut eg = [EG_PAWN, EG_KNIGHT, EG_BISHOP, EG_ROOK, EG_QUEEN, EG_KING];
    let mut kind = 0;
    while kind < 6 {
        let mut square = 0;
        while square < 64 {
            mg[kind][square] += MG_VALUE[kind];
            eg[kind][square] += EG_VALUE[kind];
            square += 1;
        }
        kind += 1;
    }
    Tables { mg, eg }
};

#[rustfmt::skip]
const MG_PAWN: [Score; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
     98, 134,  61,  95,  68, 126,  34, -11,
     -6,   7,  26,  31,  65,  56,  25, -20,
    -14,  13,   6,  21,  23,  12,  17, -23,
    -27,  -2,  -5,  12,  17,   6,  10, -25,
    -26,  -4,  -4, -10,   3,   3,  33, -12,
    -35,  -1, -20, -23, -15,  24,  38, -22,
      0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const EG_PAWN: [Score; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
    178, 173, 158, 134, 147, 132, 165, 187,
     94, 100,  85,  67,  56,  53,  82,  84,
     32,  24,  13,   5,  -2,   4,  17,  17,
     13,   9,  -3,  -7,  -7,  -8,   3,  -1,
      4,   7,  -6,   1,   0,  -5,  -1,  -8,
     13,   8,   8,  10,  13,   0,   2,  -7,
      0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const MG_KNIGHT: [Score; 64] = [
    -167, -89, -34, -49,  61, -97, -15, -107,
     -73, -41,  72,  36,  23,  62,   7,  -17,
     -47,  60,  37,  65,  84, 129,  73,   44,
      -9,  17,  19,  53,  37,  69,  18,   22,
     -13,   4,  16,  13,  28,  19,  21,   -8,
     -23,  -9,  12,  10,  19,  17,  25,  -16,
     -29, -53, -12,  -3,  -1,  18, -14,  -19,
    -105, -21, -58, -33, -17, -28, -19,  -23,
];

#[rustfmt::skip]
const EG_KNIGHT: [Score; 64] = [
    -58, -38, -13, -28, -31, -27, -63, -99,
    -25,  -8, -25,  -2,  -9, -25, -24, -52,
    -24, -20,  10,   9,  -1,  -9, -19, -41,
    -17,   3,  22,  22,  22,  11,   8, -18,
    -18,  -6,  16,  25,  16,  17,   4, -18,
    -23,  -3,  -1,  15,  10,  -3, -20, -22,
    -42, -20, -10,  -5,  -2, -20, -23, -44,
    -29, -51, -23, -15, -22, -18, -50, -64,
];

#[rustfmt::skip]
const MG_BISHOP: [Score; 64] = [
    -29,   4, -82, -37, -25, -42,   7,  -8,
    -26,  16, -18, -13,  30,  59,  18, -47,
    -16,  37,  43,  40,  35,  50,  37,  -2,
     -4,   5,  19,  50,  37,  37,   7,  -2,
     -6,  13,  13,  26,  34,  12,  10,   4,
      0,  15,  15,  15,  14,  27,  18,  10,
      4,  15,  16,   0,   7,  21,  33,   1,
    -33,  -3, -14, -21, -13, -12, -39, -21,
];

#[rustfmt::skip]
const EG_BISHOP: [Score; 64] = [
    -14, -21, -11,  -8,  -7,  -9, -17, -24,
     -8,  -4,   7, -12,  -3, -13,  -4, -14,
      2,  -8,   0,  -1,  -2,   6,   0,   4,
     -3,   9,  12,   9,  14,  10,   3,   2,
     -6,   3,  13,  19,   7,  10,  -3,  -9,
    -12,  -3,   8,  10,  13,   3,  -7, -15,
    -14, -18,  -7,  -1,   4,  -9, -15, -27,
    -23,  -9, -23,  -5,  -9, -16,  -5, -17,
];

#[rustfmt::skip]
const MG_ROOK: [Score; 64] = [
     32,  42,  32,  51,  63,   9,  31,  43,
     27,  32,  58,  62,  80,  67,  26,  44,
     -5,  19,  26,  36,  17,  45,  61,  16,
    -24, -11,   7,  26,  24,  35,  -8, -20,
    -36, -26, -12,  -1,   9,  -7,   6, -23,
    -45, -25, -16, -17,   3,   0,  -5, -33,
    -44, -16, -20,  -9,  -1,  11,  -6, -71,
    -19, -13,   1,  17,  16,   7, -37, -26,
];

#[rustfmt::skip]
const EG_ROOK: [Score; 64] = [
     13,  10,  18,  15,  12,  12,   8,   5,
     11,  13,  13,  11,  -3,   3,   8,   3,
      7,   7,   7,   5,   4,  -3,  -5,  -3,
      4,   3,  13,   1,   2,   1,  -1,   2,
      3,   5,   8,   4,  -5,  -6,  -8, -11,
     -4,   0,  -5,  -1,  -7, -12,  -8, -16,
     -6,  -6,   0,   2,  -9,  -9, -11,  -3,
     -9,   2,   3,  -1,  -5, -13,   4, -20,
];

#[rustfmt::skip]
const MG_QUEEN: [Score; 64] = [
    -28,   0,  29,  12,  59,  44,  43,  45,
    -24, -39,  -5,   1, -16,  57,  28,  54,
    -13, -17,   7,   8,  29,  56,  47,  57,
    -27, -27, -16, -16,  -1,  17,  -2,   1,
     -9, -26,  -9, -10,  -2,  -4,   3,  -3,
    -14,   2, -11,  -2,  -5,   2,  14,   5,
    -35,  -8,  11,   2,   8,  15,  -3,   1,
     -1, -18,  -9,  10, -15, -25, -31, -50,
];

#[rustfmt::skip]
const EG_QUEEN: [Score; 64] = [
     -9,  22,  22,  27,  27,  19,  10,  20,
    -17,  20,  32,  41,  58,  25,  30,   0,
    -20,   6,   9,  49,  47,  35,  19,   9,
      3,  22,  24,  45,  57,  40,  57,  36,
    -18,  28,  19,  47,  31,  34,  39,  23,
    -16, -27,  15,   6,   9,  17,  10,   5,
    -22, -23, -30, -16, -16, -23, -36, -32,
    -33, -28, -22, -43,  -5, -32, -20, -41,
];

#[rustfmt::skip]
const MG_KING: [Score; 64] = [
    -65,  23,  16, -15, -56, -34,   2,  13,
     29,  -1, -20,  -7,  -8,  -4, -38, -29,
     -9,  24,   2, -16, -20,   6,  22, -22,
    -17, -20, -12, -27, -30, -25, -14, -36,
    -49,  -1, -27, -39, -46, -44, -33, -51,
    -14, -14, -22, -46, -44, -30, -15, -27,
      1,   7,  -8, -64, -43, -16,   9,   8,
    -15,  36,  12, -54,   8, -28,  24,  14,
];

#[rustfmt::skip]
const EG_KING: [Score; 64] = [
    -74, -35, -18, -18, -11,  15,   4, -17,
    -12,  17,  14,  17,  17,  38,  23,  11,
     10,  17,  23,  15,  20,  45,  44,  13,
     -8,  22,  24,  27,  26,  33,  26,   3,
    -18,  -4,  21,  24,  27,  23,   9, -11,
    -19,  -3,  11,  21,  23,  16,   7,  -9,
    -27, -11,   4,  13,  14,   4,  -5, -17,
    -53, -34, -21, -11, -28, -14, -24, -43,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(fen: &str) -> Score {
        ClassicEval.evaluate(&Position::from_fen(fen).unwrap())
    }

    #[test]
    fn startpos_is_balanced() {
        assert_eq!(ClassicEval.evaluate(&Position::startpos()), 0);
    }

    #[test]
    fn symmetric_under_color_flip() {
        let fens = [
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        ];
        for fen in fens {
            let pos = Position::from_fen(fen).unwrap();
            assert_eq!(
                ClassicEval.evaluate(&pos),
                ClassicEval.evaluate(&pos.flipped()),
                "{fen}"
            );
        }
    }

    #[test]
    fn side_to_move_perspective() {
        let white = eval("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        let black = eval("4k3/8/8/8/8/8/8/3QK3 b - - 0 1");
        assert!(white > 800, "{white}");
        assert_eq!(white, -black);
    }

    #[test]
    fn prefers_centralised_knights_and_advanced_pawns() {
        assert!(eval("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1") > eval("4k3/8/8/8/8/8/8/N3K3 w - - 0 1"));
        assert!(eval("4k3/4P3/8/8/8/8/8/4K3 w - - 0 1") > eval("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1"));
    }
}
