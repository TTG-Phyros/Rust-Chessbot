//! Review of a chess game, in the manner of Chess.com's *Game Review*.
//!
//! - [`engine`]: the [`AnalysisEngine`] trait, implemented by Stockfish over
//!   UCI (feature `stockfish`, the reference) and by our own engine
//!   (feature `native`), and the settings of a search (depth or time per
//!   move, number of lines).
//! - [`analysis`]: the evaluation of every position of a game, best lines
//!   included, by such an engine.
//! - [`bench`]: how long an engine takes per position, for estimates.
//!
//! The crate performs no I/O other than driving the engine processes.

pub mod analysis;
pub mod bench;
pub mod engine;

pub use analysis::{GameAnalysis, PositionEval, analyse_game};
pub use engine::{AnalysisEngine, Budget, EngineSpec, Line, Score, SearchSettings};
