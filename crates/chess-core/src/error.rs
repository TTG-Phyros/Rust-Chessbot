use thiserror::Error;

use crate::notation::MoveParseError;
use crate::pgn::PgnError;
use crate::position::FenError;

/// Errors produced by `chess-core`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// A square name was not in `a1`..`h8`.
    #[error("invalid square: {0:?}")]
    InvalidSquare(String),
    /// A move is not legal in the position it was played in.
    #[error("illegal move: {0}")]
    IllegalMove(String),
    /// Invalid FEN.
    #[error(transparent)]
    Fen(#[from] FenError),
    /// Invalid move in UCI or SAN notation.
    #[error(transparent)]
    Move(#[from] MoveParseError),
    /// Invalid PGN.
    #[error(transparent)]
    Pgn(#[from] PgnError),
}
