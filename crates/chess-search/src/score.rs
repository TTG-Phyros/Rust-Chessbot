//! Search scores: centipawns, with mate scores encoded near the bounds.

use std::fmt;

/// Larger than any score; bounds the alpha-beta window.
pub(crate) const INFINITY: i32 = 32_000;
/// Score of delivering mate right now; mate in `n` plies scores `MATE - n`.
pub const MATE: i32 = 31_000;
/// Maximum search depth in plies.
pub const MAX_PLY: usize = 128;
/// Scores beyond this (in absolute value) are mate scores.
pub(crate) const MATE_BOUND: i32 = MATE - MAX_PLY as i32;
/// Score of a drawn position.
pub(crate) const DRAW: i32 = 0;

/// A score as reported to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Score {
    /// Advantage in centipawns for the side to move.
    Centipawns(i32),
    /// Mate in this many moves: positive if the side to move mates,
    /// negative if it gets mated.
    Mate(i32),
}

impl Score {
    /// Converts an internal search score.
    #[must_use]
    pub const fn from_internal(value: i32) -> Self {
        if value >= MATE_BOUND {
            Self::Mate((MATE - value + 1) / 2)
        } else if value <= -MATE_BOUND {
            Self::Mate(-(MATE + value) / 2)
        } else {
            Self::Centipawns(value)
        }
    }

    /// Whether this is a mate score.
    #[must_use]
    pub const fn is_mate(self) -> bool {
        matches!(self, Self::Mate(_))
    }
}

/// UCI spelling: `cp 35`, `mate 3`, `mate -2`.
impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Centipawns(cp) => write!(f, "cp {cp}"),
            Self::Mate(moves) => write!(f, "mate {moves}"),
        }
    }
}

/// Mate scores are stored in the transposition table relative to the node
/// (distance to mate from there), not to the root.
pub(crate) const fn to_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_BOUND {
        score + ply as i32
    } else if score <= -MATE_BOUND {
        score - ply as i32
    } else {
        score
    }
}

/// Inverse of [`to_tt`].
pub(crate) const fn from_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_BOUND {
        score - ply as i32
    } else if score <= -MATE_BOUND {
        score + ply as i32
    } else {
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mate_scores_in_moves() {
        assert_eq!(Score::from_internal(MATE - 1), Score::Mate(1));
        assert_eq!(Score::from_internal(MATE - 3), Score::Mate(2));
        assert_eq!(Score::from_internal(-MATE + 2), Score::Mate(-1));
        assert_eq!(Score::from_internal(-MATE + 4), Score::Mate(-2));
        assert_eq!(Score::from_internal(42), Score::Centipawns(42));
        assert_eq!(Score::Mate(-2).to_string(), "mate -2");
        assert_eq!(Score::Centipawns(-15).to_string(), "cp -15");
    }

    #[test]
    fn tt_conversion_round_trips() {
        for score in [MATE - 5, -MATE + 7, 120, -3000] {
            assert_eq!(from_tt(to_tt(score, 9), 9), score);
        }
        assert_eq!(to_tt(MATE - 5, 3), MATE - 2);
    }
}
