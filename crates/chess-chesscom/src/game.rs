//! An archived game made ready for analysis: PGN parsed, clocks decoded.

use std::time::Duration;

use chess_core::Color;
use chess_core::pgn::{Game, GameResult, PgnError, TimeControl, clock, parse_pgn};

use crate::model::{ApiGame, TimeClass};

/// One side of a [`PlayedGame`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    /// Username, as the player spells it.
    pub username: String,
    /// Rating in the game's time class, after the game.
    pub rating: i32,
}

/// A Chess.com game with its moves.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayedGame {
    /// Identifier (see [`ApiGame::id`]).
    pub id: String,
    /// Page of the game on chess.com.
    pub url: String,
    /// End of the game, Unix seconds.
    pub end_time: i64,
    /// Speed category.
    pub time_class: TimeClass,
    /// Decoded `time_control`, when it has a usual form.
    pub time_control: Option<TimeControl>,
    /// Rated game.
    pub rated: bool,
    /// Player with the white pieces.
    pub white: Player,
    /// Player with the black pieces.
    pub black: Player,
    /// Result, from the PGN.
    pub result: GameResult,
    /// Tags, start position and moves.
    pub game: Game,
    /// Clock of the player who just moved, after each move (`[%clk]`).
    pub clocks: Vec<Option<Duration>>,
    /// Chess.com's own accuracies (white, black), if it reviewed the game.
    pub chesscom_accuracies: Option<(f64, f64)>,
}

impl PlayedGame {
    /// Parses the PGN of an archived game.
    ///
    /// # Errors
    ///
    /// When the game has no PGN or its PGN does not hold exactly one valid
    /// game.
    pub fn from_api(api: &ApiGame) -> Result<Self, PgnError> {
        let pgn = api.pgn.as_deref().unwrap_or_default();
        let mut games = parse_pgn(pgn)?;
        let game = match (games.pop(), games.is_empty()) {
            (Some(game), true) => game,
            _ => {
                return Err(PgnError {
                    line: 1,
                    message: "expected exactly one game".into(),
                });
            }
        };
        let clocks = (0..game.moves().len())
            .map(|i| game.comment(i).and_then(clock))
            .collect();
        Ok(Self {
            id: api.id().to_owned(),
            url: api.url.clone(),
            end_time: api.end_time,
            time_class: api.time_class,
            time_control: TimeControl::from_pgn(&api.time_control),
            rated: api.rated,
            white: Player {
                username: api.white.username.clone(),
                rating: api.white.rating,
            },
            black: Player {
                username: api.black.username.clone(),
                rating: api.black.rating,
            },
            result: game.result(),
            game,
            clocks,
            chesscom_accuracies: api.accuracies.map(|a| (a.white, a.black)),
        })
    }

    /// Colour played by `username` (case-insensitive), if they played.
    #[must_use]
    pub fn color_of(&self, username: &str) -> Option<Color> {
        if self.white.username.eq_ignore_ascii_case(username) {
            Some(Color::White)
        } else if self.black.username.eq_ignore_ascii_case(username) {
            Some(Color::Black)
        } else {
            None
        }
    }

    /// The player of `color`.
    #[must_use]
    pub const fn player(&self, color: Color) -> &Player {
        match color {
            Color::White => &self.white,
            Color::Black => &self.black,
        }
    }

    /// Score of `color`: 1 for a win, ½ for a draw, 0 for a loss, `None`
    /// when the PGN records no result.
    #[must_use]
    pub fn score(&self, color: Color) -> Option<f64> {
        let white = match self.result {
            GameResult::WhiteWins => 1.0,
            GameResult::BlackWins => 0.0,
            GameResult::Draw => 0.5,
            GameResult::Unfinished => return None,
        };
        Some(match color {
            Color::White => white,
            Color::Black => 1.0 - white,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiPlayer;

    const PGN: &str = "[Event \"Live Chess\"]\n[Site \"Chess.com\"]\n[White \"Alice\"]\n\
        [Black \"Bob\"]\n[Result \"1/2-1/2\"]\n[TimeControl \"300+5\"]\n\n\
        1. d4 {[%clk 0:05:04.1]} 1... d5 {[%clk 0:05:03]} 2. Bf4 {[%clk 0:05:06]} \
        2... Nf6 1/2-1/2\n";

    fn api(pgn: &str) -> ApiGame {
        ApiGame {
            url: "https://www.chess.com/game/live/1".into(),
            uuid: "abc".into(),
            pgn: Some(pgn.into()),
            time_control: "300+5".into(),
            end_time: 1_790_900_000,
            rated: true,
            time_class: TimeClass::Blitz,
            rules: "chess".into(),
            white: ApiPlayer {
                username: "Alice".into(),
                rating: 1612,
                result: "agreed".into(),
            },
            black: ApiPlayer {
                username: "Bob".into(),
                rating: 1588,
                result: "agreed".into(),
            },
            accuracies: None,
        }
    }

    #[test]
    fn parses_an_archived_game() {
        let game = PlayedGame::from_api(&api(PGN)).unwrap();
        assert_eq!(game.id, "abc");
        assert_eq!(game.game.moves().len(), 4);
        assert_eq!(game.result, GameResult::Draw);
        assert_eq!(
            game.clocks,
            [
                Some(Duration::from_millis(304_100)),
                Some(Duration::from_secs(303)),
                Some(Duration::from_secs(306)),
                None,
            ]
        );
        assert_eq!(
            game.time_control.map(TimeControl::initial),
            Some(Duration::from_secs(300))
        );
        assert_eq!(game.color_of("alice"), Some(Color::White));
        assert_eq!(game.color_of("BOB"), Some(Color::Black));
        assert_eq!(game.color_of("carol"), None);
        assert_eq!(game.player(Color::Black).rating, 1588);
        assert_eq!(game.score(Color::Black), Some(0.5));
    }

    #[test]
    fn rejects_broken_pgn() {
        assert!(PlayedGame::from_api(&api("1. e5 *")).is_err());
        assert!(PlayedGame::from_api(&api("1. e4 * 1. d4 *")).is_err());
    }
}
