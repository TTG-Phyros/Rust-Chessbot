//! Shapes of the PubAPI responses, and the months that index the archives.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// `GET /player/{username}/games/archives`.
#[derive(Debug, Deserialize)]
pub(crate) struct ArchiveList {
    pub(crate) archives: Vec<String>,
}

/// `GET /player/{username}/games/{YYYY}/{MM}`.
#[derive(Debug, Deserialize)]
pub(crate) struct MonthlyGames {
    pub(crate) games: Vec<ApiGame>,
}

/// Speed category of a game, as Chess.com files it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeClass {
    /// Under 3 minutes.
    Bullet,
    /// 3 to 10 minutes.
    Blitz,
    /// 10 minutes or more.
    Rapid,
    /// Correspondence: days per move.
    Daily,
}

impl TimeClass {
    /// Every time class, fastest first.
    pub const ALL: [Self; 4] = [Self::Bullet, Self::Blitz, Self::Rapid, Self::Daily];

    /// Name used by the API (`blitz`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bullet => "bullet",
            Self::Blitz => "blitz",
            Self::Rapid => "rapid",
            Self::Daily => "daily",
        }
    }
}

impl fmt::Display for TimeClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TimeClass {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|class| class.as_str().eq_ignore_ascii_case(text.trim()))
            .ok_or_else(|| format!("unknown time class {text:?} (bullet, blitz, rapid or daily)"))
    }
}

/// One player of an archived game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiPlayer {
    /// Username, as the player spells it.
    pub username: String,
    /// Rating in this time class after the game.
    pub rating: i32,
    /// How the game ended for them: `win`, `checkmated`, `resigned`,
    /// `timeout`, `agreed`, `repetition`, `stalemate`…
    pub result: String,
}

/// Accuracies computed by Chess.com's own review, when someone ran it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Accuracies {
    /// White's accuracy, 0 to 100.
    pub white: f64,
    /// Black's accuracy, 0 to 100.
    pub black: f64,
}

/// A game of a monthly archive (only the fields used here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiGame {
    /// Page of the game on chess.com.
    pub url: String,
    /// Unique identifier (missing from a few old games: see [`ApiGame::id`]).
    #[serde(default)]
    pub uuid: String,
    /// Full PGN, with `[%clk]` comments (missing for some aborted games).
    #[serde(default)]
    pub pgn: Option<String>,
    /// `180+2`, `600`, `1/86400` (as the PGN `TimeControl` tag).
    pub time_control: String,
    /// End of the game, Unix seconds.
    pub end_time: i64,
    /// Rated game.
    #[serde(default)]
    pub rated: bool,
    /// Speed category.
    pub time_class: TimeClass,
    /// `chess` for standard chess; `chess960`, `bughouse`… otherwise.
    pub rules: String,
    /// Player with the white pieces.
    pub white: ApiPlayer,
    /// Player with the black pieces.
    pub black: ApiPlayer,
    /// Chess.com's accuracies, if the game was reviewed there.
    #[serde(default)]
    pub accuracies: Option<Accuracies>,
}

impl ApiGame {
    /// Stable identifier: the UUID, or the URL for the games without one.
    #[must_use]
    pub fn id(&self) -> &str {
        if self.uuid.is_empty() {
            &self.url
        } else {
            &self.uuid
        }
    }

    /// Whether the game can be analysed: standard chess with its moves.
    #[must_use]
    pub fn is_standard(&self) -> bool {
        self.rules == "chess"
            && self
                .pgn
                .as_deref()
                .is_some_and(|pgn| !pgn.trim().is_empty())
    }
}

/// A calendar month (UTC): Chess.com files games by the month they ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Month {
    /// Year, e.g. 2026.
    pub year: i32,
    /// Month, 1 to 12.
    pub month: u8,
}

impl Month {
    /// The month of an archive URL (`…/games/2026/10`).
    #[must_use]
    pub fn from_archive_url(url: &str) -> Option<Self> {
        let mut parts = url.trim_end_matches('/').rsplit('/');
        let month: u8 = parts.next()?.parse().ok()?;
        let year: i32 = parts.next()?.parse().ok()?;
        (1..=12).contains(&month).then_some(Self { year, month })
    }

    /// The month containing Unix time `seconds`.
    #[must_use]
    pub fn containing(seconds: i64) -> Self {
        let (year, month, _) = civil_from_days(seconds.div_euclid(86_400));
        Self { year, month }
    }

    /// The following month.
    #[must_use]
    pub const fn next(self) -> Self {
        if self.month == 12 {
            Self {
                year: self.year + 1,
                month: 1,
            }
        } else {
            Self {
                year: self.year,
                month: self.month + 1,
            }
        }
    }

    /// First second of the month, Unix time.
    #[must_use]
    pub fn start(self) -> i64 {
        days_from_civil(self.year, self.month, 1) * 86_400
    }

    /// First second after the month, Unix time.
    #[must_use]
    pub fn end(self) -> i64 {
        self.next().start()
    }
}

impl fmt::Display for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

impl FromStr for Month {
    type Err = String;

    /// `2026-10`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || format!("invalid month {text:?} (expected YYYY-MM)");
        let (year, month) = text.split_once('-').ok_or_else(invalid)?;
        let year = year.parse().map_err(|_| invalid())?;
        let month = month.parse().map_err(|_| invalid())?;
        if (1..=12).contains(&month) {
            Ok(Self { year, month })
        } else {
            Err(invalid())
        }
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's
/// `days_from_civil`).
pub(crate) fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Inverse of [`days_from_civil`]: (year, month, day).
pub(crate) fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    // In range for any date a chess game can have.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (year as i32, month as u8, day as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_of_archive_urls() {
        let month =
            Month::from_archive_url("https://api.chess.com/pub/player/hikaru/games/2026/03")
                .unwrap();
        assert_eq!(
            month,
            Month {
                year: 2026,
                month: 3
            }
        );
        assert_eq!(month.to_string(), "2026-03");
        assert_eq!("2026-03".parse::<Month>(), Ok(month));
        assert!(
            Month::from_archive_url("https://api.chess.com/pub/player/x/games/2026/13").is_none()
        );
        assert!("2026-00".parse::<Month>().is_err());
    }

    #[test]
    fn month_bounds() {
        // 2026-10-01T00:00:00Z
        let october = Month {
            year: 2026,
            month: 10,
        };
        assert_eq!(october.start(), 1_790_812_800);
        assert_eq!(Month::containing(october.start()), october);
        assert_eq!(
            Month::containing(october.start() - 1),
            Month {
                year: 2026,
                month: 9
            }
        );
        assert_eq!(
            Month {
                year: 2025,
                month: 12
            }
            .next(),
            Month {
                year: 2026,
                month: 1
            }
        );
        assert_eq!(
            Month {
                year: 2024,
                month: 2
            }
            .end()
                - Month {
                    year: 2024,
                    month: 2
                }
                .start(),
            29 * 86_400
        );
        for days in [-1, 0, 11_016, 20_000, 30_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    #[test]
    fn decodes_an_archived_game() {
        let json = r#"{
            "url": "https://www.chess.com/game/live/123",
            "pgn": "[Event \"Live Chess\"]\n\n1. e4 *",
            "time_control": "180+2",
            "end_time": 1790900000,
            "rated": true,
            "accuracies": {"white": 81.2, "black": 77.9},
            "tcn": "mC",
            "uuid": "0b1c",
            "initial_setup": "",
            "fen": "",
            "time_class": "blitz",
            "rules": "chess",
            "white": {"rating": 1500, "result": "win", "@id": "x", "username": "Alice", "uuid": "a"},
            "black": {"rating": 1490, "result": "resigned", "@id": "y", "username": "bob", "uuid": "b"}
        }"#;
        let game: ApiGame = serde_json::from_str(json).unwrap();
        assert_eq!(game.id(), "0b1c");
        assert_eq!(game.time_class, TimeClass::Blitz);
        assert_eq!(game.white.username, "Alice");
        assert_eq!(game.accuracies.map(|a| a.black), Some(77.9));
        assert!(game.is_standard());
        assert_eq!("Rapid".parse::<TimeClass>(), Ok(TimeClass::Rapid));
    }
}
