//! Chess positions: piece placement, game state, incremental make/unmake.

mod fen;
mod movegen;
mod status;

use std::fmt;

pub use fen::{FenError, STARTPOS_FEN};
pub use status::{DrawReason, GameStatus};

use crate::attacks::{
    between, bishop_attacks, king_attacks, knight_attacks, pawn_attacks, rook_attacks,
};
use crate::zobrist::KEYS;
use crate::{Bitboard, Color, Move, MoveKind, Piece, PieceKind, Square};

/// Castling rights, as a set of four flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CastlingRights(u8);

impl CastlingRights {
    /// No right at all.
    pub const NONE: Self = Self(0);
    /// White may castle king side.
    pub const WHITE_KINGSIDE: Self = Self(1);
    /// White may castle queen side.
    pub const WHITE_QUEENSIDE: Self = Self(2);
    /// Black may castle king side.
    pub const BLACK_KINGSIDE: Self = Self(4);
    /// Black may castle queen side.
    pub const BLACK_QUEENSIDE: Self = Self(8);
    /// All four rights.
    pub const ALL: Self = Self(15);

    /// King-side right of `color`.
    #[must_use]
    pub const fn kingside(color: Color) -> Self {
        match color {
            Color::White => Self::WHITE_KINGSIDE,
            Color::Black => Self::BLACK_KINGSIDE,
        }
    }

    /// Queen-side right of `color`.
    #[must_use]
    pub const fn queenside(color: Color) -> Self {
        match color {
            Color::White => Self::WHITE_QUEENSIDE,
            Color::Black => Self::BLACK_QUEENSIDE,
        }
    }

    /// Whether every right of `other` is in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// `self` with the rights of `other` added.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `self` with the rights of `other` removed.
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Raw bits (`1` = white king side ... `8` = black queen side).
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

/// Rights kept when a piece moves from or to each square: moving the king or
/// a rook, or capturing a rook on its home square, loses the matching right.
static CASTLING_KEPT: [u8; 64] = {
    let mut kept = [15; 64];
    kept[Square::A1.index()] = 15 & !2;
    kept[Square::E1.index()] = 15 & !3;
    kept[Square::H1.index()] = 15 & !1;
    kept[Square::A8.index()] = 15 & !8;
    kept[Square::E8.index()] = 15 & !12;
    kept[Square::H8.index()] = 15 & !4;
    kept
};

/// State needed to undo a move.
#[derive(Debug, Clone, Copy)]
struct Undo {
    mv: Move,
    captured: Option<Piece>,
    castling: CastlingRights,
    en_passant: Option<Square>,
    halfmove_clock: u16,
    hash: u64,
    checkers: Bitboard,
}

/// A chess position together with the moves that led to it.
///
/// The history (needed by [`Position::unmake_move`] and repetition detection)
/// is not part of equality: two positions are equal when the board, side to
/// move, castling rights, en passant square and clocks are.
#[derive(Clone)]
pub struct Position {
    board: [Option<Piece>; 64],
    by_color: [Bitboard; 2],
    by_kind: [Bitboard; 6],
    side_to_move: Color,
    castling: CastlingRights,
    /// Only set when a pawn can actually capture there, so that hashes and
    /// repetitions ignore meaningless en passant squares.
    en_passant: Option<Square>,
    halfmove_clock: u16,
    fullmove_number: u16,
    hash: u64,
    checkers: Bitboard,
    history: Vec<Undo>,
}

impl Position {
    /// The standard starting position.
    #[must_use]
    pub fn startpos() -> Self {
        const BACK_RANK: [PieceKind; 8] = [
            PieceKind::Rook,
            PieceKind::Knight,
            PieceKind::Bishop,
            PieceKind::Queen,
            PieceKind::King,
            PieceKind::Bishop,
            PieceKind::Knight,
            PieceKind::Rook,
        ];
        let mut pos = Self::empty();
        for (file, &kind) in BACK_RANK.iter().enumerate() {
            let file = file as u8;
            pos.put_piece(Square::from_u6(file), Piece::new(Color::White, kind));
            pos.put_piece(
                Square::from_u6(8 + file),
                Piece::new(Color::White, PieceKind::Pawn),
            );
            pos.put_piece(
                Square::from_u6(48 + file),
                Piece::new(Color::Black, PieceKind::Pawn),
            );
            pos.put_piece(Square::from_u6(56 + file), Piece::new(Color::Black, kind));
        }
        pos.set_castling(CastlingRights::ALL);
        pos
    }

    /// An empty board, white to move. Not a legal position on its own.
    fn empty() -> Self {
        Self {
            board: [None; 64],
            by_color: [Bitboard::EMPTY; 2],
            by_kind: [Bitboard::EMPTY; 6],
            side_to_move: Color::White,
            castling: CastlingRights::NONE,
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            hash: 0,
            checkers: Bitboard::EMPTY,
            history: Vec::new(),
        }
    }

    // ------------------------------------------------------------ accessors

    /// Piece standing on `square`.
    #[must_use]
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        self.board[square.index()]
    }

    /// Squares occupied by pieces of `color` and `kind`.
    #[must_use]
    pub fn pieces(&self, color: Color, kind: PieceKind) -> Bitboard {
        self.by_color[color.index()] & self.by_kind[kind.index()]
    }

    /// Squares occupied by pieces of `kind`, of either color.
    #[must_use]
    pub fn pieces_of_kind(&self, kind: PieceKind) -> Bitboard {
        self.by_kind[kind.index()]
    }

    /// Squares occupied by any piece of `color`.
    #[must_use]
    pub fn occupied_by(&self, color: Color) -> Bitboard {
        self.by_color[color.index()]
    }

    /// Occupied squares.
    #[must_use]
    pub fn occupied(&self) -> Bitboard {
        self.by_color[0] | self.by_color[1]
    }

    /// Side whose turn it is.
    #[must_use]
    pub const fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    /// Remaining castling rights.
    #[must_use]
    pub const fn castling_rights(&self) -> CastlingRights {
        self.castling
    }

    /// En passant target square, only when an en passant capture is possible.
    #[must_use]
    pub const fn en_passant(&self) -> Option<Square> {
        self.en_passant
    }

    /// Plies since the last capture or pawn move (fifty-move rule).
    #[must_use]
    pub const fn halfmove_clock(&self) -> u16 {
        self.halfmove_clock
    }

    /// Move number, starting at 1 and incremented after each black move.
    #[must_use]
    pub const fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }

    /// Zobrist hash of the position (pieces, side, castling, en passant).
    #[must_use]
    pub const fn hash(&self) -> u64 {
        self.hash
    }

    /// Pieces giving check to the side to move.
    #[must_use]
    pub const fn checkers(&self) -> Bitboard {
        self.checkers
    }

    /// Whether the side to move is in check.
    #[must_use]
    pub const fn in_check(&self) -> bool {
        !self.checkers.is_empty()
    }

    /// Square of the king of `color`.
    #[must_use]
    pub fn king_square(&self, color: Color) -> Square {
        // Every position built by this crate has exactly one king per side.
        self.pieces(color, PieceKind::King)
            .lsb()
            .unwrap_or(Square::A1)
    }

    /// Number of moves played since this position was created or parsed.
    #[must_use]
    pub fn ply(&self) -> usize {
        self.history.len()
    }

    /// The last move played, if any.
    #[must_use]
    pub fn last_move(&self) -> Option<Move> {
        self.history.last().map(|undo| undo.mv)
    }

    /// Moves played since this position was created or parsed, in order.
    pub fn moves_played(&self) -> impl ExactSizeIterator<Item = Move> + '_ {
        self.history.iter().map(|undo| undo.mv)
    }

    // -------------------------------------------------------------- attacks

    /// Pieces of both colors attacking `square`, with `occupied` as the board.
    #[must_use]
    pub fn attackers_to(&self, square: Square, occupied: Bitboard) -> Bitboard {
        let rooks = self.by_kind[PieceKind::Rook.index()] | self.by_kind[PieceKind::Queen.index()];
        let bishops =
            self.by_kind[PieceKind::Bishop.index()] | self.by_kind[PieceKind::Queen.index()];
        (pawn_attacks(Color::Black, square) & self.pieces(Color::White, PieceKind::Pawn))
            | (pawn_attacks(Color::White, square) & self.pieces(Color::Black, PieceKind::Pawn))
            | (knight_attacks(square) & self.by_kind[PieceKind::Knight.index()])
            | (king_attacks(square) & self.by_kind[PieceKind::King.index()])
            | (rook_attacks(square, occupied) & rooks)
            | (bishop_attacks(square, occupied) & bishops)
    }

    /// Whether a piece of `by` attacks `square`, with `occupied` as the board.
    #[must_use]
    pub fn is_attacked_by(&self, square: Square, by: Color, occupied: Bitboard) -> bool {
        let queens = self.pieces(by, PieceKind::Queen);
        !(pawn_attacks(!by, square) & self.pieces(by, PieceKind::Pawn)).is_empty()
            || !(knight_attacks(square) & self.pieces(by, PieceKind::Knight)).is_empty()
            || !(king_attacks(square) & self.pieces(by, PieceKind::King)).is_empty()
            || !(rook_attacks(square, occupied) & (self.pieces(by, PieceKind::Rook) | queens))
                .is_empty()
            || !(bishop_attacks(square, occupied) & (self.pieces(by, PieceKind::Bishop) | queens))
                .is_empty()
    }

    /// Pieces of `color` pinned against their own king.
    #[must_use]
    pub fn pinned(&self, color: Color) -> Bitboard {
        let king = self.king_square(color);
        let them = !color;
        let queens = self.pieces(them, PieceKind::Queen);
        let snipers = (rook_attacks(king, Bitboard::EMPTY)
            & (self.pieces(them, PieceKind::Rook) | queens))
            | (bishop_attacks(king, Bitboard::EMPTY)
                & (self.pieces(them, PieceKind::Bishop) | queens));
        let occupied = self.occupied();
        let mut pinned = Bitboard::EMPTY;
        for sniper in snipers {
            let blockers = between(king, sniper) & occupied;
            if blockers.count() == 1 {
                pinned |= blockers & self.occupied_by(color);
            }
        }
        pinned
    }

    fn compute_checkers(&self) -> Bitboard {
        let us = self.side_to_move;
        self.attackers_to(self.king_square(us), self.occupied()) & self.occupied_by(!us)
    }

    // ------------------------------------------------------- board editing

    fn put_piece(&mut self, square: Square, piece: Piece) {
        let bb = Bitboard::from_square(square);
        self.board[square.index()] = Some(piece);
        self.by_color[piece.color.index()] |= bb;
        self.by_kind[piece.kind.index()] |= bb;
        self.hash ^= KEYS.piece(piece, square);
    }

    fn remove_piece(&mut self, square: Square) -> Option<Piece> {
        let piece = self.board[square.index()].take()?;
        let bb = !Bitboard::from_square(square);
        self.by_color[piece.color.index()] &= bb;
        self.by_kind[piece.kind.index()] &= bb;
        self.hash ^= KEYS.piece(piece, square);
        Some(piece)
    }

    fn move_piece(&mut self, from: Square, to: Square) {
        if let Some(piece) = self.remove_piece(from) {
            self.put_piece(to, piece);
        }
    }

    fn set_castling(&mut self, rights: CastlingRights) {
        self.hash ^= KEYS.castling(self.castling.bits()) ^ KEYS.castling(rights.bits());
        self.castling = rights;
    }

    fn set_en_passant(&mut self, square: Option<Square>) {
        if let Some(old) = self.en_passant {
            self.hash ^= KEYS.en_passant(old);
        }
        if let Some(new) = square {
            self.hash ^= KEYS.en_passant(new);
        }
        self.en_passant = square;
    }

    /// Keeps `square` as en passant target only if a pawn of the side to move
    /// attacks it.
    fn en_passant_if_capturable(&self, square: Square) -> Option<Square> {
        let us = self.side_to_move;
        let capturers = pawn_attacks(!us, square) & self.pieces(us, PieceKind::Pawn);
        (!capturers.is_empty()).then_some(square)
    }

    // ------------------------------------------------------ make / unmake

    /// Plays `mv`, which must be legal in this position (as returned by
    /// [`Position::legal_moves`] or the move parsers); otherwise the position
    /// is left in an inconsistent state. Use [`Position::play`] for untrusted moves.
    pub fn make_move(&mut self, mv: Move) {
        let us = self.side_to_move;
        let (from, to) = (mv.from(), mv.to());
        debug_assert!(self.piece_at(from).is_some(), "no piece on {from} for {mv}");
        let Some(piece) = self.piece_at(from) else {
            return;
        };

        let mut undo = Undo {
            mv,
            captured: None,
            castling: self.castling,
            en_passant: self.en_passant,
            halfmove_clock: self.halfmove_clock,
            hash: self.hash,
            checkers: self.checkers,
        };
        self.set_en_passant(None);
        self.halfmove_clock = self.halfmove_clock.saturating_add(1);

        match mv.kind() {
            MoveKind::Capture | MoveKind::PromotionCapture(_) => {
                undo.captured = self.remove_piece(to);
            }
            MoveKind::EnPassant => {
                undo.captured = self.remove_piece(en_passant_victim(to, us));
            }
            _ => {}
        }
        if undo.captured.is_some() || piece.kind == PieceKind::Pawn {
            self.halfmove_clock = 0;
        }

        self.move_piece(from, to);
        match mv.kind() {
            MoveKind::Promotion(kind) | MoveKind::PromotionCapture(kind) => {
                self.remove_piece(to);
                self.put_piece(to, Piece::new(us, kind));
            }
            MoveKind::KingCastle | MoveKind::QueenCastle => {
                let (rook_from, rook_to) = castling_rook(to);
                self.move_piece(rook_from, rook_to);
            }
            _ => {}
        }

        let kept = CASTLING_KEPT[from.index()] & CASTLING_KEPT[to.index()];
        self.set_castling(CastlingRights(self.castling.bits() & kept));

        if us == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }
        self.side_to_move = !us;
        self.hash ^= KEYS.side;

        if mv.kind() == MoveKind::DoublePawnPush {
            let target = Square::from_u6(((from.index() + to.index()) / 2) as u8);
            self.set_en_passant(self.en_passant_if_capturable(target));
        }

        self.history.push(undo);
        self.checkers = self.compute_checkers();
    }

    /// Takes back the last move played with [`Position::make_move`]. Does
    /// nothing if no move was played.
    pub fn unmake_move(&mut self) {
        let Some(undo) = self.history.pop() else {
            return;
        };
        let mv = undo.mv;
        let (from, to) = (mv.from(), mv.to());
        let us = !self.side_to_move;
        self.side_to_move = us;
        if us == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_sub(1);
        }

        match mv.kind() {
            MoveKind::Promotion(_) | MoveKind::PromotionCapture(_) => {
                self.remove_piece(to);
                self.put_piece(to, Piece::new(us, PieceKind::Pawn));
            }
            MoveKind::KingCastle | MoveKind::QueenCastle => {
                let (rook_from, rook_to) = castling_rook(to);
                self.move_piece(rook_to, rook_from);
            }
            _ => {}
        }
        self.move_piece(to, from);
        if let Some(captured) = undo.captured {
            let square = if mv.kind() == MoveKind::EnPassant {
                en_passant_victim(to, us)
            } else {
                to
            };
            self.put_piece(square, captured);
        }

        self.castling = undo.castling;
        self.en_passant = undo.en_passant;
        self.halfmove_clock = undo.halfmove_clock;
        self.hash = undo.hash;
        self.checkers = undo.checkers;
    }

    /// Plays `mv` after checking that it is legal.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::IllegalMove`] if `mv` is not legal here.
    pub fn play(&mut self, mv: Move) -> Result<(), crate::CoreError> {
        if !self.is_legal(mv) {
            return Err(crate::CoreError::IllegalMove(mv.to_string()));
        }
        self.make_move(mv);
        Ok(())
    }

    /// A copy of the position without its move history (cheaper to clone,
    /// but cannot unmake past moves nor detect earlier repetitions).
    #[must_use]
    pub fn without_history(&self) -> Self {
        Self {
            board: self.board,
            by_color: self.by_color,
            by_kind: self.by_kind,
            side_to_move: self.side_to_move,
            castling: self.castling,
            en_passant: self.en_passant,
            halfmove_clock: self.halfmove_clock,
            fullmove_number: self.fullmove_number,
            hash: self.hash,
            checkers: self.checkers,
            history: Vec::new(),
        }
    }
}

/// Square of the pawn captured en passant by `us` landing on `target`.
fn en_passant_victim(target: Square, us: Color) -> Square {
    match us {
        Color::White => target.shifted(-8),
        Color::Black => target.shifted(8),
    }
}

/// Rook origin and destination for a castling move whose king lands on `king_to`.
fn castling_rook(king_to: Square) -> (Square, Square) {
    match king_to {
        Square::G1 => (Square::H1, Square::F1),
        Square::C1 => (Square::A1, Square::D1),
        Square::G8 => (Square::H8, Square::F8),
        _ => (Square::A8, Square::D8),
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::startpos()
    }
}

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        self.board == other.board
            && self.side_to_move == other.side_to_move
            && self.castling == other.castling
            && self.en_passant == other.en_passant
            && self.halfmove_clock == other.halfmove_clock
            && self.fullmove_number == other.fullmove_number
    }
}

impl Eq for Position {}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Position({})", self.to_fen())
    }
}

/// Draws the board (rank 8 on top) followed by its FEN.
impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..8).rev() {
            write!(f, "{} ", rank + 1)?;
            for file in 0..8 {
                let c = self.board[rank * 8 + file].map_or('.', Piece::to_fen_char);
                write!(f, " {c}")?;
            }
            writeln!(f)?;
        }
        writeln!(f, "\n   a b c d e f g h\n")?;
        write!(f, "{}", self.to_fen())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_piece_counts() {
        let pos = Position::startpos();
        for color in Color::ALL {
            assert_eq!(pos.occupied_by(color).count(), 16);
            assert_eq!(pos.pieces(color, PieceKind::Pawn).count(), 8);
            assert_eq!(pos.pieces(color, PieceKind::King).count(), 1);
        }
        assert_eq!(pos.king_square(Color::White), Square::E1);
        assert_eq!(pos.king_square(Color::Black), Square::E8);
        assert_eq!(pos.side_to_move(), Color::White);
        assert!(!pos.in_check());
    }

    #[test]
    fn startpos_matches_its_fen() {
        let parsed = Position::from_fen(STARTPOS_FEN).unwrap();
        assert_eq!(parsed, Position::startpos());
        assert_eq!(parsed.hash(), Position::startpos().hash());
    }

    #[test]
    fn make_unmake_restores_everything() {
        let fens = [
            STARTPOS_FEN,
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        ];
        for fen in fens {
            let mut pos = Position::from_fen(fen).unwrap();
            let original = pos.clone();
            for mv in pos.legal_moves().iter().copied() {
                pos.make_move(mv);
                // The incremental hash must match a hash computed from scratch.
                let reparsed = Position::from_fen(&pos.to_fen()).unwrap();
                assert_eq!(pos.hash(), reparsed.hash(), "hash after {mv} in {fen}");
                assert_eq!(pos.checkers(), reparsed.checkers(), "checkers after {mv}");
                pos.unmake_move();
                assert_eq!(pos, original, "after {mv} in {fen}");
                assert_eq!(pos.hash(), original.hash());
                assert_eq!(pos.by_color, original.by_color);
                assert_eq!(pos.by_kind, original.by_kind);
            }
        }
    }

    #[test]
    fn castling_moves_the_rook_and_drops_rights() {
        let mut pos = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        pos.make_move(Move::new(Square::E1, Square::G1, MoveKind::KingCastle));
        assert_eq!(
            pos.piece_at(Square::F1),
            Some(Piece::new(Color::White, PieceKind::Rook))
        );
        assert_eq!(pos.piece_at(Square::H1), None);
        assert_eq!(pos.to_fen(), "r3k2r/8/8/8/8/8/8/R4RK1 b kq - 1 1");
        pos.make_move(Move::new(Square::A8, Square::A1, MoveKind::Capture));
        assert_eq!(pos.castling_rights(), CastlingRights::BLACK_KINGSIDE);
    }

    #[test]
    fn en_passant_square_only_when_capturable() {
        let mut pos = Position::startpos();
        pos.make_move(Move::new(Square::E2, Square::E4, MoveKind::DoublePawnPush));
        assert_eq!(pos.en_passant(), None);
        let mut pos = Position::from_fen("4k3/8/8/8/3p4/8/4P3/4K3 w - - 0 1").unwrap();
        pos.make_move(Move::new(Square::E2, Square::E4, MoveKind::DoublePawnPush));
        assert_eq!(pos.en_passant(), Some(Square::E3));
    }

    #[test]
    fn without_history_keeps_the_board() {
        let mut pos = Position::startpos();
        pos.make_move(Move::new(Square::G1, Square::F3, MoveKind::Quiet));
        let copy = pos.without_history();
        assert_eq!(copy, pos);
        assert_eq!(copy.ply(), 0);
        assert_eq!(
            pos.last_move().map(|m| m.to_string()).as_deref(),
            Some("g1f3")
        );
    }
}
