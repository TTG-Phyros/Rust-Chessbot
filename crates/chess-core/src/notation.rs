//! Move notations: UCI long algebraic (`e2e4`, `e7e8q`) and Standard
//! Algebraic Notation (`e4`, `Nbd7`, `exd8=Q+`, `O-O`).

use thiserror::Error;

use crate::{Move, MoveKind, PieceKind, Position, Rank, Square};

/// Why a move string could not be turned into a legal move.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MoveParseError {
    /// Not valid UCI or SAN syntax.
    #[error("invalid move syntax: {0:?}")]
    Syntax(String),
    /// Well-formed, but no legal move matches.
    #[error("illegal move: {0:?}")]
    Illegal(String),
    /// Several legal moves match (missing disambiguation in SAN).
    #[error("ambiguous move: {0:?}")]
    Ambiguous(String),
}

impl Position {
    /// Parses a move in UCI notation and returns the matching legal move.
    ///
    /// # Errors
    ///
    /// [`MoveParseError::Syntax`] for malformed input, [`MoveParseError::Illegal`]
    /// if the move is not legal here.
    pub fn parse_uci_move(&self, text: &str) -> Result<Move, MoveParseError> {
        let syntax = || MoveParseError::Syntax(text.to_owned());
        let from: Square = text
            .get(0..2)
            .ok_or_else(syntax)?
            .parse()
            .map_err(|_| syntax())?;
        let to: Square = text
            .get(2..4)
            .ok_or_else(syntax)?
            .parse()
            .map_err(|_| syntax())?;
        let promotion = match text.get(4..) {
            Some("") => None,
            Some(suffix) => {
                let mut chars = suffix.chars();
                match (chars.next().and_then(PieceKind::from_char), chars.next()) {
                    (Some(kind), None) if kind != PieceKind::Pawn && kind != PieceKind::King => {
                        Some(kind)
                    }
                    _ => return Err(syntax()),
                }
            }
            None => return Err(syntax()),
        };
        self.legal_moves()
            .iter()
            .copied()
            .find(|mv| mv.from() == from && mv.to() == to && mv.promotion() == promotion)
            .ok_or_else(|| MoveParseError::Illegal(text.to_owned()))
    }

    /// Writes `mv` (which must be legal here) in Standard Algebraic Notation,
    /// with `+` or `#` when it gives check or mate.
    #[must_use]
    pub fn san(&self, mv: Move) -> String {
        let mut san = match mv.kind() {
            MoveKind::KingCastle => "O-O".to_owned(),
            MoveKind::QueenCastle => "O-O-O".to_owned(),
            _ => self.san_without_suffix(mv),
        };
        let mut after = self.without_history();
        after.make_move(mv);
        if after.in_check() {
            san.push(if after.legal_moves().is_empty() {
                '#'
            } else {
                '+'
            });
        }
        san
    }

    fn san_without_suffix(&self, mv: Move) -> String {
        let (from, to) = (mv.from(), mv.to());
        let kind = self
            .piece_at(from)
            .map_or(PieceKind::Pawn, |piece| piece.kind);
        let mut san = String::with_capacity(8);

        if kind == PieceKind::Pawn {
            if mv.is_capture() {
                san.push(from.file().to_char());
                san.push('x');
            }
            san.push_str(&to.to_string());
            if let Some(promotion) = mv.promotion() {
                san.push('=');
                san.push(promotion.to_char().to_ascii_uppercase());
            }
            return san;
        }

        san.push(kind.to_char().to_ascii_uppercase());
        // Other pieces of the same kind that could also go to `to`.
        let rivals: Vec<Square> = self
            .legal_moves()
            .iter()
            .filter(|other| {
                other.to() == to
                    && other.from() != from
                    && self.piece_at(other.from()).map(|piece| piece.kind) == Some(kind)
            })
            .map(|other| other.from())
            .collect();
        if !rivals.is_empty() {
            let file_unique = rivals.iter().all(|sq| sq.file() != from.file());
            let rank_unique = rivals.iter().all(|sq| sq.rank() != from.rank());
            if file_unique {
                san.push(from.file().to_char());
            } else if rank_unique {
                san.push(from.rank().to_char());
            } else {
                san.push_str(&from.to_string());
            }
        }
        if mv.is_capture() {
            san.push('x');
        }
        san.push_str(&to.to_string());
        san
    }

    /// Parses a move in Standard Algebraic Notation. Tolerates check/mate
    /// markers, annotations (`!?`), `0-0` for castling, and promotions
    /// written without `=` (`e8Q`).
    ///
    /// # Errors
    ///
    /// [`MoveParseError::Syntax`], [`MoveParseError::Illegal`] or
    /// [`MoveParseError::Ambiguous`].
    pub fn parse_san(&self, text: &str) -> Result<Move, MoveParseError> {
        let syntax = || MoveParseError::Syntax(text.to_owned());
        let illegal = || MoveParseError::Illegal(text.to_owned());
        let core = text.trim().trim_end_matches(['+', '#', '!', '?']);

        let castle = match core {
            "O-O" | "0-0" => Some(MoveKind::KingCastle),
            "O-O-O" | "0-0-0" => Some(MoveKind::QueenCastle),
            _ => None,
        };
        if let Some(kind) = castle {
            return self
                .legal_moves()
                .iter()
                .copied()
                .find(|mv| mv.kind() == kind)
                .ok_or_else(illegal);
        }

        let mut chars: Vec<char> = core.chars().collect();

        // Promotion suffix: `=Q` or a trailing piece letter after a rank digit.
        let mut promotion = None;
        if let Some(&last) = chars.last()
            && let Some(kind) = PieceKind::from_char(last).filter(|_| last.is_ascii_uppercase())
            && chars.len() >= 3
        {
            promotion = Some(kind);
            chars.pop();
            if chars.last() == Some(&'=') {
                chars.pop();
            }
        }
        if matches!(promotion, Some(PieceKind::Pawn | PieceKind::King)) {
            return Err(syntax());
        }

        let kind = match chars.first() {
            Some(&c) if c.is_ascii_uppercase() => {
                chars.remove(0);
                PieceKind::from_char(c)
                    .filter(|&k| k != PieceKind::Pawn)
                    .ok_or_else(syntax)?
            }
            _ => PieceKind::Pawn,
        };

        if chars.len() < 2 {
            return Err(syntax());
        }
        let destination: String = chars[chars.len() - 2..].iter().collect();
        let to: Square = destination.parse().map_err(|_| syntax())?;

        // Whatever sits between the piece letter and the destination:
        // optional disambiguation (file, rank or square) and `x` / `-`.
        let mut from_file = None;
        let mut from_rank = None;
        for &c in &chars[..chars.len() - 2] {
            match c {
                'x' | '-' | ':' => {}
                'a'..='h' => from_file = crate::File::from_char(c),
                '1'..='8' => from_rank = Rank::from_char(c),
                _ => return Err(syntax()),
            }
        }

        let candidates: Vec<Move> = self
            .legal_moves()
            .iter()
            .copied()
            .filter(|mv| {
                mv.to() == to
                    && !mv.is_castle()
                    && mv.promotion() == promotion
                    && self.piece_at(mv.from()).map(|piece| piece.kind) == Some(kind)
                    && from_file.is_none_or(|file| mv.from().file() == file)
                    && from_rank.is_none_or(|rank| mv.from().rank() == rank)
            })
            .collect();
        match candidates.as_slice() {
            [mv] => Ok(*mv),
            [] => Err(illegal()),
            _ => Err(MoveParseError::Ambiguous(text.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

    #[test]
    fn uci_round_trip_for_every_legal_move() {
        let pos = Position::from_fen(KIWIPETE).unwrap();
        for &mv in &pos.legal_moves() {
            assert_eq!(pos.parse_uci_move(&mv.to_string()).unwrap(), mv);
        }
    }

    #[test]
    fn uci_errors() {
        let pos = Position::startpos();
        assert!(matches!(
            pos.parse_uci_move("e2e5"),
            Err(MoveParseError::Illegal(_))
        ));
        for bad in ["", "e2", "e2e", "e2e4x", "e2e4qq", "z2e4", "e7e8k"] {
            assert!(
                matches!(pos.parse_uci_move(bad), Err(MoveParseError::Syntax(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn san_round_trip_for_every_legal_move() {
        let fens = [
            KIWIPETE,
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            // Three queens reaching e3: rank and full-square disambiguation.
            "8/7k/8/2Q1Q3/8/2Q5/8/K7 w - - 0 1",
        ];
        for fen in fens {
            let pos = Position::from_fen(fen).unwrap();
            for mv in pos.legal_moves().iter().copied() {
                let san = pos.san(mv);
                assert_eq!(pos.parse_san(&san).unwrap(), mv, "{san} in {fen}");
            }
        }
    }

    #[test]
    fn san_spelling() {
        let pos = Position::from_fen(KIWIPETE).unwrap();
        let san = |uci: &str| pos.san(pos.parse_uci_move(uci).unwrap());
        assert_eq!(san("e1g1"), "O-O");
        assert_eq!(san("e1c1"), "O-O-O");
        assert_eq!(san("d5e6"), "dxe6");
        assert_eq!(san("e5f7"), "Nxf7");
        assert_eq!(san("c3b1"), "Nb1");
        assert_eq!(san("e2a6"), "Bxa6");

        let rooks = Position::from_fen("4k3/8/8/8/8/8/4K3/R6R w - - 0 1").unwrap();
        assert_eq!(rooks.san(rooks.parse_uci_move("a1d1").unwrap()), "Rad1");
        let mate = Position::from_fen("4k3/8/4K3/8/8/8/8/7R w - - 0 1").unwrap();
        assert_eq!(mate.san(mate.parse_uci_move("h1h8").unwrap()), "Rh8#");
        let promo = Position::from_fen("3r3k/4P3/8/8/8/8/8/K7 w - - 0 1").unwrap();
        assert_eq!(promo.san(promo.parse_uci_move("e7d8n").unwrap()), "exd8=N");
        assert_eq!(promo.san(promo.parse_uci_move("e7e8q").unwrap()), "e8=Q+");
    }

    #[test]
    fn san_parsing_is_lenient_but_strict_on_ambiguity() {
        let pos = Position::from_fen(KIWIPETE).unwrap();
        let uci = |san: &str| pos.parse_san(san).map(|mv| mv.to_string());
        assert_eq!(uci("0-0").unwrap(), "e1g1");
        assert_eq!(uci("O-O-O").unwrap(), "e1c1");
        assert_eq!(uci("Nxf7!?").unwrap(), "e5f7");
        assert_eq!(uci("dxe6").unwrap(), "d5e6");
        assert_eq!(uci("Bd2-e3").unwrap(), "d2e3");

        let rooks = Position::from_fen("4k3/8/8/8/8/8/4K3/R6R w - - 0 1").unwrap();
        assert!(matches!(
            rooks.parse_san("Rd1"),
            Err(MoveParseError::Ambiguous(_))
        ));
        assert_eq!(rooks.parse_san("Rhd1").unwrap().to_string(), "h1d1");
        let knights = Position::from_fen("4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1").unwrap();
        assert!(matches!(
            knights.parse_san("Nd2"),
            Err(MoveParseError::Ambiguous(_))
        ));
        assert_eq!(knights.parse_san("Nbd2").unwrap().to_string(), "b1d2");

        let promo = Position::from_fen("3r3k/4P3/8/8/8/8/8/K7 w - - 0 1").unwrap();
        assert_eq!(promo.parse_san("e8Q").unwrap().to_string(), "e7e8q");
        assert_eq!(promo.parse_san("exd8=R").unwrap().to_string(), "e7d8r");
        assert!(matches!(
            promo.parse_san("e8"),
            Err(MoveParseError::Illegal(_))
        ));
        assert!(matches!(
            promo.parse_san("Xe4"),
            Err(MoveParseError::Syntax(_))
        ));
        assert!(matches!(
            promo.parse_san(""),
            Err(MoveParseError::Syntax(_))
        ));
    }
}
