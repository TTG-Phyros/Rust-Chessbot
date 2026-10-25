use std::ops::Not;

/// Side to move or owner of a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    /// White, moves first.
    White,
    /// Black.
    Black,
}

impl Color {
    /// Both colors, white first.
    pub const ALL: [Self; 2] = [Self::White, Self::Black];

    /// Index usable for arrays (`White` = 0).
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl Not for Color {
    type Output = Self;
    fn not(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }
}

/// Kind of a piece, independent of its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PieceKind {
    /// Pawn.
    Pawn,
    /// Knight.
    Knight,
    /// Bishop.
    Bishop,
    /// Rook.
    Rook,
    /// Queen.
    Queen,
    /// King.
    King,
}

impl PieceKind {
    /// All kinds, from pawn to king.
    pub const ALL: [Self; 6] = [
        Self::Pawn,
        Self::Knight,
        Self::Bishop,
        Self::Rook,
        Self::Queen,
        Self::King,
    ];

    /// Index usable for arrays (`Pawn` = 0).
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A colored piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Piece {
    /// Owner of the piece.
    pub color: Color,
    /// Kind of the piece.
    pub kind: PieceKind,
}

impl Piece {
    /// FEN character of the piece (uppercase for white).
    #[must_use]
    pub const fn to_fen_char(self) -> char {
        let c = match self.kind {
            PieceKind::Pawn => 'p',
            PieceKind::Knight => 'n',
            PieceKind::Bishop => 'b',
            PieceKind::Rook => 'r',
            PieceKind::Queen => 'q',
            PieceKind::King => 'k',
        };
        match self.color {
            Color::White => c.to_ascii_uppercase(),
            Color::Black => c,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_negation() {
        assert_eq!(!Color::White, Color::Black);
        assert_eq!(!Color::Black, Color::White);
    }

    #[test]
    fn fen_chars() {
        let white_knight = Piece {
            color: Color::White,
            kind: PieceKind::Knight,
        };
        let black_queen = Piece {
            color: Color::Black,
            kind: PieceKind::Queen,
        };
        assert_eq!(white_knight.to_fen_char(), 'N');
        assert_eq!(black_queen.to_fen_char(), 'q');
    }
}
