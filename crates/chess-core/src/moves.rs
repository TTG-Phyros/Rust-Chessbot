use std::fmt;
use std::ops::Deref;

use crate::{PieceKind, Square};

/// What a move does besides moving a piece from one square to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveKind {
    /// Non-capturing move.
    Quiet,
    /// Pawn advancing two squares from its starting rank.
    DoublePawnPush,
    /// Castling on the king's side (`O-O`).
    KingCastle,
    /// Castling on the queen's side (`O-O-O`).
    QueenCastle,
    /// Capture of the piece standing on the destination square.
    Capture,
    /// En passant capture.
    EnPassant,
    /// Non-capturing promotion.
    Promotion(PieceKind),
    /// Capturing promotion.
    PromotionCapture(PieceKind),
}

/// A move packed in 16 bits: `from` (6), `to` (6), kind (4).
///
/// A `Move` only makes sense together with the position it was generated
/// for. Castling is encoded as the king's move (`e1g1`), like UCI does.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Move(u16);

const QUIET: u16 = 0;
const DOUBLE_PAWN_PUSH: u16 = 1;
const KING_CASTLE: u16 = 2;
const QUEEN_CASTLE: u16 = 3;
const CAPTURE: u16 = 4;
const EN_PASSANT: u16 = 5;
const PROMOTION: u16 = 8;
const PROMOTION_CAPTURE: u16 = 12;

const fn promotion_bits(kind: PieceKind) -> u16 {
    match kind {
        PieceKind::Knight => 0,
        PieceKind::Bishop => 1,
        PieceKind::Rook => 2,
        // Pawn and king promotions do not exist; map them to a queen.
        PieceKind::Queen | PieceKind::Pawn | PieceKind::King => 3,
    }
}

impl Move {
    /// The null move (`a1a1`), used as a placeholder and written `0000` in UCI.
    pub const NULL: Self = Self(0);

    /// Creates a move.
    #[must_use]
    pub const fn new(from: Square, to: Square, kind: MoveKind) -> Self {
        let flag = match kind {
            MoveKind::Quiet => QUIET,
            MoveKind::DoublePawnPush => DOUBLE_PAWN_PUSH,
            MoveKind::KingCastle => KING_CASTLE,
            MoveKind::QueenCastle => QUEEN_CASTLE,
            MoveKind::Capture => CAPTURE,
            MoveKind::EnPassant => EN_PASSANT,
            MoveKind::Promotion(piece) => PROMOTION | promotion_bits(piece),
            MoveKind::PromotionCapture(piece) => PROMOTION_CAPTURE | promotion_bits(piece),
        };
        Self(from.index() as u16 | (to.index() as u16) << 6 | flag << 12)
    }

    /// Origin square.
    #[must_use]
    pub const fn from(self) -> Square {
        Square::from_u6((self.0 & 63) as u8)
    }

    /// Destination square.
    #[must_use]
    pub const fn to(self) -> Square {
        Square::from_u6(((self.0 >> 6) & 63) as u8)
    }

    const fn flag(self) -> u16 {
        self.0 >> 12
    }

    /// What the move does.
    #[must_use]
    pub const fn kind(self) -> MoveKind {
        let promoted = match self.flag() & 3 {
            0 => PieceKind::Knight,
            1 => PieceKind::Bishop,
            2 => PieceKind::Rook,
            _ => PieceKind::Queen,
        };
        match self.flag() {
            QUIET => MoveKind::Quiet,
            DOUBLE_PAWN_PUSH => MoveKind::DoublePawnPush,
            KING_CASTLE => MoveKind::KingCastle,
            QUEEN_CASTLE => MoveKind::QueenCastle,
            CAPTURE => MoveKind::Capture,
            EN_PASSANT => MoveKind::EnPassant,
            8..=11 => MoveKind::Promotion(promoted),
            _ => MoveKind::PromotionCapture(promoted),
        }
    }

    /// Whether the move captures (including en passant and capturing promotions).
    #[must_use]
    pub const fn is_capture(self) -> bool {
        self.flag() & CAPTURE != 0
    }

    /// Piece the pawn promotes to, if this is a promotion.
    #[must_use]
    pub const fn promotion(self) -> Option<PieceKind> {
        match self.kind() {
            MoveKind::Promotion(kind) | MoveKind::PromotionCapture(kind) => Some(kind),
            _ => None,
        }
    }

    /// Whether this is a castling move.
    #[must_use]
    pub const fn is_castle(self) -> bool {
        matches!(self.flag(), KING_CASTLE | QUEEN_CASTLE)
    }

    /// Whether this is the null move.
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Raw 16-bit encoding, e.g. for compact storage.
    #[must_use]
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Rebuilds a move from [`Move::to_bits`].
    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
}

/// UCI long algebraic notation: `e2e4`, `e7e8q`, `0000` for the null move.
impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_null() {
            return f.write_str("0000");
        }
        write!(f, "{}{}", self.from(), self.to())?;
        if let Some(kind) = self.promotion() {
            write!(f, "{}", kind.to_char())?;
        }
        Ok(())
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Move({self}, {:?})", self.kind())
    }
}

/// Upper bound on the number of legal moves in any position (218 is the known maximum).
pub const MAX_MOVES: usize = 256;

/// A fixed-capacity, stack-allocated list of moves.
#[derive(Clone)]
pub struct MoveList {
    moves: [Move; MAX_MOVES],
    len: usize,
}

impl MoveList {
    /// An empty list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            moves: [Move::NULL; MAX_MOVES],
            len: 0,
        }
    }

    /// Appends a move. Moves beyond [`MAX_MOVES`] are dropped, which cannot
    /// happen for lists of legal moves.
    pub fn push(&mut self, mv: Move) {
        if let Some(slot) = self.moves.get_mut(self.len) {
            *slot = mv;
            self.len += 1;
        }
    }

    /// Moves as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    /// Moves as a mutable slice (e.g. to sort them).
    pub fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for MoveList {
    type Target = [Move];
    fn deref(&self) -> &[Move] {
        self.as_slice()
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = std::slice::Iter<'a, Move>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl fmt::Debug for MoveList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.as_slice().iter().map(ToString::to_string))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_round_trip() {
        let kinds = [
            MoveKind::Quiet,
            MoveKind::DoublePawnPush,
            MoveKind::KingCastle,
            MoveKind::QueenCastle,
            MoveKind::Capture,
            MoveKind::EnPassant,
            MoveKind::Promotion(PieceKind::Knight),
            MoveKind::Promotion(PieceKind::Queen),
            MoveKind::PromotionCapture(PieceKind::Bishop),
            MoveKind::PromotionCapture(PieceKind::Rook),
        ];
        for kind in kinds {
            let mv = Move::new(Square::G7, Square::H8, kind);
            assert_eq!(mv.from(), Square::G7);
            assert_eq!(mv.to(), Square::H8);
            assert_eq!(mv.kind(), kind);
            assert_eq!(Move::from_bits(mv.to_bits()), mv);
        }
    }

    #[test]
    fn flags() {
        let ep = Move::new(Square::E5, Square::D6, MoveKind::EnPassant);
        assert!(ep.is_capture());
        let promo = Move::new(
            Square::E7,
            Square::E8,
            MoveKind::Promotion(PieceKind::Queen),
        );
        assert!(!promo.is_capture());
        assert_eq!(promo.promotion(), Some(PieceKind::Queen));
        assert!(Move::new(Square::E1, Square::G1, MoveKind::KingCastle).is_castle());
        assert!(Move::NULL.is_null());
    }

    #[test]
    fn uci_display() {
        assert_eq!(
            Move::new(Square::E2, Square::E4, MoveKind::DoublePawnPush).to_string(),
            "e2e4"
        );
        let promo = Move::new(
            Square::B7,
            Square::A8,
            MoveKind::PromotionCapture(PieceKind::Knight),
        );
        assert_eq!(promo.to_string(), "b7a8n");
        assert_eq!(Move::NULL.to_string(), "0000");
    }

    #[test]
    fn move_list() {
        let mut list = MoveList::new();
        assert!(list.is_empty());
        list.push(Move::new(Square::E2, Square::E4, MoveKind::DoublePawnPush));
        assert_eq!(list.len(), 1);
        assert_eq!(list.iter().count(), 1);
        for _ in 0..MAX_MOVES + 10 {
            list.push(Move::NULL);
        }
        assert_eq!(list.len(), MAX_MOVES);
    }
}
