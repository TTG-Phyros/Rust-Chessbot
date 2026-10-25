//! Forsyth-Edwards Notation.

use std::fmt::Write as _;
use std::str::FromStr;

use thiserror::Error;

use super::{CastlingRights, Position};
use crate::zobrist::KEYS;
use crate::{Color, Piece, PieceKind, Square};

/// FEN of the standard starting position.
pub const STARTPOS_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Why a FEN string was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FenError {
    /// Not 4 to 6 space-separated fields.
    #[error("expected 4 to 6 fields, found {0}")]
    FieldCount(usize),
    /// Malformed piece placement field.
    #[error("invalid piece placement: {0}")]
    Placement(String),
    /// Side to move is neither `w` nor `b`.
    #[error("invalid side to move: {0:?}")]
    SideToMove(String),
    /// Malformed castling field.
    #[error("invalid castling rights: {0:?}")]
    Castling(String),
    /// Malformed en passant field.
    #[error("invalid en passant square: {0:?}")]
    EnPassant(String),
    /// Malformed half-move clock or full-move number.
    #[error("invalid move counter: {0:?}")]
    Counter(String),
    /// Well-formed, but not a legal chess position.
    #[error("illegal position: {0}")]
    Illegal(&'static str),
}

impl Position {
    /// Parses a FEN string. The two move counters may be omitted (EPD style).
    ///
    /// Castling rights whose king or rook is not on its home square are
    /// dropped, and so is an en passant square no pawn can capture on.
    ///
    /// # Errors
    ///
    /// Returns a [`FenError`] if the string is malformed or describes an
    /// illegal position (missing king, pawn on a back rank, side not to move
    /// in check, ...).
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        let [placement, side, castling, en_passant, rest @ ..] = fields.as_slice() else {
            return Err(FenError::FieldCount(fields.len()));
        };
        if rest.len() > 2 {
            return Err(FenError::FieldCount(fields.len()));
        }

        let mut pos = Self::empty();
        parse_placement(&mut pos, placement)?;

        pos.side_to_move = match *side {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(FenError::SideToMove(other.to_owned())),
        };
        if pos.side_to_move == Color::Black {
            pos.hash ^= KEYS.side;
        }

        validate(&pos)?;

        let rights = parse_castling(castling)?;
        pos.set_castling(consistent_castling(&pos, rights));

        if *en_passant != "-" {
            let square = parse_en_passant(&pos, en_passant)?;
            pos.set_en_passant(pos.en_passant_if_capturable(square));
        }

        let counter = |text: &str| {
            text.parse::<u16>()
                .map_err(|_| FenError::Counter(text.into()))
        };
        pos.halfmove_clock = rest.first().map_or(Ok(0), |text| counter(text))?;
        pos.fullmove_number = rest.get(1).map_or(Ok(1), |text| counter(text))?.max(1);

        pos.checkers = pos.compute_checkers();
        if pos.checkers.count() > 2 {
            return Err(FenError::Illegal("more than two pieces give check"));
        }
        Ok(pos)
    }

    /// Serializes the position as FEN.
    #[must_use]
    pub fn to_fen(&self) -> String {
        let mut fen = String::with_capacity(90);
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                match self.board[rank * 8 + file] {
                    Some(piece) => {
                        if empty > 0 {
                            let _ = write!(fen, "{empty}");
                            empty = 0;
                        }
                        fen.push(piece.to_fen_char());
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                let _ = write!(fen, "{empty}");
            }
            if rank > 0 {
                fen.push('/');
            }
        }

        fen.push_str(match self.side_to_move {
            Color::White => " w ",
            Color::Black => " b ",
        });

        let rights = [
            (CastlingRights::WHITE_KINGSIDE, 'K'),
            (CastlingRights::WHITE_QUEENSIDE, 'Q'),
            (CastlingRights::BLACK_KINGSIDE, 'k'),
            (CastlingRights::BLACK_QUEENSIDE, 'q'),
        ];
        let before = fen.len();
        for (right, c) in rights {
            if self.castling.contains(right) {
                fen.push(c);
            }
        }
        if fen.len() == before {
            fen.push('-');
        }

        match self.en_passant {
            Some(square) => {
                let _ = write!(fen, " {square}");
            }
            None => fen.push_str(" -"),
        }
        let _ = write!(fen, " {} {}", self.halfmove_clock, self.fullmove_number);
        fen
    }
}

impl FromStr for Position {
    type Err = FenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_fen(s)
    }
}

fn parse_placement(pos: &mut Position, placement: &str) -> Result<(), FenError> {
    let ranks: Vec<&str> = placement.split('/').collect();
    if ranks.len() != 8 {
        return Err(FenError::Placement(format!(
            "expected 8 ranks, found {}",
            ranks.len()
        )));
    }
    for (row, text) in ranks.iter().enumerate() {
        let rank = 7 - row as u8;
        let wrong_length =
            || FenError::Placement(format!("rank {} does not have 8 squares", rank + 1));
        let mut file = 0_u8;
        for c in text.chars() {
            if file >= 8 {
                return Err(wrong_length());
            }
            match c.to_digit(10) {
                Some(skip @ 1..=8) => file += skip as u8,
                _ => {
                    let piece = Piece::from_fen_char(c).ok_or_else(|| {
                        FenError::Placement(format!("unexpected character {c:?}"))
                    })?;
                    pos.put_piece(Square::from_u6(rank * 8 + file), piece);
                    file += 1;
                }
            }
        }
        if file != 8 {
            return Err(wrong_length());
        }
    }
    Ok(())
}

fn validate(pos: &Position) -> Result<(), FenError> {
    for color in Color::ALL {
        if pos.pieces(color, PieceKind::King).count() != 1 {
            return Err(FenError::Illegal("each side needs exactly one king"));
        }
    }
    let back_ranks = crate::Bitboard(0xFF00_0000_0000_00FF);
    if !(pos.pieces_of_kind(PieceKind::Pawn) & back_ranks).is_empty() {
        return Err(FenError::Illegal("pawn on the first or last rank"));
    }
    let them = !pos.side_to_move;
    if pos.is_attacked_by(pos.king_square(them), pos.side_to_move, pos.occupied()) {
        return Err(FenError::Illegal("the side not to move is in check"));
    }
    Ok(())
}

fn parse_castling(text: &str) -> Result<CastlingRights, FenError> {
    if text == "-" {
        return Ok(CastlingRights::NONE);
    }
    text.chars().try_fold(CastlingRights::NONE, |rights, c| {
        let right = match c {
            'K' => CastlingRights::WHITE_KINGSIDE,
            'Q' => CastlingRights::WHITE_QUEENSIDE,
            'k' => CastlingRights::BLACK_KINGSIDE,
            'q' => CastlingRights::BLACK_QUEENSIDE,
            _ => return Err(FenError::Castling(text.to_owned())),
        };
        Ok(rights.with(right))
    })
}

/// Drops rights whose king or rook has left its home square.
fn consistent_castling(pos: &Position, rights: CastlingRights) -> CastlingRights {
    let homes = [
        (
            CastlingRights::WHITE_KINGSIDE,
            Color::White,
            Square::E1,
            Square::H1,
        ),
        (
            CastlingRights::WHITE_QUEENSIDE,
            Color::White,
            Square::E1,
            Square::A1,
        ),
        (
            CastlingRights::BLACK_KINGSIDE,
            Color::Black,
            Square::E8,
            Square::H8,
        ),
        (
            CastlingRights::BLACK_QUEENSIDE,
            Color::Black,
            Square::E8,
            Square::A8,
        ),
    ];
    homes
        .iter()
        .fold(rights, |rights, &(right, color, king, rook)| {
            let in_place = pos.piece_at(king) == Some(Piece::new(color, PieceKind::King))
                && pos.piece_at(rook) == Some(Piece::new(color, PieceKind::Rook));
            if in_place {
                rights
            } else {
                rights.without(right)
            }
        })
}

/// Parses the en passant square and checks it matches a pawn that just
/// advanced two squares.
fn parse_en_passant(pos: &Position, text: &str) -> Result<Square, FenError> {
    let invalid = || FenError::EnPassant(text.to_owned());
    let square: Square = text.parse().map_err(|_| invalid())?;
    let us = pos.side_to_move;
    // The opponent's pawn went from `start` over `square` to `landing`.
    let (expected_rank, landing, start) = match us {
        Color::White => (5, square.shifted(-8), square.shifted(8)),
        Color::Black => (2, square.shifted(8), square.shifted(-8)),
    };
    if square.rank().index() != expected_rank
        || pos.piece_at(landing) != Some(Piece::new(!us, PieceKind::Pawn))
        || pos.piece_at(square).is_some()
        || pos.piece_at(start).is_some()
    {
        return Err(invalid());
    }
    Ok(square)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUND_TRIP: [&str; 6] = [
        STARTPOS_FEN,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
        "4k3/8/8/8/8/8/8/4K3 b - - 99 150",
    ];

    #[test]
    fn round_trip() {
        for fen in ROUND_TRIP {
            assert_eq!(Position::from_fen(fen).unwrap().to_fen(), fen);
        }
    }

    #[test]
    fn counters_are_optional() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - -").unwrap();
        assert_eq!(pos.to_fen(), "4k3/8/8/8/8/8/8/4K3 w - - 0 1");
    }

    #[test]
    fn useless_en_passant_and_castling_are_dropped() {
        let pos = Position::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")
            .unwrap();
        assert_eq!(pos.en_passant(), None);
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K2R w KQkq - 0 1").unwrap();
        assert_eq!(pos.castling_rights(), CastlingRights::WHITE_KINGSIDE);
    }

    #[test]
    fn rejects_malformed_fens() {
        let cases = [
            ("", FenError::FieldCount(0)),
            ("8/8/8/8/8/8/8/8 w - - 0 1 extra", FenError::FieldCount(7)),
            (
                "4k3/8/8/8/8/8/8 w - - 0 1",
                FenError::Placement("expected 8 ranks, found 7".into()),
            ),
            (
                "4k3/8/8/8/8/8/8/4K3 x - - 0 1",
                FenError::SideToMove("x".into()),
            ),
            (
                "4k3/8/8/8/8/8/8/4K3 w X - 0 1",
                FenError::Castling("X".into()),
            ),
            (
                "4k3/8/8/8/8/8/8/4K3 w - e9 0 1",
                FenError::EnPassant("e9".into()),
            ),
            (
                "4k3/8/8/8/8/8/8/4K3 w - e6 0 1",
                FenError::EnPassant("e6".into()),
            ),
            (
                "4k3/8/8/8/8/8/8/4K3 w - - x 1",
                FenError::Counter("x".into()),
            ),
            (
                "8/8/8/8/8/8/8/4K3 w - - 0 1",
                FenError::Illegal("each side needs exactly one king"),
            ),
            (
                "4k3/8/8/8/8/8/8/4KP2 w - - 0 1",
                FenError::Illegal("pawn on the first or last rank"),
            ),
            (
                "4k3/4R3/8/8/8/8/8/4K3 w - - 0 1",
                FenError::Illegal("the side not to move is in check"),
            ),
        ];
        for (fen, expected) in cases {
            assert_eq!(Position::from_fen(fen).unwrap_err(), expected, "{fen:?}");
        }
        assert!(matches!(
            Position::from_fen("4k3/9/8/8/8/8/8/4K3 w - - 0 1"),
            Err(FenError::Placement(_))
        ));
        assert!(matches!(
            Position::from_fen("4k3/ppppppppp/8/8/8/8/8/4K3 w - - 0 1"),
            Err(FenError::Placement(_))
        ));
    }

    #[test]
    fn hash_depends_on_side_castling_and_en_passant() {
        let base = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        let black = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1").unwrap();
        let no_castle = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w - - 0 1").unwrap();
        assert_ne!(base.hash(), black.hash());
        assert_ne!(base.hash(), no_castle.hash());
        let counters = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 12 40").unwrap();
        assert_eq!(base.hash(), counters.hash());
    }
}
