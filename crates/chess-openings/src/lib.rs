//! Opening names from [lichess-org/chess-openings] (CC0), recognised by
//! **position** rather than by move order, so that transpositions are named
//! like the line they reach: `1. Nf3 d5 2. Bf4 Nf6 3. d4` is a London System.
//!
//! Each line of the base names the position it ends in. Positions are
//! compared by their EPD (the FEN without its move counters). A game gets
//! the name of its deepest named position; its moves are *book* (theory) as
//! long as each one reaches a position of some line of the base.
//!
//! The base is compiled in ([`Book::lichess`]); [`Book::from_tsv`] reads
//! another one in the same format.
//!
//! ```
//! use chess_core::Position;
//! use chess_openings::Book;
//!
//! let mut position = Position::startpos();
//! let mut moves = Vec::new();
//! for san in ["d4", "Nf6", "c4", "e6", "g3"] {
//!     let mv = position.parse_san(san)?;
//!     position.make_move(mv);
//!     moves.push(mv);
//! }
//! let found = Book::lichess().classify(&Position::startpos(), &moves);
//! assert_eq!(found.opening.map(|o| o.name.as_str()), Some("Catalan Opening"));
//! assert_eq!(found.book_plies, 5);
//! # Ok::<(), chess_core::MoveParseError>(())
//! ```
//!
//! [lichess-org/chess-openings]: https://github.com/lichess-org/chess-openings

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use chess_core::{Move, Position};
use thiserror::Error;

/// The base compiled in: `data/*.tsv` (see `data/README.md`).
const LICHESS: [(&str, &str); 5] = [
    ("a.tsv", include_str!("../data/a.tsv")),
    ("b.tsv", include_str!("../data/b.tsv")),
    ("c.tsv", include_str!("../data/c.tsv")),
    ("d.tsv", include_str!("../data/d.tsv")),
    ("e.tsv", include_str!("../data/e.tsv")),
];

/// A named opening line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    /// ECO code (`D02`).
    pub eco: String,
    /// Full name: family, then variations (`Queen's Pawn Game: London System`).
    pub name: String,
    /// The moves of the line, as in the base (`1. d4 d5 2. Nf3 Nf6 3. Bf4`).
    pub moves: String,
    /// Number of moves (plies) of the line.
    pub plies: usize,
}

impl Opening {
    /// The family: the name before its first `:` (`Queen's Pawn Game`).
    #[must_use]
    pub fn family(&self) -> &str {
        self.name
            .split_once(':')
            .map_or(self.name.as_str(), |(family, _)| family)
            .trim()
    }

    /// The variation: the name after its first `:`, if any (`London System`).
    #[must_use]
    pub fn variation(&self) -> Option<&str> {
        self.name
            .split_once(':')
            .map(|(_, variation)| variation.trim())
    }
}

/// A line of the base could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{file} line {line}: {message}")]
pub struct BookError {
    /// File (or source name) of the line.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// What is wrong.
    pub message: String,
}

/// What the base says about a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classification<'a> {
    /// Name of the deepest named position the game reached.
    pub opening: Option<&'a Opening>,
    /// Number of moves played when it reached that position.
    pub opening_ply: usize,
    /// Number of first moves that are book: each one reaches a position of
    /// the base (the theory ends at the first that does not).
    pub book_plies: usize,
}

/// Opening names indexed by position.
#[derive(Debug, Default)]
pub struct Book {
    openings: Vec<Opening>,
    /// EPD of the position a line ends in → its opening (the shortest line
    /// when several end in the same position).
    named: HashMap<String, usize>,
    /// EPD of every position along every line.
    theory: HashSet<String>,
}

static LICHESS_BOOK: LazyLock<Book> = LazyLock::new(|| {
    // The data is checked by the tests: an error cannot happen in a build.
    Book::from_tsv(&LICHESS).unwrap_or_default()
});

impl Book {
    /// The lichess base (about 3,600 lines), indexed on first use.
    #[must_use]
    pub fn lichess() -> &'static Self {
        &LICHESS_BOOK
    }

    /// Reads bases in the lichess TSV format (`eco`, `name`, `pgn` columns
    /// with a header line), given as `(file name, contents)`.
    ///
    /// # Errors
    ///
    /// A line without three columns, or whose moves are not legal SAN from
    /// the starting position.
    pub fn from_tsv(sources: &[(&str, &str)]) -> Result<Self, BookError> {
        let mut book = Self::default();
        for (file, text) in sources {
            for (index, line) in text.lines().enumerate().skip(1) {
                if line.trim().is_empty() {
                    continue;
                }
                let error = |message: String| BookError {
                    file: (*file).to_owned(),
                    line: index + 1,
                    message,
                };
                let mut columns = line.split('\t');
                let (Some(eco), Some(name), Some(moves)) =
                    (columns.next(), columns.next(), columns.next())
                else {
                    return Err(error("expected eco, name and pgn columns".into()));
                };
                book.add(eco.trim(), name.trim(), moves.trim())
                    .map_err(error)?;
            }
        }
        Ok(book)
    }

    fn add(&mut self, eco: &str, name: &str, moves: &str) -> Result<(), String> {
        let mut position = Position::startpos();
        let mut plies = 0;
        // Move numbers (`1.`, `12...`) are skipped; everything else is SAN.
        for token in moves.split_whitespace() {
            let san = token.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
            if san.is_empty() {
                continue;
            }
            let mv = position
                .parse_san(san)
                .map_err(|err| format!("{san}: {err}"))?;
            position.make_move(mv);
            plies += 1;
            self.theory.insert(position.epd());
        }
        if plies == 0 {
            return Err("no moves".into());
        }
        let index = self.openings.len();
        self.openings.push(Opening {
            eco: eco.to_owned(),
            name: name.to_owned(),
            moves: moves.to_owned(),
            plies,
        });
        let openings = &self.openings;
        self.named
            .entry(position.epd())
            .and_modify(|current| {
                if openings[index].plies < openings[*current].plies {
                    *current = index;
                }
            })
            .or_insert(index);
        Ok(())
    }

    /// Number of lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.openings.len()
    }

    /// Whether the base is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.openings.is_empty()
    }

    /// The opening named after `position`, if a line ends there.
    #[must_use]
    pub fn lookup(&self, position: &Position) -> Option<&Opening> {
        self.named
            .get(&position.epd())
            .map(|&index| &self.openings[index])
    }

    /// Whether `position` is on some line of the base.
    #[must_use]
    pub fn is_theory(&self, position: &Position) -> bool {
        self.theory.contains(&position.epd())
    }

    /// The opening of the game made of `moves` from `start`, and how many
    /// of its first moves are book. Illegal moves end the classification.
    #[must_use]
    pub fn classify(&self, start: &Position, moves: &[Move]) -> Classification<'_> {
        let mut position = start.without_history();
        let mut found = Classification {
            opening: None,
            opening_ply: 0,
            book_plies: 0,
        };
        let mut in_book = true;
        for (index, &mv) in moves.iter().enumerate() {
            if position.play(mv).is_err() {
                break;
            }
            let ply = index + 1;
            in_book = in_book && self.is_theory(&position);
            if in_book {
                found.book_plies = ply;
            }
            if let Some(opening) = self.lookup(&position) {
                found.opening = Some(opening);
                found.opening_ply = ply;
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The moves of `sans` from the starting position.
    fn moves(sans: &[&str]) -> Vec<Move> {
        let mut position = Position::startpos();
        sans.iter()
            .map(|san| {
                let mv = position.parse_san(san).unwrap();
                position.make_move(mv);
                mv
            })
            .collect()
    }

    fn classify(sans: &[&str]) -> Classification<'static> {
        Book::lichess().classify(&Position::startpos(), &moves(sans))
    }

    fn name<'a>(found: &Classification<'a>) -> (&'a str, &'a str) {
        let opening = found.opening.unwrap();
        (opening.eco.as_str(), opening.name.as_str())
    }

    #[test]
    fn the_whole_base_is_read() {
        let book = Book::from_tsv(&LICHESS).unwrap();
        let lines: usize = LICHESS
            .iter()
            .map(|(_, text)| text.lines().count() - 1)
            .sum();
        assert_eq!(book.len(), lines);
        assert!(book.len() > 3_000);
        assert_eq!(Book::lichess().len(), book.len());
    }

    #[test]
    fn names_by_move_order() {
        assert_eq!(
            name(&classify(&["d4", "d5", "Nf3", "Nf6", "Bf4"])),
            ("D02", "Queen's Pawn Game: London System")
        );
        assert_eq!(
            name(&classify(&["d4", "d5", "Nf3", "Nf6", "e3"])),
            ("D04", "Queen's Pawn Game: Colle System")
        );
        assert_eq!(
            name(&classify(&["d4", "Nf6", "c4", "e6", "g3"])),
            ("E00", "Catalan Opening")
        );
    }

    #[test]
    fn a_transposition_to_the_london_system() {
        // The London lines of the base start 1. d4 d5 2. Nf3; this game goes
        // 1. Nf3 Nf6 2. d4 d5 3. Bf4, and still reaches their position.
        let found = classify(&["Nf3", "Nf6", "d4", "d5", "Bf4"]);
        assert_eq!(name(&found), ("D02", "Queen's Pawn Game: London System"));
        assert_eq!(found.opening_ply, 5);
        let direct = classify(&["d4", "d5", "Nf3", "Nf6", "Bf4"]);
        assert_eq!(found.opening, direct.opening);
    }

    #[test]
    fn a_transposition_to_the_catalan() {
        let found = classify(&["c4", "e6", "g3", "Nf6", "d4"]);
        assert_eq!(name(&found), ("E00", "Catalan Opening"));
    }

    #[test]
    fn the_deepest_name_is_kept() {
        let found = classify(&[
            "e4", "e5", "Nf3", "Nc6", "Bb5", "a6", "Ba4", "Nf6", "O-O", "Be7", "Re1", "b5", "Bb3",
            "d6", "c3", "O-O", "h3",
        ]);
        let (eco, name) = name(&found);
        assert!(eco.starts_with("C9"), "{eco} {name}");
        assert!(name.starts_with("Ruy Lopez"), "{name}");
        assert_eq!(found.opening_ply, 17);
        assert_eq!(found.book_plies, 17);
    }

    #[test]
    fn book_moves_end_at_the_first_unknown_position() {
        // 2... Ke7?? leaves the theory; the name stays the last one reached.
        let found = classify(&["e4", "e5", "Nf3", "Ke7", "Nxe5", "Ke8"]);
        assert_eq!(found.book_plies, 3);
        assert_eq!(found.opening_ply, 3);
        assert_eq!(found.opening.unwrap().name, "King's Knight Opening");
        // An unnamed position on a line still counts as theory.
        let book = Book::lichess();
        let mut position = Position::startpos();
        for mv in moves(&["Nh3", "d5", "g3"]) {
            position.make_move(mv);
        }
        assert!(book.is_theory(&position));
    }

    #[test]
    fn families_and_variations() {
        let found = classify(&["d4", "d5", "Nf3", "Nf6", "Bf4"]);
        let opening = found.opening.unwrap();
        assert_eq!(opening.family(), "Queen's Pawn Game");
        assert_eq!(opening.variation(), Some("London System"));
        assert_eq!(opening.plies, 5);
        let catalan = classify(&["d4", "Nf6", "c4", "e6", "g3"]).opening.unwrap();
        assert_eq!(
            (catalan.family(), catalan.variation()),
            ("Catalan Opening", None)
        );
    }

    #[test]
    fn games_outside_the_base() {
        let found = classify(&[]);
        assert_eq!((found.opening, found.book_plies), (None, 0));
        // From a position of the base set up by FEN.
        let start = Position::from_fen(
            "rnbqkb1r/ppp1pppp/5n2/3p4/3P1B2/5N2/PPP1PPPP/RN1QKB1R b KQkq - 3 3",
        )
        .unwrap();
        assert_eq!(
            Book::lichess().lookup(&start).map(|o| o.eco.as_str()),
            Some("D02")
        );
    }

    #[test]
    fn rejects_malformed_bases() {
        let header = "eco\tname\tpgn\n";
        assert!(Book::from_tsv(&[("x", &format!("{header}A00\tFine\t1. e4\n"))]).is_ok());
        let err = Book::from_tsv(&[("x", &format!("{header}A00\tBad\t1. e5\n"))]).unwrap_err();
        assert_eq!((err.file.as_str(), err.line), ("x", 2));
        assert!(Book::from_tsv(&[("x", &format!("{header}A00 only one column\n"))]).is_err());
        assert!(Book::from_tsv(&[("x", &format!("{header}A00\tEmpty\t\n"))]).is_err());
    }
}
