//! Legal move generation.
//!
//! Moves are generated legal directly (no make/test/unmake): the king avoids
//! attacked squares, a check restricts the other pieces to capturing or
//! blocking the checker, pinned pieces stay on their pin line, and en passant
//! (the one move that can uncover a check along a rank) is verified by
//! recomputing the attacks on the king.

use super::{CastlingRights, Position};
use crate::attacks::{
    between, bishop_attacks, king_attacks, knight_attacks, line, pawn_attacks, rook_attacks,
};
use crate::{Bitboard, Color, Move, MoveKind, MoveList, PieceKind, Square};

impl Position {
    /// All legal moves of the side to move.
    #[must_use]
    pub fn legal_moves(&self) -> MoveList {
        let mut moves = MoveList::new();
        self.generate(&mut moves);
        moves
    }

    /// Whether `mv` is legal in this position.
    #[must_use]
    pub fn is_legal(&self, mv: Move) -> bool {
        self.legal_moves().contains(&mv)
    }

    fn generate(&self, moves: &mut MoveList) {
        let us = self.side_to_move;
        let them = !us;
        let ours = self.occupied_by(us);
        let theirs = self.occupied_by(them);
        let occupied = ours | theirs;
        let king = self.king_square(us);

        // King: any square not attacked once the king has left its square
        // (so that it cannot hide "behind itself" from a slider).
        let without_king = occupied.without(king);
        for to in king_attacks(king) & !ours {
            if !self.is_attacked_by(to, them, without_king) {
                push_simple(moves, king, to, theirs);
            }
        }

        let checkers = self.checkers;
        if checkers.more_than_one() {
            return; // Double check: only the king may move.
        }
        // Squares that resolve the check (or anywhere when not in check).
        let target = match checkers.lsb() {
            Some(checker) => between(king, checker) | checkers,
            None => {
                self.castling_moves(moves, king, occupied);
                Bitboard::FULL
            }
        };
        let pinned = self.pinned(us);
        let movable = !ours & target;

        for from in self.pieces(us, PieceKind::Knight) & !pinned {
            push_targets(moves, from, knight_attacks(from) & movable, theirs);
        }
        let queens = self.pieces(us, PieceKind::Queen);
        for from in self.pieces(us, PieceKind::Bishop) | queens {
            let mut targets = bishop_attacks(from, occupied) & movable;
            if pinned.contains(from) {
                targets &= line(king, from);
            }
            push_targets(moves, from, targets, theirs);
        }
        for from in self.pieces(us, PieceKind::Rook) | queens {
            let mut targets = rook_attacks(from, occupied) & movable;
            if pinned.contains(from) {
                targets &= line(king, from);
            }
            push_targets(moves, from, targets, theirs);
        }

        self.pawn_moves(moves, king, pinned, target);
    }

    fn pawn_moves(&self, moves: &mut MoveList, king: Square, pinned: Bitboard, target: Bitboard) {
        let us = self.side_to_move;
        let theirs = self.occupied_by(!us);
        let occupied = self.occupied();
        let (forward, start_rank, last_rank) = match us {
            Color::White => (8, 1, 7),
            Color::Black => (-8, 6, 0),
        };

        for from in self.pieces(us, PieceKind::Pawn) {
            let allowed = if pinned.contains(from) {
                line(king, from) & target
            } else {
                target
            };

            // Pawns never stand on their last rank, so one step forward is on the board.
            let one = from.shifted(forward);
            if !occupied.contains(one) {
                if allowed.contains(one) {
                    push_pawn(moves, from, one, false, last_rank);
                }
                if from.rank().index() == start_rank {
                    let two = one.shifted(forward);
                    if !occupied.contains(two) && allowed.contains(two) {
                        moves.push(Move::new(from, two, MoveKind::DoublePawnPush));
                    }
                }
            }

            for to in pawn_attacks(us, from) & theirs & allowed {
                push_pawn(moves, from, to, true, last_rank);
            }

            if let Some(ep) = self.en_passant
                && pawn_attacks(us, from).contains(ep)
                && self.en_passant_is_legal(from, ep, king)
            {
                moves.push(Move::new(from, ep, MoveKind::EnPassant));
            }
        }
    }

    /// Plays the capture on a scratch occupancy and checks that no enemy
    /// piece attacks the king afterwards. This covers pins, checks by the
    /// captured pawn, and the horizontal discovered check of `8/8/8/KPp4r/...`.
    fn en_passant_is_legal(&self, from: Square, ep: Square, king: Square) -> bool {
        let us = self.side_to_move;
        let them = !us;
        let victim = match us {
            Color::White => ep.shifted(-8),
            Color::Black => ep.shifted(8),
        };
        let occupied = self.occupied().without(from).without(victim).with(ep);
        let queens = self.pieces(them, PieceKind::Queen);
        let attackers = (rook_attacks(king, occupied)
            & (self.pieces(them, PieceKind::Rook) | queens))
            | (bishop_attacks(king, occupied) & (self.pieces(them, PieceKind::Bishop) | queens))
            | (knight_attacks(king) & self.pieces(them, PieceKind::Knight))
            | (pawn_attacks(us, king) & self.pieces(them, PieceKind::Pawn).without(victim));
        attackers.is_empty()
    }

    fn castling_moves(&self, moves: &mut MoveList, king: Square, occupied: Bitboard) {
        let us = self.side_to_move;
        let (kingside, queenside) = match us {
            Color::White => (
                (Square::H1, Square::G1, [Square::F1, Square::G1]),
                (Square::A1, Square::C1, [Square::D1, Square::C1]),
            ),
            Color::Black => (
                (Square::H8, Square::G8, [Square::F8, Square::G8]),
                (Square::A8, Square::C8, [Square::D8, Square::C8]),
            ),
        };
        let sides = [
            (CastlingRights::kingside(us), MoveKind::KingCastle, kingside),
            (
                CastlingRights::queenside(us),
                MoveKind::QueenCastle,
                queenside,
            ),
        ];
        for (right, kind, (rook, king_to, king_path)) in sides {
            // Rights guarantee that the king and rook are on their home squares.
            let free = (between(king, rook) & occupied).is_empty();
            if self.castling.contains(right)
                && free
                && king_path
                    .iter()
                    .all(|&sq| !self.is_attacked_by(sq, !us, occupied))
            {
                moves.push(Move::new(king, king_to, kind));
            }
        }
    }
}

fn push_simple(moves: &mut MoveList, from: Square, to: Square, theirs: Bitboard) {
    let kind = if theirs.contains(to) {
        MoveKind::Capture
    } else {
        MoveKind::Quiet
    };
    moves.push(Move::new(from, to, kind));
}

fn push_targets(moves: &mut MoveList, from: Square, targets: Bitboard, theirs: Bitboard) {
    for to in targets {
        push_simple(moves, from, to, theirs);
    }
}

fn push_pawn(moves: &mut MoveList, from: Square, to: Square, capture: bool, last_rank: u8) {
    if to.rank().index() == last_rank {
        for piece in PieceKind::PROMOTIONS {
            let kind = if capture {
                MoveKind::PromotionCapture(piece)
            } else {
                MoveKind::Promotion(piece)
            };
            moves.push(Move::new(from, to, kind));
        }
    } else {
        let kind = if capture {
            MoveKind::Capture
        } else {
            MoveKind::Quiet
        };
        moves.push(Move::new(from, to, kind));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moves_of(fen: &str) -> Vec<String> {
        let mut moves: Vec<String> = Position::from_fen(fen)
            .unwrap()
            .legal_moves()
            .iter()
            .map(Move::to_string)
            .collect();
        moves.sort();
        moves
    }

    #[test]
    fn startpos_has_twenty_moves() {
        assert_eq!(Position::startpos().legal_moves().len(), 20);
    }

    #[test]
    fn double_check_allows_only_king_moves() {
        // Rook e8 and bishop b4 both check e1; the knight could take e8 but must not.
        let moves = moves_of("4r2k/8/3N4/8/1b6/8/8/4K3 w - - 0 1");
        assert!(moves.iter().all(|m| m.starts_with("e1")), "{moves:?}");
    }

    #[test]
    fn pinned_piece_moves_along_the_pin() {
        // The rook on e4 is pinned by the rook on e8: it can only move on the e file.
        let moves = moves_of("4r2k/8/8/8/4R3/8/8/4K3 w - - 0 1");
        let rook_moves: Vec<_> = moves.iter().filter(|m| m.starts_with("e4")).collect();
        assert_eq!(rook_moves.len(), 6);
        assert!(rook_moves.iter().all(|m| m.as_bytes()[2] == b'e'));
    }

    #[test]
    fn en_passant_discovered_check_is_illegal() {
        // bxc6 would remove both pawns from the 5th rank and expose the king.
        let moves = moves_of("8/8/8/KPp4r/8/8/8/7k w - c6 0 1");
        assert!(!moves.contains(&"b5c6".to_owned()), "{moves:?}");
    }

    #[test]
    fn en_passant_can_capture_the_checker() {
        let moves = moves_of("8/8/8/2k5/3Pp3/8/8/4K3 b - d3 0 1");
        assert!(moves.contains(&"e4d3".to_owned()), "{moves:?}");
    }

    #[test]
    fn castling_through_check_is_illegal() {
        // The bishop on c4 attacks f1: no king-side castling.
        let moves = moves_of("r3k2r/8/8/8/2b5/8/8/R3K2R w KQkq - 0 1");
        assert!(!moves.contains(&"e1g1".to_owned()));
        assert!(moves.contains(&"e1c1".to_owned()));
    }

    #[test]
    fn promotions_generate_four_moves() {
        let moves = moves_of("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1");
        for promo in ["b7b8q", "b7b8r", "b7b8b", "b7b8n"] {
            assert!(moves.contains(&promo.to_owned()));
        }
    }

    #[test]
    fn checkmate_has_no_moves() {
        let fools_mate = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
        assert!(moves_of(fools_mate).is_empty());
    }
}
