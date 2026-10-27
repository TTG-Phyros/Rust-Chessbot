//! Search limits (the UCI `go` parameters) and time management.

use std::time::Duration;

use chess_core::Color;

/// When to stop searching. Every limit set applies; with none, the search
/// goes on until stopped (or the maximum depth is reached).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchLimits {
    /// Maximum depth in plies.
    pub depth: Option<u32>,
    /// Maximum number of nodes.
    pub nodes: Option<u64>,
    /// Fixed time for this move.
    pub movetime: Option<Duration>,
    /// White's remaining clock time.
    pub white_time: Option<Duration>,
    /// Black's remaining clock time.
    pub black_time: Option<Duration>,
    /// White's increment per move.
    pub white_increment: Option<Duration>,
    /// Black's increment per move.
    pub black_increment: Option<Duration>,
    /// Moves until the next time control (sudden death if `None`).
    pub moves_to_go: Option<u32>,
    /// Search until stopped, ignoring the clocks.
    pub infinite: bool,
    /// Time kept in reserve for communication delays (GUI, network).
    pub move_overhead: Duration,
    /// Number of best lines to find (UCI `MultiPV`); 0 and 1 both mean one.
    pub multipv: usize,
}

impl SearchLimits {
    /// Search to a fixed depth.
    #[must_use]
    pub fn depth(depth: u32) -> Self {
        Self {
            depth: Some(depth),
            ..Self::default()
        }
    }

    /// Search a fixed number of nodes.
    #[must_use]
    pub fn nodes(nodes: u64) -> Self {
        Self {
            nodes: Some(nodes),
            ..Self::default()
        }
    }

    /// Search for a fixed time.
    #[must_use]
    pub fn movetime(time: Duration) -> Self {
        Self {
            movetime: Some(time),
            ..Self::default()
        }
    }

    /// Whether any limit uses the wall clock.
    #[must_use]
    pub fn is_timed(&self) -> bool {
        !self.infinite
            && (self.movetime.is_some() || self.white_time.is_some() || self.black_time.is_some())
    }
}

/// How long the search may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TimeBudget {
    /// Do not start a new iteration after this.
    pub(crate) soft: Option<Duration>,
    /// Abort the search in progress after this.
    pub(crate) hard: Option<Duration>,
}

/// Assumed number of moves left when the time control does not say.
const DEFAULT_MOVES_TO_GO: u32 = 25;

impl TimeBudget {
    pub(crate) fn new(limits: &SearchLimits, side: Color) -> Self {
        if limits.infinite {
            return Self::default();
        }
        if let Some(movetime) = limits.movetime {
            let time = movetime
                .saturating_sub(limits.move_overhead)
                .max(Duration::from_millis(1));
            return Self {
                soft: Some(time),
                hard: Some(time),
            };
        }
        let (time, increment) = match side {
            Color::White => (limits.white_time, limits.white_increment),
            Color::Black => (limits.black_time, limits.black_increment),
        };
        let Some(time) = time else {
            return Self::default();
        };
        let increment = increment.unwrap_or_default();
        let available = time.saturating_sub(limits.move_overhead);
        let moves_to_go = limits
            .moves_to_go
            .unwrap_or(DEFAULT_MOVES_TO_GO)
            .clamp(1, 50);

        // Spend an even share of the clock plus most of the increment, never
        // more than a fraction of what is left.
        let share = available / moves_to_go + increment * 3 / 4;
        let soft = share.min(available * 2 / 5);
        let hard = (share * 3).min(available * 3 / 5).max(soft);
        let floor = Duration::from_millis(1);
        Self {
            soft: Some(soft.max(floor)),
            hard: Some(hard.max(floor)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn movetime_is_exact_minus_overhead() {
        let limits = SearchLimits {
            movetime: Some(MS(1000)),
            move_overhead: MS(30),
            ..SearchLimits::default()
        };
        let budget = TimeBudget::new(&limits, Color::White);
        assert_eq!(budget.soft, Some(MS(970)));
        assert_eq!(budget.hard, Some(MS(970)));
    }

    #[test]
    fn clock_uses_the_side_to_move() {
        let limits = SearchLimits {
            white_time: Some(MS(60_000)),
            black_time: Some(MS(1_000)),
            white_increment: Some(MS(1_000)),
            ..SearchLimits::default()
        };
        let white = TimeBudget::new(&limits, Color::White);
        let black = TimeBudget::new(&limits, Color::Black);
        assert_eq!(white.soft, Some(MS(60_000 / 25 + 750)));
        assert!(black.soft < white.soft);
        let black_hard = black.hard.unwrap();
        assert!(black_hard <= MS(600), "{black_hard:?}");
        assert!(white.hard >= white.soft);
    }

    #[test]
    fn no_clock_means_no_budget() {
        assert_eq!(
            TimeBudget::new(&SearchLimits::depth(5), Color::White),
            TimeBudget::default()
        );
        let infinite = SearchLimits {
            infinite: true,
            white_time: Some(MS(10)),
            ..SearchLimits::default()
        };
        assert_eq!(
            TimeBudget::new(&infinite, Color::White),
            TimeBudget::default()
        );
        assert!(!infinite.is_timed());
    }

    #[test]
    fn tiny_clock_still_leaves_time_to_move() {
        let limits = SearchLimits {
            white_time: Some(MS(5)),
            move_overhead: MS(30),
            ..SearchLimits::default()
        };
        let budget = TimeBudget::new(&limits, Color::White);
        assert_eq!(budget.hard, Some(MS(1)));
    }
}
