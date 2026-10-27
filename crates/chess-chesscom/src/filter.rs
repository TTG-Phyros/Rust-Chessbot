//! Which games of a player to keep: time classes, period, number.

use serde::{Deserialize, Serialize};

use crate::model::{ApiGame, Month, days_from_civil};

/// Selection of games among a player's archives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameFilter {
    /// Time classes to keep; all of them when empty.
    pub time_classes: Vec<crate::TimeClass>,
    /// At most this many games, the most recent first.
    pub max_games: usize,
    /// Games that ended at or after this Unix time.
    pub since: Option<i64>,
    /// Games that ended before this Unix time.
    pub until: Option<i64>,
    /// Rated games only.
    pub rated_only: bool,
}

impl Default for GameFilter {
    fn default() -> Self {
        Self {
            time_classes: Vec::new(),
            max_games: 50,
            since: None,
            until: None,
            rated_only: false,
        }
    }
}

impl GameFilter {
    /// Whether `game` is kept (standard chess with a PGN, in the period,
    /// of a kept time class).
    #[must_use]
    pub fn matches(&self, game: &ApiGame) -> bool {
        game.is_standard()
            && (self.time_classes.is_empty() || self.time_classes.contains(&game.time_class))
            && self.since.is_none_or(|since| game.end_time >= since)
            && self.until.is_none_or(|until| game.end_time < until)
            && (!self.rated_only || game.rated)
    }

    /// The months of `available` that may hold games of the period, most
    /// recent first (the order in which to read them).
    #[must_use]
    pub fn months(&self, available: &[Month]) -> Vec<Month> {
        let mut months: Vec<Month> = available
            .iter()
            .copied()
            .filter(|month| {
                self.since.is_none_or(|since| month.end() > since)
                    && self.until.is_none_or(|until| month.start() < until)
            })
            .collect();
        months.sort_unstable_by(|a, b| b.cmp(a));
        months.dedup();
        months
    }
}

/// Unix time of midnight UTC on `YYYY-MM-DD`.
#[must_use]
pub fn parse_date(text: &str) -> Option<i64> {
    let mut parts = text.trim().splitn(3, '-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || day == 0 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    // Rejects 2026-02-30 and the like: the day must stay in its month.
    let first_of_next = Month { year, month }.end() / 86_400;
    (days < first_of_next).then_some(days * 86_400)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApiPlayer, TimeClass};

    fn game(time_class: TimeClass, end_time: i64, rules: &str) -> ApiGame {
        let player = |name: &str| ApiPlayer {
            username: name.into(),
            rating: 1500,
            result: "win".into(),
        };
        ApiGame {
            url: "u".into(),
            uuid: "id".into(),
            pgn: Some("1. e4 *".into()),
            time_control: "180".into(),
            end_time,
            rated: false,
            time_class,
            rules: rules.into(),
            white: player("a"),
            black: player("b"),
            accuracies: None,
        }
    }

    #[test]
    fn dates() {
        assert_eq!(parse_date("2026-10-01"), Some(1_790_812_800));
        assert_eq!(parse_date("1970-01-01"), Some(0));
        assert_eq!(parse_date("2026-02-29"), None);
        assert_eq!(parse_date("2024-02-29"), Some(1_709_164_800));
        assert_eq!(parse_date("2026-13-01"), None);
        assert_eq!(parse_date("yesterday"), None);
    }

    #[test]
    fn filters_games() {
        let filter = GameFilter {
            time_classes: vec![TimeClass::Blitz, TimeClass::Rapid],
            since: Some(100),
            until: Some(200),
            ..GameFilter::default()
        };
        assert!(filter.matches(&game(TimeClass::Blitz, 100, "chess")));
        assert!(!filter.matches(&game(TimeClass::Blitz, 200, "chess")));
        assert!(!filter.matches(&game(TimeClass::Bullet, 150, "chess")));
        assert!(!filter.matches(&game(TimeClass::Rapid, 150, "chess960")));
        let mut unrated = GameFilter::default();
        assert!(unrated.matches(&game(TimeClass::Daily, 0, "chess")));
        unrated.rated_only = true;
        assert!(!unrated.matches(&game(TimeClass::Daily, 0, "chess")));
    }

    #[test]
    fn months_of_the_period_newest_first() {
        let month = |m| Month {
            year: 2026,
            month: m,
        };
        let available = [month(1), month(2), month(3), month(5)];
        let all = GameFilter::default();
        assert_eq!(
            all.months(&available),
            [month(5), month(3), month(2), month(1)]
        );
        let period = GameFilter {
            since: parse_date("2026-02-15"),
            until: parse_date("2026-04-01"),
            ..GameFilter::default()
        };
        assert_eq!(period.months(&available), [month(3), month(2)]);
    }
}
