use crate::{Bitboard, Color, PieceKind};

/// A chess position.
///
/// Phase 1 only stores piece placement and the side to move; castling rights,
/// en passant, move counters, Zobrist hashing and move generation arrive in phase 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    by_color: [Bitboard; 2],
    by_kind: [Bitboard; 6],
    side_to_move: Color,
}

impl Position {
    /// The standard starting position.
    #[must_use]
    pub const fn startpos() -> Self {
        Self {
            by_color: [
                Bitboard(0x0000_0000_0000_FFFF),
                Bitboard(0xFFFF_0000_0000_0000),
            ],
            by_kind: [
                Bitboard(0x00FF_0000_0000_FF00), // pawns
                Bitboard(0x4200_0000_0000_0042), // knights
                Bitboard(0x2400_0000_0000_0024), // bishops
                Bitboard(0x8100_0000_0000_0081), // rooks
                Bitboard(0x0800_0000_0000_0008), // queens
                Bitboard(0x1000_0000_0000_0010), // kings
            ],
            side_to_move: Color::White,
        }
    }

    /// Squares occupied by pieces of `color` and `kind`.
    #[must_use]
    pub fn pieces(&self, color: Color, kind: PieceKind) -> Bitboard {
        self.by_color[color.index()] & self.by_kind[kind.index()]
    }

    /// Squares occupied by any piece of `color`.
    #[must_use]
    pub const fn occupied_by(&self, color: Color) -> Bitboard {
        self.by_color[color.index()]
    }

    /// Side whose turn it is.
    #[must_use]
    pub const fn side_to_move(&self) -> Color {
        self.side_to_move
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::startpos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_piece_counts() {
        let pos = Position::startpos();
        for color in Color::ALL {
            assert_eq!(pos.occupied_by(color).count(), 16);
            assert_eq!(pos.pieces(color, PieceKind::Pawn).count(), 8);
            assert_eq!(pos.pieces(color, PieceKind::King).count(), 1);
        }
        let e1 = "e1".parse().unwrap();
        assert!(pos.pieces(Color::White, PieceKind::King).contains(e1));
        assert_eq!(pos.side_to_move(), Color::White);
    }
}
