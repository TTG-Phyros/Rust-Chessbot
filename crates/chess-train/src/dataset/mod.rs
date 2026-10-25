//! Training dataset: fixed-size binary records, streamed from disk.
//!
//! # File format (version 1, little-endian)
//!
//! A 16-byte header — magic `CBDS`, version (`u16` = 1), record size
//! (`u16` = 32), 8 reserved bytes — followed by 32-byte records:
//!
//! | Offset | Size | Content |
//! | --- | --- | --- |
//! | 0 | 8 | occupancy bitboard (`u64`, bit `i` = square `i`, `a1` = 0) |
//! | 8 | 16 | one nibble per occupied square, in bit order: `color << 3 \| kind` |
//! | 24 | 2 | search score, centipawns, White's point of view (`i16`) |
//! | 26 | 1 | game result: 0 = Black won, 1 = draw, 2 = White won |
//! | 27 | 1 | side to move: 0 = White, 1 = Black |
//! | 28 | 2 | ply of the position in its game (`u16`) |
//! | 30 | 2 | reserved (0) |
//!
//! Fixed-size records make shuffling and splitting trivial.

mod reader;
mod writer;

use anyhow::{Result, bail, ensure};
use chess_core::{Bitboard, Color, Piece, PieceKind, Position, Square};
use chess_eval::nnue::feature_index;

pub(crate) use reader::{DatasetReader, dataset_files, record_count};
pub(crate) use writer::DatasetWriter;

pub(crate) const MAGIC: &[u8; 4] = b"CBDS";
pub(crate) const VERSION: u16 = 1;
pub(crate) const HEADER_SIZE: usize = 16;
pub(crate) const RECORD_SIZE: usize = 32;
/// Extension of finished dataset files (files being written end in `.tmp`).
pub(crate) const EXTENSION: &str = "bin";

/// Game result from White's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    BlackWins = 0,
    Draw = 1,
    WhiteWins = 2,
}

impl Outcome {
    /// Expected score for the side `color`: 1, ½ or 0.
    pub(crate) fn score_for(self, color: Color) -> f32 {
        let white = match self {
            Self::WhiteWins => 1.0,
            Self::Draw => 0.5,
            Self::BlackWins => 0.0,
        };
        if color == Color::White {
            white
        } else {
            1.0 - white
        }
    }
}

/// One training position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Record {
    occupancy: u64,
    pieces: [u8; 16],
    /// Search score in centipawns, White's point of view.
    pub(crate) score: i16,
    pub(crate) outcome: Outcome,
    pub(crate) side_to_move: Color,
    pub(crate) ply: u16,
}

impl Record {
    /// Packs `pos` with its score (White's point of view) and the game outcome.
    pub(crate) fn new(pos: &Position, score: i16, outcome: Outcome) -> Self {
        let occupied = pos.occupied();
        let mut pieces = [0_u8; 16];
        for (i, square) in occupied.enumerate().take(32) {
            if let Some(piece) = pos.piece_at(square) {
                let nibble = ((piece.color.index() as u8) << 3) | piece.kind.index() as u8;
                pieces[i / 2] |= nibble << (4 * (i % 2));
            }
        }
        Self {
            occupancy: occupied.0,
            pieces,
            score,
            outcome,
            side_to_move: pos.side_to_move(),
            ply: pos.ply().min(usize::from(u16::MAX)) as u16,
        }
    }

    pub(crate) fn to_bytes(self) -> [u8; RECORD_SIZE] {
        let mut bytes = [0_u8; RECORD_SIZE];
        bytes[0..8].copy_from_slice(&self.occupancy.to_le_bytes());
        bytes[8..24].copy_from_slice(&self.pieces);
        bytes[24..26].copy_from_slice(&self.score.to_le_bytes());
        bytes[26] = self.outcome as u8;
        bytes[27] = self.side_to_move.index() as u8;
        bytes[28..30].copy_from_slice(&self.ply.to_le_bytes());
        bytes
    }

    pub(crate) fn from_bytes(bytes: &[u8; RECORD_SIZE]) -> Result<Self> {
        let mut occupancy = [0; 8];
        occupancy.copy_from_slice(&bytes[0..8]);
        let mut pieces = [0; 16];
        pieces.copy_from_slice(&bytes[8..24]);
        let outcome = match bytes[26] {
            0 => Outcome::BlackWins,
            1 => Outcome::Draw,
            2 => Outcome::WhiteWins,
            other => bail!("invalid outcome byte {other}"),
        };
        let side_to_move = match bytes[27] {
            0 => Color::White,
            1 => Color::Black,
            other => bail!("invalid side-to-move byte {other}"),
        };
        let record = Self {
            occupancy: u64::from_le_bytes(occupancy),
            pieces,
            score: i16::from_le_bytes([bytes[24], bytes[25]]),
            outcome,
            side_to_move,
            ply: u16::from_le_bytes([bytes[28], bytes[29]]),
        };
        ensure!(record.occupancy.count_ones() <= 32, "more than 32 pieces");
        ensure!(
            record.pieces().all(|(_, piece)| piece.is_some()),
            "invalid piece nibble"
        );
        Ok(record)
    }

    /// Pieces with their squares (`None` for an invalid nibble).
    fn pieces(&self) -> impl Iterator<Item = (Square, Option<Piece>)> + '_ {
        Bitboard(self.occupancy).enumerate().map(|(i, square)| {
            let nibble = (self.pieces[i / 2] >> (4 * (i % 2))) & 0xF;
            let color = if nibble & 8 == 0 {
                Color::White
            } else {
                Color::Black
            };
            let kind = PieceKind::ALL.get(usize::from(nibble & 7)).copied();
            (square, kind.map(|kind| Piece::new(color, kind)))
        })
    }

    /// Input features from `perspective` (see `chess_eval::nnue`).
    pub(crate) fn features(&self, perspective: Color) -> impl Iterator<Item = usize> + '_ {
        self.pieces()
            .filter_map(move |(square, piece)| piece.map(|p| feature_index(perspective, p, square)))
    }

    /// Score for the side to move.
    pub(crate) fn score_for_side_to_move(&self) -> f32 {
        f32::from(self.score) * self.side_to_move.sign() as f32
    }

    /// Rebuilds the position (castling and en passant rights are not stored).
    #[cfg(test)]
    pub(crate) fn to_position(self) -> Result<Position> {
        let mut board = [None; 64];
        for (square, piece) in self.pieces() {
            board[square.index()] = piece;
        }
        let mut fen = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                match board[rank * 8 + file] {
                    Some(piece) => {
                        if empty > 0 {
                            fen.push_str(&empty.to_string());
                            empty = 0;
                        }
                        fen.push(Piece::to_fen_char(piece));
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                fen.push_str(&empty.to_string());
            }
            if rank > 0 {
                fen.push('/');
            }
        }
        fen.push_str(if self.side_to_move == Color::White {
            " w - - 0 1"
        } else {
            " b - - 0 1"
        });
        Ok(Position::from_fen(&fen)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FENS: [&str; 4] = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b - - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "4k3/8/8/8/8/8/8/4K3 b - - 0 1",
    ];

    #[test]
    fn records_round_trip() {
        for fen in FENS {
            let pos = Position::from_fen(fen).unwrap();
            let record = Record::new(&pos, -123, Outcome::Draw);
            let decoded = Record::from_bytes(&record.to_bytes()).unwrap();
            assert_eq!(decoded, record);
            assert_eq!(decoded.to_position().unwrap(), pos, "{fen}");
            assert_eq!(decoded.score, -123);
        }
    }

    #[test]
    fn features_match_the_engine() {
        let pos = Position::from_fen(FENS[1]).unwrap();
        let record = Record::new(&pos, 0, Outcome::WhiteWins);
        for color in Color::ALL {
            let mut from_record: Vec<usize> = record.features(color).collect();
            let mut from_position: Vec<usize> =
                chess_eval::nnue::active_features(&pos, color).collect();
            from_record.sort_unstable();
            from_position.sort_unstable();
            assert_eq!(from_record, from_position);
        }
    }

    #[test]
    fn side_relative_values() {
        let black = Record::new(
            &Position::from_fen(FENS[3]).unwrap(),
            50,
            Outcome::WhiteWins,
        );
        assert!((black.score_for_side_to_move() + 50.0).abs() < f32::EPSILON);
        assert!((Outcome::WhiteWins.score_for(Color::Black)).abs() < f32::EPSILON);
        assert!((Outcome::Draw.score_for(Color::White) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn rejects_corrupt_records() {
        let mut bytes = Record::new(&Position::startpos(), 0, Outcome::Draw).to_bytes();
        bytes[26] = 7;
        assert!(Record::from_bytes(&bytes).is_err());
        let mut bytes = Record::new(&Position::startpos(), 0, Outcome::Draw).to_bytes();
        bytes[8] = 0x77; // kind 7 does not exist
        assert!(Record::from_bytes(&bytes).is_err());
    }
}
