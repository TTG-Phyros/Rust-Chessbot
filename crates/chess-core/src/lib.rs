//! Core chess primitives: bitboards, positions, legal move generation,
//! FEN/SAN/UCI/PGN notations, Zobrist hashing and end-of-game detection.
//!
//! This crate is pure computation: it performs no I/O and depends on no other
//! crate of the project. Every other crate builds on the types defined here.
//!
//! ```
//! use chess_core::{Position, perft};
//!
//! let mut pos = Position::startpos();
//! let mv = pos.parse_san("e4")?;
//! pos.make_move(mv);
//! assert_eq!(pos.to_fen(), "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
//! assert_eq!(perft(&mut pos, 2), 600);
//! # Ok::<(), chess_core::CoreError>(())
//! ```

pub mod attacks;
mod bitboard;
mod error;
mod magic;
mod moves;
pub mod notation;
mod perft;
pub mod pgn;
mod piece;
mod position;
mod square;
mod zobrist;

pub use bitboard::Bitboard;
pub use error::CoreError;
pub use moves::{MAX_MOVES, Move, MoveKind, MoveList};
pub use notation::MoveParseError;
pub use perft::{perft, perft_divide};
pub use piece::{Color, Piece, PieceKind};
pub use position::{CastlingRights, DrawReason, FenError, GameStatus, Position, STARTPOS_FEN};
pub use square::{File, Rank, Square};
