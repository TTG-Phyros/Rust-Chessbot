//! NNUE evaluation: a [`chess_nnue::Network`] (any architecture) behind the
//! [`Evaluator`] trait.
//!
//! The names of the version 1 network ([`NnueWeights`], [`feature_index`],
//! [`SCALE`], …) are re-exported for the trainer, which still produces
//! that architecture.

use std::sync::Arc;

use chess_core::{Move, Position};

pub use chess_nnue::legacy::{
    HIDDEN, INPUTS, NnueWeights, QA, QB, SCALE, active_features, feature_index,
};
pub use chess_nnue::{Network, NnueError, NnueState};

use crate::{Evaluator, Score};

/// [`Evaluator`] backed by a quantized network.
#[derive(Debug, Clone)]
pub struct NnueEval {
    network: Arc<Network>,
}

impl NnueEval {
    /// Wraps a loaded network.
    #[must_use]
    pub fn new(network: Network) -> Self {
        Self {
            network: Arc::new(network),
        }
    }

    /// Loads a network file (any supported format version).
    ///
    /// # Errors
    ///
    /// [`NnueError`] if the bytes are not a valid network.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, NnueError> {
        Network::from_bytes(bytes).map(Self::new)
    }

    /// Evaluator of version 1 weights.
    ///
    /// # Errors
    ///
    /// [`NnueError::Range`] if the weights exceed the legacy bounds.
    pub fn from_legacy(weights: &NnueWeights) -> Result<Self, NnueError> {
        Network::from_legacy(weights).map(Self::new)
    }

    /// The network.
    #[must_use]
    pub fn network(&self) -> &Network {
        &self.network
    }
}

impl Evaluator for NnueEval {
    type State = NnueState;

    fn evaluate(&self, pos: &Position) -> Score {
        self.network.evaluate(pos)
    }

    fn new_state(&self, pos: &Position) -> NnueState {
        self.network.new_state(pos)
    }

    fn on_make(&self, state: &mut NnueState, before: &Position, mv: Move) {
        self.network.on_make(state, before, mv);
    }

    fn on_unmake(&self, state: &mut NnueState) {
        self.network.on_unmake(state);
    }

    fn evaluate_incremental(&self, state: &NnueState, pos: &Position) -> Score {
        self.network.evaluate_incremental(state, pos)
    }
}
