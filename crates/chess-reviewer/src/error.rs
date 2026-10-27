//! Errors of the review service.

/// What can go wrong while syncing, analysing or reading the cache.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The Chess.com API failed.
    #[error(transparent)]
    ChessCom(#[from] chess_chesscom::Error),
    /// The cache database failed.
    #[error("review cache: {0}")]
    Store(#[from] rusqlite::Error),
    /// The cache directory could not be created.
    #[error("review cache: {0}")]
    Io(#[from] std::io::Error),
    /// A cached row could not be decoded.
    #[error("review cache: corrupt row: {0}")]
    Corrupt(String),
    /// The analysis engine could not be started.
    #[error("analysis engine: {0}")]
    Engine(String),
    /// A request out of bounds.
    #[error("{0}")]
    Invalid(String),
    /// The service is not configured for this request.
    #[error("{0}")]
    Unavailable(String),
}
