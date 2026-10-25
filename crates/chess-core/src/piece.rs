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

    /// `1` for white, `-1` for black: turns a white-relative score into a
    /// score from this side's point of view.
    #[must_use]
    pub const fn sign(self) -> i32 {
        match self {
            Self::White => 1,
            Self::Black => -1,
        }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    /// Kinds a pawn can promote to, best first.
    pub const PROMOTIONS: [Self; 4] = [Self::Queen, Self::Rook, Self::Bishop, Self::Knight];

    /// Index usable for arrays (`Pawn` = 0).
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Lowercase letter (`p`, `n`, `b`, `r`, `q`, `k`), as in FEN and UCI.
    #[must_use]
    pub const fn to_char(self) -> char {
        match self {
            Self::Pawn => 'p',
            Self::Knight => 'n',
            Self::Bishop => 'b',
            Self::Rook => 'r',
            Self::Queen => 'q',
            Self::King => 'k',
        }
    }

    /// Parses a piece letter, case-insensitively.
    #[must_use]
    pub const fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            'p' => Some(Self::Pawn),
            'n' => Some(Self::Knight),
            'b' => Some(Self::Bishop),
            'r' => Some(Self::Rook),
            'q' => Some(Self::Queen),
            'k' => Some(Self::King),
            _ => None,
        }
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
    /// Creates a piece.
    #[must_use]
    pub const fn new(color: Color, kind: PieceKind) -> Self {
        Self { color, kind }
    }

    /// Index in `0..12` (white pieces first), usable for arrays.
    #[must_use]
    pub const fn index(self) -> usize {
        self.color.index() * 6 + self.kind.index()
    }

    /// FEN character of the piece (uppercase for white).
    #[must_use]
    pub const fn to_fen_char(self) -> char {
        let c = self.kind.to_char();
        match self.color {
            Color::White => c.to_ascii_uppercase(),
            Color::Black => c,
        }
    }

    /// Parses a FEN character (uppercase for white).
    #[must_use]
    pub const fn from_fen_char(c: char) -> Option<Self> {
        let color = if c.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        match PieceKind::from_char(c) {
            Some(kind) => Some(Self { color, kind }),
            None => None,
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
        assert_eq!(Color::Black.sign(), -1);
    }

    #[test]
    fn fen_chars_round_trip() {
        for color in Color::ALL {
            for kind in PieceKind::ALL {
                let piece = Piece::new(color, kind);
                assert_eq!(Piece::from_fen_char(piece.to_fen_char()), Some(piece));
            }
        }
        assert_eq!(
            Piece::new(Color::White, PieceKind::Knight).to_fen_char(),
            'N'
        );
        assert_eq!(
            Piece::new(Color::Black, PieceKind::Queen).to_fen_char(),
            'q'
        );
        assert_eq!(Piece::from_fen_char('x'), None);
    }

    #[test]
    fn indices_are_distinct() {
        let mut seen = [false; 12];
        for color in Color::ALL {
            for kind in PieceKind::ALL {
                let index = Piece::new(color, kind).index();
                assert!(!seen[index]);
                seen[index] = true;
            }
        }
    }
}
