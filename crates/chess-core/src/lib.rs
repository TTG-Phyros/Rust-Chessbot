//! Core chess primitives: bitboards, squares, pieces and positions.
//!
//! This crate is pure computation: it performs no I/O and depends on no other
//! crate of the project. Every other crate builds on the types defined here.

pub mod attacks;
mod bitboard;
mod error;
mod magic;
mod piece;
mod position;
mod square;

pub use bitboard::Bitboard;
pub use error::CoreError;
pub use piece::{Color, Piece, PieceKind};
pub use position::Position;
pub use square::{File, Rank, Square};
