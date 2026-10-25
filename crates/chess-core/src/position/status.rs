//! End-of-game detection.

use super::Position;
use crate::{Bitboard, Color, PieceKind};

/// Why a game is drawn without stalemate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawReason {
    /// 50 moves by each side without capture or pawn move.
    FiftyMoveRule,
    /// Same position three times with the same side to move.
    ThreefoldRepetition,
    /// Neither side can possibly checkmate.
    InsufficientMaterial,
}

/// State of the game in a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameStatus {
    /// The game goes on.
    Ongoing,
    /// The side to move is checkmated.
    Checkmate {
        /// The side that delivered mate.
        winner: Color,
    },
    /// The side to move has no legal move but is not in check.
    Stalemate,
    /// Drawn by rule.
    Draw(DrawReason),
}

impl GameStatus {
    /// Whether the game is over.
    #[must_use]
    pub const fn is_over(self) -> bool {
        !matches!(self, Self::Ongoing)
    }
}

impl Position {
    /// State of the game. Checkmate takes precedence over the fifty-move rule.
    ///
    /// Repetitions only count positions reached through moves played on
    /// this `Position` (its history), not those before it was parsed.
    #[must_use]
    pub fn status(&self) -> GameStatus {
        if self.legal_moves().is_empty() {
            return if self.in_check() {
                GameStatus::Checkmate {
                    winner: !self.side_to_move,
                }
            } else {
                GameStatus::Stalemate
            };
        }
        if self.is_insufficient_material() {
            GameStatus::Draw(DrawReason::InsufficientMaterial)
        } else if self.halfmove_clock >= 100 {
            GameStatus::Draw(DrawReason::FiftyMoveRule)
        } else if self.repetition_count() >= 3 {
            GameStatus::Draw(DrawReason::ThreefoldRepetition)
        } else {
            GameStatus::Ongoing
        }
    }

    /// How many times the current position has occurred, this one included.
    #[must_use]
    pub fn repetition_count(&self) -> usize {
        let plies = self.history.len();
        // Positions before the last capture or pawn move cannot repeat.
        let reach = plies.min(self.halfmove_clock as usize);
        let mut count = 1;
        // `history[plies - back]` holds the position `back` plies ago; only
        // positions with the same side to move (even distance) can match.
        let mut back = 2;
        while back <= reach {
            if self.history[plies - back].hash == self.hash {
                count += 1;
            }
            back += 2;
        }
        count
    }

    /// Whether the position already occurred (for the search, where a single
    /// repetition is treated as a draw).
    #[must_use]
    pub fn is_repetition(&self) -> bool {
        self.repetition_count() >= 2
    }

    /// Whether no sequence of legal moves can lead to checkmate: K vs K,
    /// K+minor vs K, and kings with bishops all on squares of one color.
    #[must_use]
    pub fn is_insufficient_material(&self) -> bool {
        let heavy_or_pawns = self.pieces_of_kind(PieceKind::Pawn)
            | self.pieces_of_kind(PieceKind::Rook)
            | self.pieces_of_kind(PieceKind::Queen);
        if !heavy_or_pawns.is_empty() {
            return false;
        }
        let knights = self.pieces_of_kind(PieceKind::Knight);
        let bishops = self.pieces_of_kind(PieceKind::Bishop);
        if (knights | bishops).count() <= 1 {
            return true;
        }
        knights.is_empty()
            && ((bishops & Bitboard::LIGHT_SQUARES).is_empty()
                || (bishops & Bitboard::DARK_SQUARES).is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Move, MoveKind, Square};

    fn status(fen: &str) -> GameStatus {
        Position::from_fen(fen).unwrap().status()
    }

    #[test]
    fn checkmate_and_stalemate() {
        assert_eq!(
            status("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3"),
            GameStatus::Checkmate {
                winner: Color::Black
            }
        );
        assert_eq!(
            status("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1"),
            GameStatus::Stalemate
        );
        assert_eq!(Position::startpos().status(), GameStatus::Ongoing);
    }

    #[test]
    fn fifty_move_rule_yields_to_checkmate() {
        assert_eq!(
            status("4k3/8/8/8/8/8/8/R3K3 w - - 100 80"),
            GameStatus::Draw(DrawReason::FiftyMoveRule)
        );
        assert_eq!(
            status("R3k3/8/4K3/8/8/8/8/8 b - - 100 80"),
            GameStatus::Checkmate {
                winner: Color::White
            }
        );
    }

    #[test]
    fn insufficient_material() {
        let draws = [
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
            "4k3/8/8/8/8/8/8/4KN2 w - - 0 1",
            "4k3/8/8/8/8/8/8/4KB2 w - - 0 1",
            // Bishops on the same color (f1 and c8 are both light).
            "2b1k3/8/8/8/8/8/8/4KB2 w - - 0 1",
        ];
        for fen in draws {
            assert_eq!(
                status(fen),
                GameStatus::Draw(DrawReason::InsufficientMaterial),
                "{fen}"
            );
        }
        let playable = [
            "4k3/8/8/8/8/8/8/3NKN2 w - - 0 1",
            "1b2k3/8/8/8/8/8/8/4KB2 w - - 0 1",
            "4k3/8/8/8/8/8/8/4KBN1 w - - 0 1",
            "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1",
        ];
        for fen in playable {
            assert_eq!(status(fen), GameStatus::Ongoing, "{fen}");
        }
    }

    #[test]
    fn threefold_repetition_by_knight_shuffles() {
        let mut pos = Position::startpos();
        let shuffle = [
            Move::new(Square::G1, Square::F3, MoveKind::Quiet),
            Move::new(Square::G8, Square::F6, MoveKind::Quiet),
            Move::new(Square::F3, Square::G1, MoveKind::Quiet),
            Move::new(Square::F6, Square::G8, MoveKind::Quiet),
        ];
        assert_eq!(pos.repetition_count(), 1);
        for mv in shuffle {
            pos.make_move(mv);
        }
        assert_eq!(pos.repetition_count(), 2);
        assert!(pos.is_repetition());
        assert_eq!(pos.status(), GameStatus::Ongoing);
        for mv in shuffle {
            pos.make_move(mv);
        }
        assert_eq!(pos.repetition_count(), 3);
        assert_eq!(
            pos.status(),
            GameStatus::Draw(DrawReason::ThreefoldRepetition)
        );
        pos.unmake_move();
        assert_eq!(pos.status(), GameStatus::Ongoing);
    }
}
