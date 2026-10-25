use std::fmt;
use std::str::FromStr;

use crate::CoreError;

/// A file (column) of the board, `A` to `H`.
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
}

/// A square of the board, using little-endian rank-file mapping (`a1` = 0, `h8` = 63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square(u8);

impl Square {
    /// Number of squares on the board.
    pub const COUNT: usize = 64;

    /// Returns the square with the given index, if it is in `0..64`.
    #[must_use]
    pub const fn new(index: u8) -> Option<Self> {
        if index < 64 { Some(Self(index)) } else { None }
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
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = char::from(b'a' + self.file().0);
        let rank = char::from(b'1' + self.rank().0);
        write!(f, "{file}{rank}")
    }
}

impl FromStr for Square {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || CoreError::InvalidSquare(s.to_owned());
        let &[file, rank] = s.as_bytes() else {
            return Err(invalid());
        };
        let file = File::new(file.wrapping_sub(b'a')).ok_or_else(invalid)?;
        let rank = Rank::new(rank.wrapping_sub(b'1')).ok_or_else(invalid)?;
        Ok(Self::from_coords(file, rank))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_round_trip() {
        for index in 0..64 {
            let square = Square::new(index).unwrap();
            assert_eq!(square.to_string().parse::<Square>().unwrap(), square);
        }
        assert_eq!("a1".parse::<Square>().unwrap().index(), 0);
        assert_eq!("h8".parse::<Square>().unwrap().index(), 63);
        assert_eq!("e4".parse::<Square>().unwrap().index(), 28);
    }

    #[test]
    fn rejects_invalid_names() {
        for name in ["", "a", "i1", "a9", "a0", "e44", "E4"] {
            assert!(name.parse::<Square>().is_err(), "{name} should be rejected");
        }
    }

    #[test]
    fn flip_rank_mirrors_vertically() {
        let e2: Square = "e2".parse().unwrap();
        assert_eq!(e2.flip_rank().to_string(), "e7");
    }
}
