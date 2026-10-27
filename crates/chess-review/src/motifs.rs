//! What a position or a mistake is made of: the phase of the game, and the
//! tactical motif behind an error (heuristics over the board and the
//! engine's lines).

use chess_core::attacks::{
    bishop_attacks, king_attacks, knight_attacks, pawn_attacks, queen_attacks, rook_attacks,
};
use chess_core::{Bitboard, Move, PieceKind, Position};
use serde::{Deserialize, Serialize};

use crate::analysis::PositionEval;
use crate::engine::Score;
use crate::exchange;

/// Phase of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// The first moves, pieces still on.
    Opening,
    /// Between the two.
    Middlegame,
    /// Few pieces left.
    Endgame,
}

impl Phase {
    /// Every phase, in game order.
    pub const ALL: [Self; 3] = [Self::Opening, Self::Middlegame, Self::Endgame];
}

/// Where the phases begin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhaseRules {
    /// Last move number of the opening.
    pub opening_moves: u16,
    /// Most knights, bishops, rooks and queens (both sides) of an endgame.
    pub endgame_pieces: u32,
}

impl Default for PhaseRules {
    fn default() -> Self {
        // Lichess counts an endgame from 6 minor and major pieces.
        Self {
            opening_moves: 12,
            endgame_pieces: 6,
        }
    }
}

/// Phase of `position`: by its material first (an endgame), then by its
/// move number.
#[must_use]
pub fn phase(position: &Position, rules: &PhaseRules) -> Phase {
    let pieces = position.occupied()
        & !position.pieces_of_kind(PieceKind::Pawn)
        & !position.pieces_of_kind(PieceKind::King);
    if pieces.count() <= rules.endgame_pieces {
        Phase::Endgame
    } else if position.fullmove_number() <= rules.opening_moves {
        Phase::Opening
    } else {
        Phase::Middlegame
    }
}

/// Tactical motif of a mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Motif {
    /// A mate was on the board and the move let it go.
    MissedMate,
    /// The move lets the opponent mate.
    AllowedMate,
    /// The move leaves material to be taken.
    HangingPiece,
    /// The best move won material at once (a capture).
    MissedMaterial,
    /// The best move was a fork.
    MissedFork,
    /// The move lets the opponent fork.
    AllowedFork,
    /// None of the above: a positional or deeper mistake.
    Other,
}

impl Motif {
    /// Every motif.
    pub const ALL: [Self; 7] = [
        Self::MissedMate,
        Self::AllowedMate,
        Self::HangingPiece,
        Self::MissedMaterial,
        Self::MissedFork,
        Self::AllowedFork,
        Self::Other,
    ];
}

/// The motif of `mv`, a mistake played in `position`; `eval` and `next`
/// are the evaluations of the position and of the next one. `material` is
/// the least material (centipawns) a motif is about.
#[must_use]
pub fn motif(
    position: &Position,
    mv: Move,
    [eval, next]: [&PositionEval; 2],
    material: i32,
) -> Motif {
    let mates = |score: Score| matches!(score, Score::Mate(moves) if moves > 0);
    // Scores of the player: before (best line) and after the move.
    let before = eval.score();
    let after = if next.checkmated {
        Score::Mate(1)
    } else {
        next.score().negate()
    };
    if mates(before) && !mates(after) {
        return Motif::MissedMate;
    }
    if mates(next.score()) && !mates(before.negate()) {
        return Motif::AllowedMate;
    }
    if exchange::sacrifice(position, mv) >= material {
        return Motif::HangingPiece;
    }
    if let Some(best) = parse(position, eval.best_move())
        && best != mv
    {
        if best.is_capture() && exchange::exchange(position, best.from(), best.to()) >= material {
            return Motif::MissedMaterial;
        }
        if is_fork(position, best) {
            return Motif::MissedFork;
        }
    }
    let mut played = position.without_history();
    played.make_move(mv);
    if let Some(reply) = parse(&played, next.best_move())
        && is_fork(&played, reply)
    {
        return Motif::AllowedFork;
    }
    Motif::Other
}

fn parse(position: &Position, uci: Option<&str>) -> Option<Move> {
    position.parse_uci_move(uci?).ok()
}

/// Whether `mv` forks: the piece moved attacks at least two pieces it wins
/// (the king, a more valuable piece, or one left without enough defence),
/// standing where it cannot be taken with profit.
#[must_use]
pub fn is_fork(position: &Position, mv: Move) -> bool {
    let Some(piece) = position.piece_at(mv.from()) else {
        return false;
    };
    if mv.is_castle() {
        return false;
    }
    let kind = mv.promotion().unwrap_or(piece.kind);
    let mut after = position.without_history();
    after.make_move(mv);
    let square = mv.to();
    if exchange::threat(&after, square) > 0 {
        return false;
    }
    let occupied = after.occupied();
    let attacks: Bitboard = match kind {
        PieceKind::Pawn => pawn_attacks(piece.color, square),
        PieceKind::Knight => knight_attacks(square),
        PieceKind::Bishop => bishop_attacks(square, occupied),
        PieceKind::Rook => rook_attacks(square, occupied),
        PieceKind::Queen => queen_attacks(square, occupied),
        PieceKind::King => king_attacks(square),
    };
    let targets = (attacks & after.occupied_by(!piece.color))
        .filter(|&target| {
            after.piece_at(target).is_some_and(|victim| {
                victim.kind == PieceKind::King
                    || exchange::value(victim.kind) > exchange::value(kind)
                    || exchange::threat(&after, target) > 0
            })
        })
        .count();
    targets >= 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Line;

    fn position(fen: &str) -> Position {
        Position::from_fen(fen).unwrap()
    }

    fn eval(lines: &[(&str, Score)]) -> PositionEval {
        PositionEval {
            lines: lines
                .iter()
                .map(|&(uci, score)| Line {
                    score,
                    depth: 10,
                    pv: vec![uci.to_owned()],
                })
                .collect(),
            checkmated: false,
        }
    }

    #[test]
    fn phases_follow_material_then_move_number() {
        let rules = PhaseRules::default();
        assert_eq!(phase(&Position::startpos(), &rules), Phase::Opening);
        let middlegame =
            position("r1bq1rk1/pp2bppp/2n1pn2/3p4/3P4/2NBPN2/PP3PPP/R2QK2R w KQ - 0 13");
        assert_eq!(phase(&middlegame, &rules), Phase::Middlegame);
        // Rook and bishop against rook and knight: 4 pieces.
        let endgame = position("4r1k1/5ppp/8/3n4/8/4B3/5PPP/3R2K1 w - - 0 5");
        assert_eq!(phase(&endgame, &rules), Phase::Endgame);
    }

    #[test]
    fn forks_are_recognised() {
        // Nc7+ attacks the king and the rook on a8.
        let pos = position("r3k3/8/8/3N4/8/8/8/4K3 w - - 0 1");
        assert!(is_fork(&pos, pos.parse_uci_move("d5c7").unwrap()));
        assert!(!is_fork(&pos, pos.parse_uci_move("d5e3").unwrap()));
        // The same fork, but the knight is taken on c7.
        let pos = position("r3k3/1q6/8/3N4/8/8/8/4K3 w - - 0 1");
        assert!(!is_fork(&pos, pos.parse_uci_move("d5c7").unwrap()));
    }

    #[test]
    fn motifs_of_mistakes() {
        let pos = position("r3k3/8/8/3N4/8/8/8/4K3 w - - 0 1");
        // Missing Nc7+.
        let mistake = pos.parse_uci_move("e1e2").unwrap();
        let before = eval(&[("d5c7", Score::Cp(500))]);
        let next = eval(&[("a8a2", Score::Cp(0))]);
        assert_eq!(
            motif(&pos, mistake, [&before, &next], 200),
            Motif::MissedFork
        );

        // Pushing the pawn that defended the knight.
        let pos = position("3rk3/8/8/3N4/4P3/8/8/4K3 w - - 0 1");
        let mistake = pos.parse_uci_move("e4e5").unwrap();
        let before = eval(&[("d5f6", Score::Cp(300))]);
        let next = eval(&[("d8d5", Score::Cp(0))]);
        assert_eq!(
            motif(&pos, mistake, [&before, &next], 200),
            Motif::HangingPiece
        );

        // Not taking a free rook.
        let pos = position("4k3/8/8/3r4/8/8/8/3QK3 w - - 0 1");
        let mistake = pos.parse_uci_move("e1e2").unwrap();
        let before = eval(&[("d1d5", Score::Cp(900))]);
        let next = eval(&[("d5d1", Score::Cp(400))]);
        assert_eq!(
            motif(&pos, mistake, [&before, &next], 200),
            Motif::MissedMaterial
        );

        // Mates, missed and allowed.
        let missed = eval(&[("d1d5", Score::Mate(2))]);
        assert_eq!(
            motif(&pos, mistake, [&missed, &next], 200),
            Motif::MissedMate
        );
        let allowed = eval(&[("d5d2", Score::Mate(3))]);
        assert_eq!(
            motif(&pos, mistake, [&before, &allowed], 200),
            Motif::AllowedMate
        );
    }
}
