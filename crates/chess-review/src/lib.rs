//! Review of a chess game, in the manner of Chess.com's *Game Review*.
//!
//! - [`engine`]: the [`AnalysisEngine`] trait, implemented by Stockfish over
//!   UCI (feature `stockfish`, the reference) and by our own engine
//!   (feature `native`), and the settings of a search (depth or time per
//!   move, number of lines).
//! - [`analysis`]: the evaluation of every position of a game, best lines
//!   included, by such an engine.
//! - [`classify`]: the class of each move (Brilliant, Great, Best… Miss,
//!   Blunder) and the accuracy of the players, from such an analysis.
//! - [`accuracy`]: the formulas of Lichess for winning chances and accuracy.
//! - [`exchange`]: material at stake on the board (sacrifices).
//! - [`motifs`]: the phase of a position and the tactical motif of a mistake.
//! - [`bench`]: how long an engine takes per position, for estimates.
//!
//! The crate performs no I/O other than driving the engine processes.

pub mod accuracy;
pub mod analysis;
pub mod bench;
pub mod classify;
pub mod engine;
pub mod exchange;
pub mod motifs;

pub use analysis::{GameAnalysis, PositionEval, analyse_game};
pub use classify::{GameReview, MoveClass, MoveReview, PlayerReview, Thresholds, review_game};
pub use engine::{AnalysisEngine, Budget, EngineSpec, Line, Score, SearchSettings};
