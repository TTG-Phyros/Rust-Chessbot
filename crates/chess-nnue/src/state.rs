//! Incremental evaluation: accumulators along the line being searched,
//! updated with the features each move adds and removes. A king move that
//! changes its perspective's bucket or mirroring rebuilds that perspective.

use std::cell::RefCell;

use chess_core::{Color, Move, MoveKind, Piece, PieceKind, Position, Square};

use crate::features::pieces;
use crate::{Network, Score, simd};

/// Accumulators along the line being searched, root first, one slot of
/// `perspectives × accumulator` values per ply. Slots past `ply` belong to
/// lines already left: they are overwritten, never reallocated.
#[derive(Debug, Clone)]
pub struct NnueState {
    stack: Vec<i16>,
    /// Values per ply.
    stride: usize,
    ply: usize,
    /// Activations of the generic output path.
    scratch: RefCell<Vec<i16>>,
}

impl NnueState {
    fn top(&self) -> Option<&[i16]> {
        self.stack
            .get(self.ply * self.stride..(self.ply + 1) * self.stride)
    }

    /// Current depth in the line (0 at the root).
    #[must_use]
    pub fn ply(&self) -> usize {
        self.ply
    }
}

/// Pieces that leave and enter squares with a move: at most two of each
/// (a capture, a castling rook).
struct Delta {
    removed: [(Piece, Square); 2],
    n_removed: usize,
    added: [(Piece, Square); 2],
    n_added: usize,
}

impl Delta {
    fn removed(&self) -> &[(Piece, Square)] {
        &self.removed[..self.n_removed]
    }

    fn added(&self) -> &[(Piece, Square)] {
        &self.added[..self.n_added]
    }

    fn remove(&mut self, piece: Piece, square: Square) {
        self.removed[self.n_removed] = (piece, square);
        self.n_removed += 1;
    }

    fn add(&mut self, piece: Piece, square: Square) {
        self.added[self.n_added] = (piece, square);
        self.n_added += 1;
    }
}

/// Feature changes when `mv` is played in `before`.
fn move_delta(before: &Position, mv: Move) -> Delta {
    let placeholder = (Piece::new(Color::White, PieceKind::Pawn), Square::A1);
    let mut delta = Delta {
        removed: [placeholder; 2],
        n_removed: 0,
        added: [placeholder; 2],
        n_added: 0,
    };
    let (from, to) = (mv.from(), mv.to());
    let Some(piece) = before.piece_at(from) else {
        return delta;
    };

    delta.remove(piece, from);
    if mv.kind() == MoveKind::EnPassant {
        let victim = Square::from_coords(to.file(), from.rank());
        delta.remove(Piece::new(!piece.color, PieceKind::Pawn), victim);
    } else if let Some(captured) = before.piece_at(to) {
        delta.remove(captured, to);
    }
    delta.add(
        Piece::new(piece.color, mv.promotion().unwrap_or(piece.kind)),
        to,
    );

    if mv.is_castle() {
        let (rook_from, rook_to) = match to {
            Square::G1 => (Square::H1, Square::F1),
            Square::C1 => (Square::A1, Square::D1),
            Square::G8 => (Square::H8, Square::F8),
            _ => (Square::A8, Square::D8),
        };
        // Castling captures nothing, so the second slots are free.
        let rook = Piece::new(piece.color, PieceKind::Rook);
        delta.remove(rook, rook_from);
        delta.add(rook, rook_to);
    }
    delta
}

impl Network {
    /// State for a search starting at `pos`.
    #[must_use]
    pub fn new_state(&self, pos: &Position) -> NnueState {
        let stride = self.layout().accumulator * self.encoder().perspectives().len();
        let mut stack = Vec::with_capacity(stride * 128);
        stack.extend(self.accumulators(pos));
        NnueState {
            stack,
            stride,
            ply: 0,
            scratch: RefCell::new(Vec::new()),
        }
    }

    /// `mv` (possibly [`Move::NULL`]) is about to be played in `before`.
    pub fn on_make(&self, state: &mut NnueState, before: &Position, mv: Move) {
        let (ply, stride) = (state.ply, state.stride);
        if state.stack.len() < (ply + 2) * stride {
            state.stack.resize((ply + 2) * stride, 0);
        }
        state.ply = ply + 1;
        // Both the parent and the child slot exist.
        let (line, next) = state.stack.split_at_mut((ply + 1) * stride);
        let (parent, child) = (&line[ply * stride..], &mut next[..stride]);
        if mv.is_null() {
            child.copy_from_slice(parent);
            return;
        }

        let delta = move_delta(before, mv);
        match self.fixed_update() {
            Some(256) => self.update_fixed::<256>(child, parent, &delta),
            Some(512) => self.update_fixed::<512>(child, parent, &delta),
            Some(768) => self.update_fixed::<768>(child, parent, &delta),
            Some(1024) => self.update_fixed::<1024>(child, parent, &delta),
            Some(1536) => self.update_fixed::<1536>(child, parent, &delta),
            Some(2048) => self.update_fixed::<2048>(child, parent, &delta),
            _ => self.update_generic(child, parent, before, mv, &delta),
        }
    }

    /// Accumulator update of networks without king refreshes, two
    /// perspectives and `N` neurons: rows addressed at compile-time size.
    #[inline(never)]
    fn update_fixed<const N: usize>(&self, child: &mut [i16], parent: &[i16], delta: &Delta) {
        let encoder = self.encoder();
        let rows = self.ft_weights().as_chunks::<N>().0;
        let (children, parents) = (child.as_chunks_mut::<N>().0, parent.as_chunks::<N>().0);
        for ((child, parent), &perspective) in children.iter_mut().zip(parents).zip(&Color::ALL) {
            let context = encoder.fixed_context(perspective);
            let select = |pieces: &[(Piece, Square)]| {
                let mut selected = [parent; 2];
                let mut count = 0;
                for &(piece, square) in pieces {
                    let row = encoder
                        .index(context, perspective, piece, square)
                        .and_then(|feature| rows.get(feature));
                    if let (Some(row), Some(slot)) = (row, selected.get_mut(count)) {
                        *slot = row;
                        count += 1;
                    }
                }
                (selected, count)
            };
            let ((added, n_added), (removed, n_removed)) =
                (select(delta.added()), select(delta.removed()));
            simd::update_arrays(child, parent, &added[..n_added], &removed[..n_removed]);
        }
    }

    /// Accumulator update of any network: king moves that change a
    /// perspective's bucket or mirroring rebuild it.
    #[inline(never)]
    fn update_generic(
        &self,
        child: &mut [i16],
        parent: &[i16],
        before: &Position,
        mv: Move,
        delta: &Delta,
    ) {
        let n = self.layout().accumulator;
        let encoder = self.encoder();
        // Without king buckets or mirroring, a perspective's context never changes.
        let refreshes = self.layout().king_refreshes;
        let moved = if refreshes {
            before.piece_at(mv.from())
        } else {
            None
        };
        for (i, &perspective) in encoder.perspectives().iter().enumerate() {
            let slot = i * n..(i + 1) * n;
            let (Some(child), Some(parent)) = (child.get_mut(slot.clone()), parent.get(slot))
            else {
                continue;
            };
            let context = if refreshes {
                let old_context = encoder.context_in(before, perspective);
                let king = match moved {
                    Some(piece) if piece == Piece::new(perspective, PieceKind::King) => mv.to(),
                    _ => before.king_square(perspective),
                };
                let context = encoder.context(perspective, king);
                if context != old_context {
                    // Rebuild from the pieces after the move.
                    let after = pieces(before)
                        .filter(|placed| !delta.removed().contains(placed))
                        .chain(delta.added().iter().copied());
                    self.refresh(child, perspective, context, after);
                    continue;
                }
                context
            } else {
                encoder.fixed_context(perspective)
            };
            let rows = |pieces: &[(Piece, Square)]| {
                let mut rows: [&[i16]; 2] = [&[]; 2];
                let mut count = 0;
                for &(piece, square) in pieces {
                    if let Some(feature) = encoder.index(context, perspective, piece, square) {
                        rows[count] = self.row(feature);
                        count += 1;
                    }
                }
                (rows, count)
            };
            let ((added, n_added), (removed, n_removed)) =
                (rows(delta.added()), rows(delta.removed()));
            simd::update(child, parent, &added[..n_added], &removed[..n_removed]);
        }
    }

    /// The last move passed to [`Network::on_make`] was taken back.
    pub fn on_unmake(&self, state: &mut NnueState) {
        // The root accumulator is never left by a balanced search.
        state.ply = state.ply.saturating_sub(1);
    }

    /// Same result as [`Network::evaluate`], from the incremental state.
    #[must_use]
    pub fn evaluate_incremental(&self, state: &NnueState, pos: &Position) -> Score {
        match state.top() {
            Some(acc) => self.output(acc, pos, &state.scratch),
            None => self.evaluate(pos),
        }
    }
}
