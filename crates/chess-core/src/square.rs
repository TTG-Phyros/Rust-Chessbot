use std::fmt;
use std::str::FromStr;

use crate::CoreError;

/// A file (column) of the board, `a` to `h`, stored as `0..8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct File(u8);

/// A rank (row) of the board, `1` to `8`, stored as `0..8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rank(u8);

impl File {
    /// Returns the file with the given zero-based index, if it is on the board.
    #[must_use]
    pub const fn new(index: u8) -> Option<Self> {
        if index < 8 { Some(Self(index)) } else { None }
    }

    /// Zero-based index (`a` = 0).
    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }

    /// Lowercase letter of the file.
    #[must_use]
    pub const fn to_char(self) -> char {
        (b'a' + self.0) as char
    }

    /// Parses a lowercase file letter.
    #[must_use]
    pub const fn from_char(c: char) -> Option<Self> {
        if c.is_ascii_lowercase() {
            Self::new((c as u8).wrapping_sub(b'a'))
        } else {
            None
        }
    }
}

impl Rank {
    /// Returns the rank with the given zero-based index, if it is on the board.
    #[must_use]
    pub const fn new(index: u8) -> Option<Self> {
        if index < 8 { Some(Self(index)) } else { None }
    }

    /// Zero-based index (rank 1 = 0).
    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }

    /// Digit of the rank (`'1'` to `'8'`).
    #[must_use]
    pub const fn to_char(self) -> char {
        (b'1' + self.0) as char
    }

    /// Parses a rank digit.
    #[must_use]
    pub const fn from_char(c: char) -> Option<Self> {
        if c.is_ascii_digit() {
            Self::new((c as u8).wrapping_sub(b'1'))
        } else {
            None
        }
    }
}

/// A square of the board, using little-endian rank-file mapping (`a1` = 0, `h8` = 63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square(u8);

macro_rules! square_consts {
    ($($name:ident = $index:literal),* $(,)?) => {
        /// Named squares.
        impl Square {
            $(
                #[allow(missing_docs)]
                pub const $name: Self = Self($index);
            )*
        }
    };
}

square_consts!(
    A1 = 0,
    B1 = 1,
    C1 = 2,
    D1 = 3,
    E1 = 4,
    F1 = 5,
    G1 = 6,
    H1 = 7,
    A2 = 8,
    B2 = 9,
    C2 = 10,
    D2 = 11,
    E2 = 12,
    F2 = 13,
    G2 = 14,
    H2 = 15,
    A3 = 16,
    B3 = 17,
    C3 = 18,
    D3 = 19,
    E3 = 20,
    F3 = 21,
    G3 = 22,
    H3 = 23,
    A4 = 24,
    B4 = 25,
    C4 = 26,
    D4 = 27,
    E4 = 28,
    F4 = 29,
    G4 = 30,
    H4 = 31,
    A5 = 32,
    B5 = 33,
    C5 = 34,
    D5 = 35,
    E5 = 36,
    F5 = 37,
    G5 = 38,
    H5 = 39,
    A6 = 40,
    B6 = 41,
    C6 = 42,
    D6 = 43,
    E6 = 44,
    F6 = 45,
    G6 = 46,
    H6 = 47,
    A7 = 48,
    B7 = 49,
    C7 = 50,
    D7 = 51,
    E7 = 52,
    F7 = 53,
    G7 = 54,
    H7 = 55,
    A8 = 56,
    B8 = 57,
    C8 = 58,
    D8 = 59,
    E8 = 60,
    F8 = 61,
    G8 = 62,
    H8 = 63,
);

impl Square {
    /// Number of squares on the board.
    pub const COUNT: usize = 64;

    /// Returns the square with the given index, if it is in `0..64`.
    #[must_use]
    pub const fn new(index: u8) -> Option<Self> {
        if index < 64 { Some(Self(index)) } else { None }
    }

    /// Square of index `index & 63`; for indices known to be on the board.
    pub(crate) const fn from_u6(index: u8) -> Self {
        Self(index & 63)
    }

    /// Builds a square from its file and rank.
    #[must_use]
    pub const fn from_coords(file: File, rank: Rank) -> Self {
        Self(rank.0 * 8 + file.0)
    }

    /// Index in `0..64`.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// File of the square.
    #[must_use]
    pub const fn file(self) -> File {
        File(self.0 & 7)
    }

    /// Rank of the square.
    #[must_use]
    pub const fn rank(self) -> Rank {
        Rank(self.0 >> 3)
    }

    /// The same square seen from the other side of the board (`a1` <-> `a8`).
    #[must_use]
    pub const fn flip_rank(self) -> Self {
        Self(self.0 ^ 56)
    }

    /// The square `delta` indices away, wrapping around the board.
    ///
    /// Only meaningful when the caller knows the destination is on the board
    /// (e.g. one rank forward for a pawn that is not on its last rank).
    pub(crate) const fn shifted(self, delta: i8) -> Self {
        Self::from_u6(self.0.wrapping_add_signed(delta))
    }

    /// All 64 squares, from `a1` to `h8`.
    pub fn all() -> impl ExactSizeIterator<Item = Self> + DoubleEndedIterator {
        (0..64).map(Self)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.file().to_char(), self.rank().to_char())
    }
}

impl FromStr for Square {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || CoreError::InvalidSquare(s.to_owned());
        let mut chars = s.chars();
        let (Some(file), Some(rank), None) = (chars.next(), chars.next(), chars.next()) else {
            return Err(invalid());
        };
        let file = File::from_char(file).ok_or_else(invalid)?;
        let rank = Rank::from_char(rank).ok_or_else(invalid)?;
        Ok(Self::from_coords(file, rank))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_round_trip() {
        for square in Square::all() {
            assert_eq!(square.to_string().parse::<Square>().unwrap(), square);
        }
        assert_eq!("a1".parse::<Square>().unwrap(), Square::A1);
        assert_eq!("h8".parse::<Square>().unwrap(), Square::H8);
        assert_eq!("e4".parse::<Square>().unwrap(), Square::E4);
    }

    #[test]
    fn rejects_invalid_names() {
        for name in ["", "a", "i1", "a9", "a0", "e44", "E4", "é4"] {
            assert!(name.parse::<Square>().is_err(), "{name} should be rejected");
        }
    }

    #[test]
    fn flip_rank_mirrors_vertically() {
        assert_eq!(Square::E2.flip_rank(), Square::E7);
    }

    #[test]
    fn coordinates() {
        assert_eq!(Square::C6.file().to_char(), 'c');
        assert_eq!(Square::C6.rank().to_char(), '6');
        assert_eq!(Square::E2.shifted(16), Square::E4);
        assert_eq!(Square::E4.shifted(-8), Square::E3);
    }
}
