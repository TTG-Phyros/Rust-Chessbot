//! NNUE ("efficiently updatable neural network") evaluation.
//!
//! # Architecture
//!
//! `768 → 256 ×2 → 1`, with perspective:
//!
//! - **Inputs**: 768 binary features per perspective, one per (piece relative
//!   color, piece kind, square relative to the perspective); see [`feature_index`].
//! - **Feature transformer**: a shared `768 → 256` affine layer, applied once
//!   from the side to move's perspective and once from the opponent's, each
//!   followed by a clipped ReLU (`clamp(x, 0, 1)`).
//! - **Output**: `512 → 1` affine layer on the concatenation (side to move
//!   first). Its value, times [`SCALE`], is the evaluation in centipawns for
//!   the side to move.
//!
//! # Quantization
//!
//! Feature transformer weights and biases are stored as `round(x * QA)` in
//! `i16`, output weights as `round(x * QB)` in `i16` and the output bias as
//! `round(x * QA * QB)` in `i32`, so that inference is integer-only:
//!
//! ```text
//! acc[h]  = ft_bias[h] + Σ_active ft_weights[f][h]          (i16 domain, ×QA)
//! eval_cp = (out_bias + Σ clamp(acc, 0, QA) · out_weights) · SCALE / (QA · QB)
//! ```
//!
//! # File format (version 1, little-endian)
//!
//! | Offset | Size | Content |
//! | --- | --- | --- |
//! | 0 | 4 | magic `CBNN` |
//! | 4 | 4 | format version (`u32` = 1) |
//! | 8 | 4 | inputs (`u32` = 768) |
//! | 12 | 4 | hidden size (`u32` = 256) |
//! | 16 | 4 | `QA` (`u32` = 255) |
//! | 20 | 4 | `QB` (`u32` = 64) |
//! | 24 | 4 | `SCALE` (`u32` = 400) |
//! | 28 | 768 × 256 × 2 | feature transformer weights, `i16`, feature-major |
//! | … | 256 × 2 | feature transformer biases, `i16` |
//! | … | 512 × 2 | output weights, `i16` (side to move half first) |
//! | … | 4 | output bias, `i32` |

use chess_core::{Color, Piece, Position, Square};
use thiserror::Error;

use crate::{Evaluator, Score};

/// Number of input features per perspective.
pub const INPUTS: usize = 768;
/// Width of the feature transformer (per perspective).
pub const HIDDEN: usize = 256;
/// Quantization factor of the feature transformer.
pub const QA: i32 = 255;
/// Quantization factor of the output layer.
pub const QB: i32 = 64;
/// Network output to centipawns.
pub const SCALE: i32 = 400;
/// Current file format version.
pub const FORMAT_VERSION: u32 = 1;

const MAGIC: &[u8; 4] = b"CBNN";
const HEADER_LEN: usize = 28;

/// Input feature of `piece` on `square`, seen from `perspective`: own pieces
/// come first, and Black's perspective mirrors the board vertically, so both
/// sides are encoded the same way.
#[must_use]
pub fn feature_index(perspective: Color, piece: Piece, square: Square) -> usize {
    let relative_square = match perspective {
        Color::White => square,
        Color::Black => square.flip_rank(),
    };
    let side = if piece.color == perspective { 0 } else { 6 };
    (side + piece.kind.index()) * 64 + relative_square.index()
}

/// Active features of `pos` from `perspective` (at most 32).
pub fn active_features(pos: &Position, perspective: Color) -> impl Iterator<Item = usize> + '_ {
    pos.occupied().filter_map(move |square| {
        pos.piece_at(square)
            .map(|piece| feature_index(perspective, piece, square))
    })
}

/// Why a weights file was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NnueError {
    /// Not a `CBNN` file.
    #[error("not a chess-bot network file (bad magic)")]
    BadMagic,
    /// Written by an incompatible version.
    #[error("unsupported network format version {0} (expected {FORMAT_VERSION})")]
    UnsupportedVersion(u32),
    /// Architecture or quantization different from this build's.
    #[error("network architecture mismatch: {0}")]
    Architecture(String),
    /// Truncated or oversized file.
    #[error("network file has {actual} bytes, expected {expected}")]
    Length {
        /// Size of the file.
        actual: usize,
        /// Size implied by the header.
        expected: usize,
    },
}

/// Quantized network weights.
#[derive(Clone, PartialEq, Eq)]
pub struct NnueWeights {
    /// `INPUTS × HIDDEN`, feature-major.
    pub ft_weights: Vec<i16>,
    /// `HIDDEN`.
    pub ft_bias: Vec<i16>,
    /// `2 × HIDDEN`: side to move half, then opponent half.
    pub out_weights: Vec<i16>,
    /// Output bias, scaled by `QA × QB`.
    pub out_bias: i32,
}

impl std::fmt::Debug for NnueWeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NnueWeights({INPUTS}→{HIDDEN}x2→1)")
    }
}

impl NnueWeights {
    /// Size of a version 1 file.
    pub const FILE_LEN: usize = HEADER_LEN + (INPUTS * HIDDEN + HIDDEN + 2 * HIDDEN) * 2 + 4;

    /// All-zero weights (evaluates every position to 0).
    #[must_use]
    pub fn zeros() -> Self {
        Self {
            ft_weights: vec![0; INPUTS * HIDDEN],
            ft_bias: vec![0; HIDDEN],
            out_weights: vec![0; 2 * HIDDEN],
            out_bias: 0,
        }
    }

    /// Serializes in the version 1 format.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::FILE_LEN);
        bytes.extend_from_slice(MAGIC);
        for value in [
            FORMAT_VERSION,
            INPUTS as u32,
            HIDDEN as u32,
            QA as u32,
            QB as u32,
            SCALE as u32,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for &value in self
            .ft_weights
            .iter()
            .chain(&self.ft_bias)
            .chain(&self.out_weights)
        {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&self.out_bias.to_le_bytes());
        bytes
    }

    /// Parses a version 1 file.
    ///
    /// # Errors
    ///
    /// [`NnueError`] if the file is not a compatible network.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, NnueError> {
        if bytes.get(..4) != Some(MAGIC.as_slice()) {
            return Err(NnueError::BadMagic);
        }
        let header = |index: usize| -> u32 {
            let start = 4 + index * 4;
            bytes
                .get(start..start + 4)
                .and_then(|b| b.try_into().ok())
                .map_or(0, u32::from_le_bytes)
        };
        let version = header(0);
        if version != FORMAT_VERSION {
            return Err(NnueError::UnsupportedVersion(version));
        }
        let expected = [
            INPUTS as u32,
            HIDDEN as u32,
            QA as u32,
            QB as u32,
            SCALE as u32,
        ];
        let found = [header(1), header(2), header(3), header(4), header(5)];
        if found != expected {
            return Err(NnueError::Architecture(format!(
                "file has inputs/hidden/QA/QB/scale = {found:?}, this build expects {expected:?}"
            )));
        }
        if bytes.len() != Self::FILE_LEN {
            return Err(NnueError::Length {
                actual: bytes.len(),
                expected: Self::FILE_LEN,
            });
        }

        let mut values = bytes[HEADER_LEN..bytes.len() - 4]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| i16::from_le_bytes(*pair));
        let ft_weights: Vec<i16> = values.by_ref().take(INPUTS * HIDDEN).collect();
        let ft_bias: Vec<i16> = values.by_ref().take(HIDDEN).collect();
        let out_weights: Vec<i16> = values.take(2 * HIDDEN).collect();
        let tail = &bytes[bytes.len() - 4..];
        let out_bias = i32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]);
        Ok(Self {
            ft_weights,
            ft_bias,
            out_weights,
            out_bias,
        })
    }

    /// Feature transformer output for `perspective`, computed from scratch.
    fn accumulate(&self, pos: &Position, perspective: Color) -> [i32; HIDDEN] {
        let mut acc = [0_i32; HIDDEN];
        for (a, &b) in acc.iter_mut().zip(&self.ft_bias) {
            *a = i32::from(b);
        }
        for feature in active_features(pos, perspective) {
            let row = &self.ft_weights[feature * HIDDEN..(feature + 1) * HIDDEN];
            for (a, &w) in acc.iter_mut().zip(row) {
                *a += i32::from(w);
            }
        }
        acc
    }

    /// Evaluation of `pos` in centipawns for the side to move, recomputing
    /// everything (reference implementation; the search uses an incremental one).
    #[must_use]
    pub fn evaluate(&self, pos: &Position) -> Score {
        let us = pos.side_to_move();
        let halves = [self.accumulate(pos, us), self.accumulate(pos, !us)];
        let mut sum = i64::from(self.out_bias);
        for (half, weights) in halves.iter().zip(self.out_weights.as_chunks::<HIDDEN>().0) {
            for (&a, &w) in half.iter().zip(weights) {
                sum += i64::from(a.clamp(0, QA)) * i64::from(w);
            }
        }
        (sum * i64::from(SCALE) / i64::from(QA * QB)) as Score
    }
}

/// [`Evaluator`] backed by a quantized network.
#[derive(Debug, Clone)]
pub struct NnueEval {
    weights: std::sync::Arc<NnueWeights>,
}

impl NnueEval {
    /// Wraps loaded weights.
    #[must_use]
    pub fn new(weights: NnueWeights) -> Self {
        Self {
            weights: std::sync::Arc::new(weights),
        }
    }

    /// The network's weights.
    #[must_use]
    pub fn weights(&self) -> &NnueWeights {
        &self.weights
    }
}

impl Evaluator for NnueEval {
    fn evaluate(&self, pos: &Position) -> Score {
        self.weights.evaluate(pos)
    }
}

#[cfg(test)]
mod tests {
    use chess_core::PieceKind;

    use super::*;

    #[test]
    fn feature_indices_are_perspective_relative() {
        let white_pawn = Piece::new(Color::White, PieceKind::Pawn);
        let black_pawn = Piece::new(Color::Black, PieceKind::Pawn);
        // A white pawn on e2 for White is a black pawn on e7 for Black.
        assert_eq!(
            feature_index(Color::White, white_pawn, Square::E2),
            feature_index(Color::Black, black_pawn, Square::E7)
        );
        assert_eq!(feature_index(Color::White, white_pawn, Square::A1), 0);
        let black_king = Piece::new(Color::Black, PieceKind::King);
        assert_eq!(
            feature_index(Color::White, black_king, Square::H8),
            INPUTS - 1
        );
    }

    #[test]
    fn features_swap_under_color_flip() {
        let pos = Position::from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        )
        .unwrap();
        let flipped = pos.flipped();
        let mut a: Vec<usize> = active_features(&pos, Color::White).collect();
        let mut b: Vec<usize> = active_features(&flipped, Color::Black).collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    /// Small deterministic weights for tests.
    fn pseudo_random_weights() -> NnueWeights {
        let mut state = 0x1234_5678_9ABC_DEF0_u64;
        let mut next = move |range: i64| -> i64 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % (2 * range as u64 + 1)) as i64 - range
        };
        let mut weights = NnueWeights::zeros();
        weights
            .ft_weights
            .iter_mut()
            .for_each(|w| *w = next(40) as i16);
        weights
            .ft_bias
            .iter_mut()
            .for_each(|w| *w = next(60) as i16);
        weights
            .out_weights
            .iter_mut()
            .for_each(|w| *w = next(100) as i16);
        weights.out_bias = next(5000) as i32;
        weights
    }

    #[test]
    fn file_round_trip() {
        let weights = pseudo_random_weights();
        let bytes = weights.to_bytes();
        assert_eq!(bytes.len(), NnueWeights::FILE_LEN);
        assert_eq!(NnueWeights::from_bytes(&bytes).unwrap(), weights);
    }

    #[test]
    fn rejects_bad_files() {
        let bytes = NnueWeights::zeros().to_bytes();
        assert_eq!(
            NnueWeights::from_bytes(b"nope").unwrap_err(),
            NnueError::BadMagic
        );
        let mut wrong_version = bytes.clone();
        wrong_version[4] = 9;
        assert_eq!(
            NnueWeights::from_bytes(&wrong_version).unwrap_err(),
            NnueError::UnsupportedVersion(9)
        );
        let mut wrong_hidden = bytes.clone();
        wrong_hidden[12] = 128;
        assert!(matches!(
            NnueWeights::from_bytes(&wrong_hidden),
            Err(NnueError::Architecture(_))
        ));
        assert!(matches!(
            NnueWeights::from_bytes(&bytes[..bytes.len() - 1]),
            Err(NnueError::Length { .. })
        ));
    }

    #[test]
    fn evaluation_is_symmetric_and_side_relative() {
        let eval = NnueEval::new(pseudo_random_weights());
        for fen in [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 b - - 0 1",
        ] {
            let pos = Position::from_fen(fen).unwrap();
            assert_eq!(eval.evaluate(&pos), eval.evaluate(&pos.flipped()), "{fen}");
        }
        assert_eq!(
            NnueEval::new(NnueWeights::zeros()).evaluate(&Position::startpos()),
            0
        );
    }
}
