use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

use crate::{File, Rank, Square};

/// A set of squares packed in a `u64`, bit `i` standing for [`Square`] index `i`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Bitboard(pub u64);

impl Bitboard {
    /// The empty set.
    pub const EMPTY: Self = Self(0);
    /// All 64 squares.
    pub const FULL: Self = Self(u64::MAX);
    /// Light squares (`h1` is light).
    pub const LIGHT_SQUARES: Self = Self(0x55AA_55AA_55AA_55AA);
    /// Dark squares (`a1` is dark).
    pub const DARK_SQUARES: Self = Self(!0x55AA_55AA_55AA_55AA);

    /// The set containing only `square`.
    #[must_use]
    pub const fn from_square(square: Square) -> Self {
        Self(1 << square.index())
    }

    /// All squares of `file`.
    #[must_use]
    pub const fn file(file: File) -> Self {
        Self(0x0101_0101_0101_0101 << file.index())
    }

    /// All squares of `rank`.
    #[must_use]
    pub const fn rank(rank: Rank) -> Self {
        Self(0xFF << (rank.index() * 8))
    }

    /// Number of squares in the set.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether the set has at least two squares.
    #[must_use]
    pub const fn more_than_one(self) -> bool {
        self.0 & self.0.wrapping_sub(1) != 0
    }

    /// Whether `square` belongs to the set.
    #[must_use]
    pub const fn contains(self, square: Square) -> bool {
        self.0 & (1 << square.index()) != 0
    }

    /// The set with `square` added.
    #[must_use]
    pub const fn with(self, square: Square) -> Self {
        Self(self.0 | (1 << square.index()))
    }

    /// The set with `square` removed.
    #[must_use]
    pub const fn without(self, square: Square) -> Self {
        Self(self.0 & !(1 << square.index()))
    }

    /// Lowest square of the set (closest to `a1`).
    #[must_use]
    pub const fn lsb(self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square::from_u6(self.0.trailing_zeros() as u8))
        }
    }
}

/// Iterates over the squares of the set, from `a1` to `h8`.
impl Iterator for Bitboard {
    type Item = Square;

    fn next(&mut self) -> Option<Square> {
        let square = self.lsb()?;
        self.0 &= self.0 - 1;
        Some(square)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.0.count_ones() as usize;
        (count, Some(count))
    }
}

impl ExactSizeIterator for Bitboard {}

impl FromIterator<Square> for Bitboard {
    fn from_iter<I: IntoIterator<Item = Square>>(iter: I) -> Self {
        iter.into_iter().fold(Self::EMPTY, Self::with)
    }
}

macro_rules! impl_bit_op {
    ($trait:ident, $method:ident, $assign_trait:ident, $assign_method:ident, $op:tt) => {
        impl $trait for Bitboard {
            type Output = Self;
            fn $method(self, rhs: Self) -> Self {
                Self(self.0 $op rhs.0)
            }
        }
        impl $assign_trait for Bitboard {
            fn $assign_method(&mut self, rhs: Self) {
                self.0 = self.0 $op rhs.0;
            }
        }
    };
}

impl_bit_op!(BitAnd, bitand, BitAndAssign, bitand_assign, &);
impl_bit_op!(BitOr, bitor, BitOrAssign, bitor_assign, |);
impl_bit_op!(BitXor, bitxor, BitXorAssign, bitxor_assign, ^);

impl Not for Bitboard {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

/// Draws the set as an 8x8 grid, rank 8 on top (`X` = member).
impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Bitboard({:#018x})", self.0)?;
        for rank in (0..8).rev() {
            for file in 0..8 {
                let member = self.0 & (1 << (rank * 8 + file)) != 0;
                f.write_str(if member { " X" } else { " ." })?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_squares_in_order() {
        let bb = Bitboard::EMPTY
            .with(Square::H8)
            .with(Square::A1)
            .with(Square::E4);
        assert_eq!(bb.count(), 3);
        assert_eq!(bb.len(), 3);
        assert_eq!(
            bb.collect::<Vec<_>>(),
            vec![Square::A1, Square::E4, Square::H8]
        );
    }

    #[test]
    fn set_operations() {
        let a = Bitboard(0b1100);
        let b = Bitboard(0b1010);
        assert_eq!(a & b, Bitboard(0b1000));
        assert_eq!(a | b, Bitboard(0b1110));
        assert_eq!(a ^ b, Bitboard(0b0110));
        assert_eq!(!Bitboard::EMPTY, Bitboard::FULL);
        assert!(Bitboard::EMPTY.is_empty());
        assert!(a.more_than_one());
        assert!(!Bitboard(0b1000).more_than_one());
        assert_eq!(a.without(Square::C1), Bitboard(0b1000));
    }

    #[test]
    fn files_ranks_and_colors() {
        let file_e = Bitboard::file(Square::E4.file());
        let rank_4 = Bitboard::rank(Square::E4.rank());
        assert_eq!(file_e & rank_4, Bitboard::from_square(Square::E4));
        assert_eq!(file_e.count(), 8);
        assert!(Bitboard::DARK_SQUARES.contains(Square::A1));
        assert!(Bitboard::LIGHT_SQUARES.contains(Square::H1));
        assert_eq!(Bitboard::LIGHT_SQUARES.count(), 32);
    }
}
