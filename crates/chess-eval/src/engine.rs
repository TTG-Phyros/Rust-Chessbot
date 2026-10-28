//! The evaluator binaries choose at runtime: the NNUE when a network is
//! loaded, the classical evaluation otherwise.

use chess_core::{Move, Position};

#[cfg(test)]
use crate::nnue::NnueWeights;
use crate::nnue::{NnueError, NnueEval, NnueState};
use crate::{ClassicEval, Evaluator, Score};

/// Classical or NNUE evaluation, decided at runtime.
#[derive(Debug, Clone)]
pub enum EngineEval {
    /// Material + piece-square tables (no network available).
    Classic(ClassicEval),
    /// Quantized neural network.
    Nnue(NnueEval),
}

impl Default for EngineEval {
    fn default() -> Self {
        Self::Classic(ClassicEval)
    }
}

impl EngineEval {
    /// Network evaluation from the bytes of a `.nnue` file (any supported
    /// format version and architecture).
    ///
    /// # Errors
    ///
    /// [`NnueError`] if the bytes are not a compatible network; callers
    /// usually fall back to [`EngineEval::default`].
    pub fn from_nnue_bytes(bytes: &[u8]) -> Result<Self, NnueError> {
        NnueEval::from_bytes(bytes).map(Self::Nnue)
    }

    /// `"nnue"` or `"classic"`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Classic(_) => "classic",
            Self::Nnue(_) => "nnue",
        }
    }
}

/// Incremental state of an [`EngineEval`].
pub enum EngineEvalState {
    /// The classical evaluation keeps no state.
    Classic,
    /// NNUE accumulators.
    Nnue(NnueState),
}

impl Evaluator for EngineEval {
    type State = EngineEvalState;

    fn evaluate(&self, pos: &Position) -> Score {
        match self {
            Self::Classic(eval) => eval.evaluate(pos),
            Self::Nnue(eval) => eval.evaluate(pos),
        }
    }

    fn new_state(&self, pos: &Position) -> EngineEvalState {
        match self {
            Self::Classic(_) => EngineEvalState::Classic,
            Self::Nnue(eval) => EngineEvalState::Nnue(eval.new_state(pos)),
        }
    }

    fn on_make(&self, state: &mut EngineEvalState, before: &Position, mv: Move) {
        if let (Self::Nnue(eval), EngineEvalState::Nnue(state)) = (self, state) {
            eval.on_make(state, before, mv);
        }
    }

    fn on_unmake(&self, state: &mut EngineEvalState) {
        if let (Self::Nnue(eval), EngineEvalState::Nnue(state)) = (self, state) {
            eval.on_unmake(state);
        }
    }

    fn evaluate_incremental(&self, state: &EngineEvalState, pos: &Position) -> Score {
        match (self, state) {
            (Self::Nnue(eval), EngineEvalState::Nnue(state)) => {
                eval.evaluate_incremental(state, pos)
            }
            _ => self.evaluate(pos),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_on_bad_networks() {
        assert!(EngineEval::from_nnue_bytes(b"garbage").is_err());
        assert_eq!(EngineEval::default().name(), "classic");
        let nnue = EngineEval::from_nnue_bytes(&NnueWeights::zeros().to_bytes()).unwrap();
        assert_eq!(nnue.name(), "nnue");
        assert_eq!(nnue.evaluate(&Position::startpos()), 0);
    }
}
