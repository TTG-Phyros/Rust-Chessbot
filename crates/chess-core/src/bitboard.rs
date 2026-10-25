use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

use crate::Square;

/// A set of squares packed in a `u64`, bit `i` standing for [`Square`] index `i`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Bitboard(pub u64);

impl Bitboard {
    /// The empty set.
    pub const EMPTY: Self = Self(0);
    /// All 64 squares.
    pub const FULL: Self = Self(u64::MAX);

    /// The set containing only `square`.
    #[must_use]
    pub const fn from_square(square: Square) -> Self {
        Self(1 << square.index())
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
}

/// Iterates over the squares of the set, from `a1` to `h8`.
impl Iterator for Bitboard {
    type Item = Square;

    fn next(&mut self) -> Option<Square> {
        if self.0 == 0 {
            return None;
        }
        // trailing_zeros of a non-zero u64 is < 64, so the conversion cannot fail.
        let square = Square::new(self.0.trailing_zeros() as u8);
        self.0 &= self.0 - 1;
        square
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.0.count_ones() as usize;
        (count, Some(count))
    }
}

impl ExactSizeIterator for Bitboard {}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_squares_in_order() {
        let a1 = Square::new(0).unwrap();
        let e4 = Square::new(28).unwrap();
        let h8 = Square::new(63).unwrap();
        let bb = Bitboard::EMPTY.with(h8).with(a1).with(e4);
        assert_eq!(bb.count(), 3);
        assert_eq!(bb.len(), 3);
        assert_eq!(bb.collect::<Vec<_>>(), vec![a1, e4, h8]);
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
    }
}
