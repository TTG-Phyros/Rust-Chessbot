//! Portable Game Notation import and export.
//!
//! The parser accepts real-world PGN: several games per text, comments
//! (`{...}` and `;`), variations (skipped), NAGs (`$1`), move numbers glued
//! to moves (`12.e4`), and games starting from a `FEN` tag.

use std::fmt::Write as _;

use thiserror::Error;

use crate::{Color, CoreError, GameStatus, Move, Position};

/// Error while reading PGN.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("PGN line {line}: {message}")]
pub struct PgnError {
    /// 1-based line where the problem was found.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

/// Outcome recorded for a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameResult {
    /// `1-0`.
    WhiteWins,
    /// `0-1`.
    BlackWins,
    /// `1/2-1/2`.
    Draw,
    /// `*`: unfinished or unknown.
    #[default]
    Unfinished,
}

impl GameResult {
    /// PGN spelling of the result.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WhiteWins => "1-0",
            Self::BlackWins => "0-1",
            Self::Draw => "1/2-1/2",
            Self::Unfinished => "*",
        }
    }

    /// Parses the PGN spelling of a result.
    #[must_use]
    pub fn from_pgn(text: &str) -> Option<Self> {
        match text {
            "1-0" => Some(Self::WhiteWins),
            "0-1" => Some(Self::BlackWins),
            "1/2-1/2" => Some(Self::Draw),
            "*" => Some(Self::Unfinished),
            _ => None,
        }
    }

    /// Result implied by the state of the final position.
    #[must_use]
    pub const fn from_status(status: GameStatus) -> Self {
        match status {
            GameStatus::Ongoing => Self::Unfinished,
            GameStatus::Checkmate {
                winner: Color::White,
            } => Self::WhiteWins,
            GameStatus::Checkmate {
                winner: Color::Black,
            } => Self::BlackWins,
            GameStatus::Stalemate | GameStatus::Draw(_) => Self::Draw,
        }
    }
}

/// A game: tags, starting position, moves and result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    tags: Vec<(String, String)>,
    start: Position,
    current: Position,
    moves: Vec<Move>,
    result: GameResult,
}

/// Tags every exported game carries, in this order (PGN "Seven Tag Roster").
const SEVEN_TAG_ROSTER: [(&str, &str); 6] = [
    ("Event", "?"),
    ("Site", "?"),
    ("Date", "????.??.??"),
    ("Round", "?"),
    ("White", "?"),
    ("Black", "?"),
];

impl Game {
    /// An empty game from the standard starting position.
    #[must_use]
    pub fn new() -> Self {
        Self::from_position(&Position::startpos())
    }

    /// An empty game starting from `start` (its history is discarded).
    #[must_use]
    pub fn from_position(start: &Position) -> Self {
        let start = start.without_history();
        Self {
            tags: Vec::new(),
            current: start.clone(),
            start,
            moves: Vec::new(),
            result: GameResult::Unfinished,
        }
    }

    /// Starting position.
    #[must_use]
    pub const fn start(&self) -> &Position {
        &self.start
    }

    /// Position after all the moves, with the game's moves as history
    /// (so repetitions and [`Position::unmake_move`] work).
    #[must_use]
    pub const fn position(&self) -> &Position {
        &self.current
    }

    /// Moves played, in order.
    #[must_use]
    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    /// Recorded result.
    #[must_use]
    pub const fn result(&self) -> GameResult {
        self.result
    }

    /// Records the result.
    pub fn set_result(&mut self, result: GameResult) {
        self.result = result;
    }

    /// Tags in their original order (the result is not stored as a tag).
    #[must_use]
    pub fn tags(&self) -> &[(String, String)] {
        &self.tags
    }

    /// Value of tag `name`.
    #[must_use]
    pub fn tag(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Sets tag `name`, replacing any previous value.
    pub fn set_tag(&mut self, name: &str, value: &str) {
        match self.tags.iter_mut().find(|(key, _)| key == name) {
            Some(entry) => entry.1 = value.to_owned(),
            None => self.tags.push((name.to_owned(), value.to_owned())),
        }
    }

    /// Plays `mv` at the end of the game.
    ///
    /// # Errors
    ///
    /// [`CoreError::IllegalMove`] if `mv` is not legal in the current position.
    pub fn push(&mut self, mv: Move) -> Result<(), CoreError> {
        self.current.play(mv)?;
        self.moves.push(mv);
        Ok(())
    }

    /// Takes back the last move. Returns it, or `None` if there was none.
    pub fn pop(&mut self) -> Option<Move> {
        let mv = self.moves.pop()?;
        self.current.unmake_move();
        Some(mv)
    }

    /// Exports the game as PGN, moves in SAN, lines wrapped at 80 columns.
    #[must_use]
    pub fn to_pgn(&self) -> String {
        let mut pgn = String::new();
        let mut write_tag = |name: &str, value: &str| {
            let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
            let _ = writeln!(pgn, "[{name} \"{escaped}\"]");
        };
        for (name, default) in SEVEN_TAG_ROSTER {
            write_tag(name, self.tag(name).unwrap_or(default));
        }
        write_tag("Result", self.result.as_str());
        let custom_start = self.start != Position::startpos();
        if custom_start {
            write_tag("SetUp", "1");
            write_tag("FEN", &self.start.to_fen());
        }
        let reserved = |name: &str| {
            SEVEN_TAG_ROSTER.iter().any(|(key, _)| *key == name)
                || matches!(name, "Result" | "SetUp" | "FEN")
        };
        for (name, value) in &self.tags {
            if !reserved(name) {
                write_tag(name, value);
            }
        }
        pgn.push('\n');

        let mut tokens = Vec::with_capacity(self.moves.len() * 2 + 1);
        let mut pos = self.start.without_history();
        for (i, &mv) in self.moves.iter().enumerate() {
            let number = pos.fullmove_number();
            match pos.side_to_move() {
                Color::White => tokens.push(format!("{number}.")),
                Color::Black if i == 0 => tokens.push(format!("{number}...")),
                Color::Black => {}
            }
            tokens.push(pos.san(mv));
            pos.make_move(mv);
        }
        tokens.push(self.result.as_str().to_owned());

        let mut line_len = 0;
        for token in tokens {
            if line_len > 0 && line_len + 1 + token.len() > 80 {
                pgn.push('\n');
                line_len = 0;
            } else if line_len > 0 {
                pgn.push(' ');
                line_len += 1;
            }
            line_len += token.len();
            pgn.push_str(&token);
        }
        pgn.push('\n');
        pgn
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

/// Parses every game of a PGN text.
///
/// # Errors
///
/// [`PgnError`] for malformed syntax, an invalid `FEN` tag or an illegal or
/// ambiguous move, with the line where it occurred.
pub fn parse_pgn(text: &str) -> Result<Vec<Game>, PgnError> {
    let mut games = Vec::new();
    let mut builder = Builder::default();
    for (token, line) in tokenize(text)? {
        match token {
            Token::Tag(name, value) => {
                if builder.game.is_some() {
                    games.push(std::mem::take(&mut builder).finish(line)?);
                }
                builder.tags.push((name, value));
            }
            Token::Move(san) => {
                let game = builder.game(line)?;
                let mv = game.current.parse_san(san).map_err(|err| PgnError {
                    line,
                    message: err.to_string(),
                })?;
                game.current.make_move(mv);
                game.moves.push(mv);
            }
            Token::Result(result) => {
                builder.game(line)?.result = result;
                builder.ended = true;
                games.push(std::mem::take(&mut builder).finish(line)?);
            }
        }
    }
    if !builder.tags.is_empty() || builder.game.is_some() {
        let last_line = text.lines().count().max(1);
        games.push(builder.finish(last_line)?);
    }
    Ok(games)
}

#[derive(Default)]
struct Builder {
    tags: Vec<(String, String)>,
    game: Option<Game>,
    ended: bool,
}

impl Builder {
    /// The game being read, created from the tags on its first move.
    fn game(&mut self, line: usize) -> Result<&mut Game, PgnError> {
        if self.game.is_none() {
            let start = match self.tags.iter().find(|(name, _)| name == "FEN") {
                Some((_, fen)) => Position::from_fen(fen).map_err(|err| PgnError {
                    line,
                    message: format!("FEN tag: {err}"),
                })?,
                None => Position::startpos(),
            };
            let mut game = Game::from_position(&start);
            game.tags = std::mem::take(&mut self.tags);
            game.result = game
                .tag("Result")
                .and_then(GameResult::from_pgn)
                .unwrap_or_default();
            self.game = Some(game);
        }
        // Just created above when missing.
        self.game.as_mut().ok_or(PgnError {
            line,
            message: "internal error".into(),
        })
    }

    fn finish(mut self, line: usize) -> Result<Game, PgnError> {
        let ended = self.ended;
        let game = self.game(line)?;
        if !ended {
            game.result = game
                .tag("Result")
                .and_then(GameResult::from_pgn)
                .unwrap_or_default();
        }
        game.tags.retain(|(name, _)| name != "Result");
        self.game.ok_or(PgnError {
            line,
            message: "internal error".into(),
        })
    }
}

enum Token<'a> {
    Tag(String, String),
    Move(&'a str),
    Result(GameResult),
}

fn tokenize(text: &str) -> Result<Vec<(Token<'_>, usize)>, PgnError> {
    let mut tokens = Vec::new();
    let mut line = 1;
    let bytes = text.as_bytes();
    let mut i = 0;
    let error = |line, message: &str| PgnError {
        line,
        message: message.to_owned(),
    };

    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'\n' => {
                line += 1;
                i += 1;
            }
            _ if c.is_ascii_whitespace() => i += 1,
            // Escape mechanism: a line starting with `%` is ignored.
            b'%' if i == 0 || bytes[i - 1] == b'\n' => i = skip_line(bytes, i),
            b';' => i = skip_line(bytes, i),
            b'{' => {
                let end =
                    find(bytes, i, b'}').ok_or_else(|| error(line, "unterminated comment"))?;
                line += count_newlines(&bytes[i..end]);
                i = end + 1;
            }
            b'(' => {
                let end = skip_variation(bytes, i)
                    .ok_or_else(|| error(line, "unterminated variation"))?;
                line += count_newlines(&bytes[i..end]);
                i = end;
            }
            b')' => return Err(error(line, "unbalanced ')'")),
            b'[' => {
                let (name, value, end) =
                    parse_tag(text, i).ok_or_else(|| error(line, "malformed tag"))?;
                tokens.push((Token::Tag(name, value), line));
                i = end;
            }
            b'$' => {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            _ => {
                let start = i;
                while i < bytes.len()
                    && !bytes[i].is_ascii_whitespace()
                    && !b"{}()[];$".contains(&bytes[i])
                {
                    i += 1;
                }
                let word = &text[start..i];
                if let Some(result) = GameResult::from_pgn(word) {
                    tokens.push((Token::Result(result), line));
                } else {
                    // Move numbers (`12.`, `12...`) may be glued to the move (`12.e4`).
                    let san = word.trim_start_matches(|c: char| c.is_ascii_digit());
                    let san = if san.len() < word.len() && san.starts_with('.') {
                        san.trim_start_matches('.')
                    } else {
                        word
                    };
                    if !san.is_empty() {
                        tokens.push((Token::Move(san), line));
                    }
                }
            }
        }
    }
    Ok(tokens)
}

fn skip_line(bytes: &[u8], from: usize) -> usize {
    find(bytes, from, b'\n').unwrap_or(bytes.len())
}

fn find(bytes: &[u8], from: usize, needle: u8) -> Option<usize> {
    bytes[from..]
        .iter()
        .position(|&b| b == needle)
        .map(|offset| from + offset)
}

fn count_newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b == b'\n').count()
}

/// Index just past the `)` closing the variation opened at `from`.
fn skip_variation(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            b'{' => i = find(bytes, i, b'}')?,
            b';' => i = skip_line(bytes, i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Parses `[Name "value"]` at `from`; returns name, unescaped value, and
/// the index past `]`.
fn parse_tag(text: &str, from: usize) -> Option<(String, String, usize)> {
    let rest = text.get(from + 1..)?;
    let name_len = rest.find(|c: char| c.is_whitespace() || c == '"')?;
    let name = rest[..name_len].to_owned();
    if name.is_empty() {
        return None;
    }
    let after_name = rest[name_len..].trim_start();
    let mut chars = after_name.strip_prefix('"')?.char_indices();
    let mut value = String::new();
    let close = loop {
        match chars.next()? {
            (_, '\\') => value.push(chars.next()?.1),
            (index, '"') => break index,
            (_, c) => value.push(c),
        }
    };
    let after_value = &after_name[1 + close + 1..];
    let bracket = after_value.find(']')?;
    if !after_value[..bracket].trim().is_empty() {
        return None;
    }
    let end = text.len() - after_value.len() + bracket + 1;
    Some((name, value, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPERA_GAME: &str = r#"[Event "Paris"]
[Site "Paris FRA"]
[Date "1858.??.??"]
[Round "?"]
[White "Paul Morphy"]
[Black "Duke Karl / Count Isouard"]
[Result "1-0"]
[ECO "C41"]

1.e4 e5 2.Nf3 d6 3.d4 Bg4 {This is a weak move already.} 4.dxe5 Bxf3 5.Qxf3 dxe5
6.Bc4 Nf6 7.Qb3 qe7 $2 8.Nc3 c6 9.Bg5 b5 10.Nxb5 cxb5 11.Bxb5+ Nbd7 12.O-O-O Rd8
13.Rxd7 Rxd7 14.Rd1 Qe6 15.Bxd7+ Nxd7 16.Qb8+ Nxb8 17.Rd8# 1-0
"#;

    #[test]
    fn rejects_illegal_moves_with_their_line() {
        // `qe7` (lowercase) is not valid SAN: the error points at line 11.
        let err = parse_pgn(OPERA_GAME).unwrap_err();
        assert_eq!(err.line, 11);
    }

    #[test]
    fn parses_a_real_game() {
        let text = OPERA_GAME.replace("qe7", "Qe7");
        let games = parse_pgn(&text).unwrap();
        assert_eq!(games.len(), 1);
        let game = &games[0];
        assert_eq!(game.moves().len(), 33);
        assert_eq!(game.result(), GameResult::WhiteWins);
        assert_eq!(game.tag("White"), Some("Paul Morphy"));
        assert_eq!(game.tag("ECO"), Some("C41"));
        assert_eq!(game.tag("Result"), None);
        assert_eq!(
            game.position().status(),
            GameStatus::Checkmate {
                winner: Color::White
            }
        );
    }

    #[test]
    fn export_then_import_round_trips() {
        let text = OPERA_GAME.replace("qe7", "Qe7");
        let game = parse_pgn(&text).unwrap().remove(0);
        let exported = game.to_pgn();
        assert!(exported.starts_with("[Event \"Paris\"]\n[Site \"Paris FRA\"]"));
        assert!(exported.contains("[Result \"1-0\"]\n[ECO \"C41\"]\n\n1. e4 e5 2. Nf3"));
        assert!(exported.lines().all(|line| line.len() <= 80));
        assert!(exported.trim_end().ends_with("17. Rd8# 1-0"));
        let reimported = parse_pgn(&exported).unwrap().remove(0);
        assert_eq!(reimported, game);
    }

    #[test]
    fn skips_comments_variations_and_nags() {
        let text = "% escaped line\n1. e4 {a comment (with parens)} e5 (1... c5 2. Nf3 (2. c3) d6) \
                    2. Nf3 $1 ; rest of line ignored\n Nc6 3.a3?! *";
        let game = parse_pgn(text).unwrap().remove(0);
        let uci: Vec<String> = game.moves().iter().map(ToString::to_string).collect();
        assert_eq!(uci, ["e2e4", "e7e5", "g1f3", "b8c6", "a2a3"]);
        assert_eq!(game.result(), GameResult::Unfinished);
    }

    #[test]
    fn games_from_a_fen_tag() {
        let text =
            "[SetUp \"1\"]\n[FEN \"4k3/8/8/8/8/8/4P3/4K3 b - - 0 40\"]\n\n40... Kd7 41. e4 *";
        let game = parse_pgn(text).unwrap().remove(0);
        assert_eq!(game.moves().len(), 2);
        let exported = game.to_pgn();
        assert!(exported.contains("[FEN \"4k3/8/8/8/8/8/4P3/4K3 b - - 0 40\"]"));
        assert!(exported.contains("40... Kd7 41. e4 *"));
        let reimported = parse_pgn(&exported).unwrap().remove(0);
        assert_eq!(reimported.start(), game.start());
        assert_eq!(reimported.moves(), game.moves());
        assert_eq!(reimported.tag("Event"), Some("?"));
    }

    #[test]
    fn several_games_and_escaped_tags() {
        let text = "[White \"A \\\"quoted\\\" name\"]\n1. d4 1-0\n\n[White \"B\"]\n1. c4 1/2-1/2\n[White \"C\"]";
        let games = parse_pgn(text).unwrap();
        assert_eq!(games.len(), 3);
        assert_eq!(games[0].tag("White"), Some("A \"quoted\" name"));
        assert_eq!(games[1].result(), GameResult::Draw);
        assert!(games[2].moves().is_empty());
        assert!(
            games[0]
                .to_pgn()
                .contains("[White \"A \\\"quoted\\\" name\"]")
        );
    }

    #[test]
    fn syntax_errors() {
        assert!(parse_pgn("1. e4 {never closed").is_err());
        assert!(parse_pgn("1. e4 (1. d4").is_err());
        assert!(parse_pgn("1. e4 )").is_err());
        assert!(parse_pgn("[White \"unterminated]").is_err());
        assert!(parse_pgn("[FEN \"not a fen\"]\n1. e4").is_err());
        assert!(parse_pgn("1. e5").is_err());
    }

    #[test]
    fn game_editing() {
        let mut game = Game::new();
        let pos = game.position().clone();
        game.push(pos.parse_san("e4").unwrap()).unwrap();
        assert!(game.push(Move::NULL).is_err());
        assert_eq!(game.pop().map(|mv| mv.to_string()).as_deref(), Some("e2e4"));
        assert_eq!(game.pop(), None);
        game.set_tag("White", "me");
        game.set_tag("White", "you");
        assert_eq!(game.tags().len(), 1);
        assert_eq!(game.to_pgn().lines().last(), Some("*"));
    }
}
