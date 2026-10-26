//! Static evaluation of chess positions.
//!
//! The [`Evaluator`] trait is always available. Concrete evaluators sit behind
//! cargo features so that `chess-search`, which depends on this crate with
//! `default-features = false`, can only ever see the trait.

use chess_core::{Move, Position};

#[cfg(feature = "classic")]
mod classic;

#[cfg(feature = "classic")]
pub use classic::ClassicEval;

#[cfg(feature = "nnue")]
pub mod nnue;

#[cfg(all(feature = "classic", feature = "nnue"))]
mod engine;

#[cfg(all(feature = "classic", feature = "nnue"))]
pub use engine::{EngineEval, EngineEvalState};

/// Score in centipawns.
pub type Score = i32;

/// Static evaluation of a position.
///
/// Implementations must be cheap to call and thread-safe: the search calls
/// them at every leaf, possibly from several threads.
///
/// An evaluator may keep incremental state through a search (e.g. NNUE
/// accumulators): the search creates it with [`Evaluator::new_state`], calls
/// [`Evaluator::on_make`] before every move it plays and
/// [`Evaluator::on_unmake`] after taking it back, and evaluates with
/// [`Evaluator::evaluate_incremental`]. Stateless evaluators use `()`.
pub trait Evaluator: Send + Sync {
    /// Per-search incremental state; `()` for stateless evaluators.
    type State: Send;

    /// Evaluates `pos` from scratch, in centipawns from the point of view of
    /// the side to move (positive means the side to move is better).
    fn evaluate(&self, pos: &Position) -> Score;

    /// State for a search starting at `pos`.
    fn new_state(&self, pos: &Position) -> Self::State;

    /// `mv` (possibly [`Move::NULL`] for a null move) is about to be played
    /// in `before`.
    fn on_make(&self, state: &mut Self::State, before: &Position, mv: Move);

    /// The last move passed to [`Evaluator::on_make`] was taken back.
    fn on_unmake(&self, state: &mut Self::State);

    /// Same result as [`Evaluator::evaluate`], using the incremental state.
    fn evaluate_incremental(&self, state: &Self::State, pos: &Position) -> Score;
}

macro_rules! forward_evaluator {
    ($($wrapper:ty),*) => {
        $(
            impl<E: Evaluator + ?Sized> Evaluator for $wrapper {
                type State = E::State;

                fn evaluate(&self, pos: &Position) -> Score {
                    (**self).evaluate(pos)
                }

                fn new_state(&self, pos: &Position) -> Self::State {
                    (**self).new_state(pos)
                }

                fn on_make(&self, state: &mut Self::State, before: &Position, mv: Move) {
                    (**self).on_make(state, before, mv);
                }

                fn on_unmake(&self, state: &mut Self::State) {
                    (**self).on_unmake(state);
                }

                fn evaluate_incremental(&self, state: &Self::State, pos: &Position) -> Score {
                    (**self).evaluate_incremental(state, pos)
                }
            }
        )*
    };
}

forward_evaluator!(&E, Box<E>, std::sync::Arc<E>);
