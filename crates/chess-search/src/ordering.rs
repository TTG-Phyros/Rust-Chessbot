//! Move ordering: alpha-beta prunes most when the best move comes first.
//!
//! Order: transposition-table move, captures by MVV-LVA (most valuable
//! victim, least valuable attacker) and queen promotions, killer moves,
//! then quiet moves by history score; under-promotions last.

use chess_core::{Color, MAX_MOVES, Move, MoveKind, MoveList, PieceKind, Position};

use crate::score::MAX_PLY;

/// History scores stay within `±MAX_HISTORY`.
pub(crate) const MAX_HISTORY: i32 = 16_384;

/// Rough piece values for MVV-LVA (index = `PieceKind::index`).
const ORDER_VALUE: [i32; 6] = [100, 320, 330, 500, 900, 0];

const TT_MOVE: i32 = 4_000_000;
const GOOD_NOISY: i32 = 2_000_000;
const KILLER_1: i32 = 1_000_000;
const KILLER_2: i32 = 900_000;
const UNDER_PROMOTION: i32 = -1_000_000;

/// Quiet moves that caused cutoffs, by search ply.
pub(crate) struct Killers([[Move; 2]; MAX_PLY + 2]);

impl Killers {
    pub(crate) fn new() -> Self {
        Self([[Move::NULL; 2]; MAX_PLY + 2])
    }

    pub(crate) fn get(&self, ply: usize) -> [Move; 2] {
        self.0.get(ply).copied().unwrap_or_default()
    }

    pub(crate) fn add(&mut self, ply: usize, mv: Move) {
        if let Some(slot) = self.0.get_mut(ply)
            && slot[0] != mv
        {
            slot[1] = slot[0];
            slot[0] = mv;
        }
    }
}

/// Butterfly history: how often a quiet move (by side, from, to) caused a
/// cutoff, with exponential decay ("gravity") so the scores stay bounded.
pub(crate) struct History(Box<[[[i32; 64]; 64]; 2]>);

impl History {
    pub(crate) fn new() -> Self {
        Self(Box::new([[[0; 64]; 64]; 2]))
    }

    pub(crate) fn clear(&mut self) {
        *self.0 = [[[0; 64]; 64]; 2];
    }

    /// Halves every score, so that old searches weigh less than new ones.
    pub(crate) fn age(&mut self) {
        self.0
            .iter_mut()
            .flatten()
            .flatten()
            .for_each(|score| *score /= 2);
    }

    pub(crate) fn get(&self, side: Color, mv: Move) -> i32 {
        self.0[side.index()][mv.from().index()][mv.to().index()]
    }

    /// Rewards (`bonus > 0`) or penalizes (`bonus < 0`) a quiet move.
    pub(crate) fn update(&mut self, side: Color, mv: Move, bonus: i32) {
        let bonus = bonus.clamp(-MAX_HISTORY, MAX_HISTORY);
        let entry = &mut self.0[side.index()][mv.from().index()][mv.to().index()];
        *entry += bonus - *entry * bonus.abs() / MAX_HISTORY;
    }
}

/// Whether `mv` is a capture or a promotion.
pub(crate) const fn is_noisy(mv: Move) -> bool {
    mv.is_capture() || mv.promotion().is_some()
}

/// MVV-LVA score of a capture, plus the promotion gain.
fn noisy_score(pos: &Position, mv: Move) -> i32 {
    let victim = match mv.kind() {
        MoveKind::EnPassant => ORDER_VALUE[PieceKind::Pawn.index()],
        _ => pos
            .piece_at(mv.to())
            .map_or(0, |piece| ORDER_VALUE[piece.kind.index()]),
    };
    let attacker = pos
        .piece_at(mv.from())
        .map_or(0, |piece| ORDER_VALUE[piece.kind.index()]);
    let promotion = mv.promotion().map_or(0, |kind| ORDER_VALUE[kind.index()]);
    victim * 16 + promotion * 16 - attacker / 10
}

/// Yields moves best first (lazy selection sort: cutoffs often happen early).
pub(crate) struct MovePicker {
    moves: MoveList,
    scores: [i32; MAX_MOVES],
    next: usize,
}

impl MovePicker {
    pub(crate) fn new(
        pos: &Position,
        moves: MoveList,
        tt_move: Move,
        killers: [Move; 2],
        history: &History,
    ) -> Self {
        let side = pos.side_to_move();
        let mut scores = [0; MAX_MOVES];
        for (score, &mv) in scores.iter_mut().zip(moves.iter()) {
            *score = if mv == tt_move {
                TT_MOVE
            } else if matches!(mv.promotion(), Some(kind) if kind != PieceKind::Queen) {
                UNDER_PROMOTION + noisy_score(pos, mv)
            } else if is_noisy(mv) {
                GOOD_NOISY + noisy_score(pos, mv)
            } else if mv == killers[0] {
                KILLER_1
            } else if mv == killers[1] {
                KILLER_2
            } else {
                history.get(side, mv)
            };
        }
        Self {
            moves,
            scores,
            next: 0,
        }
    }
}

impl Iterator for MovePicker {
    type Item = Move;

    fn next(&mut self) -> Option<Move> {
        let len = self.moves.len();
        if self.next >= len {
            return None;
        }
        let best = (self.next..len)
            .max_by_key(|&i| self.scores[i])
            .unwrap_or(self.next);
        self.moves.as_mut_slice().swap(self.next, best);
        self.scores.swap(self.next, best);
        let mv = self.moves.get(self.next).copied();
        self.next += 1;
        mv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_priorities() {
        // White can take the queen with the pawn or the rook, or play quietly.
        let pos = Position::from_fen("4k3/8/8/3q4/4P3/8/8/3RK3 w - - 0 1").unwrap();
        let quiet = pos.parse_uci_move("e1f2").unwrap();
        let killer = pos.parse_uci_move("d1c1").unwrap();
        let mut history = History::new();
        history.update(Color::White, quiet, 500);
        let picker = MovePicker::new(
            &pos,
            pos.legal_moves(),
            quiet,
            [killer, Move::NULL],
            &history,
        );
        let order: Vec<String> = picker.map(|mv| mv.to_string()).collect();
        // TT move first, then PxQ before RxQ, then the killer.
        assert_eq!(&order[..4], ["e1f2", "e4d5", "d1d5", "d1c1"]);
    }

    #[test]
    fn under_promotions_come_last() {
        let pos = Position::from_fen("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let picker = MovePicker::new(
            &pos,
            pos.legal_moves(),
            Move::NULL,
            [Move::NULL; 2],
            &History::new(),
        );
        let order: Vec<String> = picker.map(|mv| mv.to_string()).collect();
        assert_eq!(order[0], "b7b8q");
        assert!(
            order[order.len() - 3..]
                .iter()
                .all(|mv| mv.starts_with("b7b8"))
        );
    }

    #[test]
    fn history_stays_bounded() {
        let mut history = History::new();
        let pos = Position::startpos();
        let mv = pos.parse_uci_move("g1f3").unwrap();
        for _ in 0..1000 {
            history.update(Color::White, mv, 4000);
        }
        assert!(history.get(Color::White, mv) <= MAX_HISTORY);
        history.age();
        assert!(history.get(Color::White, mv) <= MAX_HISTORY / 2);
    }
}
