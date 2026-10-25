//! Static evaluation of chess positions.
//!
//! The [`Evaluator`] trait is always available. Concrete evaluators sit behind
//! cargo features so that `chess-search`, which depends on this crate with
//! `default-features = false`, can only ever see the trait.

use chess_core::Position;

#[cfg(feature = "classic")]
mod classic;

#[cfg(feature = "classic")]
pub use classic::ClassicEval;

/// Score in centipawns.
pub type Score = i32;

/// Static evaluation of a position.
///
/// Implementations must be cheap to call and thread-safe: the search calls
/// them at every leaf, possibly from several threads.
pub trait Evaluator: Send + Sync {
    /// Evaluates `pos` in centipawns from the point of view of the side to move
    /// (positive means the side to move is better), as negamax expects.
    fn evaluate(&self, pos: &Position) -> Score;
}

impl<E: Evaluator + ?Sized> Evaluator for &E {
    fn evaluate(&self, pos: &Position) -> Score {
        (**self).evaluate(pos)
    }
}

impl<E: Evaluator + ?Sized> Evaluator for Box<E> {
    fn evaluate(&self, pos: &Position) -> Score {
        (**self).evaluate(pos)
    }
}

impl<E: Evaluator + ?Sized> Evaluator for std::sync::Arc<E> {
    fn evaluate(&self, pos: &Position) -> Score {
        (**self).evaluate(pos)
    }
}
