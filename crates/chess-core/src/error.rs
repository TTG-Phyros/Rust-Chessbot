use thiserror::Error;

/// Errors produced by `chess-core`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// A square index or name was outside the board.
    #[error("invalid square: {0}")]
    InvalidSquare(String),
}
