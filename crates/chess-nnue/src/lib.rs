//! NNUE inference for any architecture of `chess-nnue-spec`.
//!
//! A [`Network`] is loaded from a `CBNN` file whose header carries its
//! architecture (version 2), or from a legacy version 1 file. Evaluation is
//! integer-only and incremental: [`Network::new_state`],
//! [`Network::on_make`], [`Network::on_unmake`] and
//! [`Network::evaluate_incremental`] keep a stack of accumulators updated
//! with the features each move adds and removes; `chess-eval` plugs them
//! into its `Evaluator` trait.
//!
//! Inference is generic (shapes read from the architecture), with
//! vectorized kernels for accumulator updates and a fused output for the
//! common "accumulator → clipped ReLU → output" shape.

mod divisor;
mod features;
mod format;
pub mod legacy;
mod network;
mod simd;
mod state;

use thiserror::Error;

pub use chess_nnue_spec as spec;
pub use features::Encoder;
pub use format::{FORMAT_VERSION, FileInfo, inspect, legacy_architecture};
pub use network::{DenseWeights, Network};
pub use state::NnueState;

/// Score in centipawns, for the side to move.
pub type Score = i32;

/// First bytes of every network file.
pub(crate) const MAGIC: &[u8; 4] = b"CBNN";

/// Why a network was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NnueError {
    /// Not a `CBNN` file.
    #[error("not a chess-bot network file (bad magic)")]
    BadMagic,
    /// Written by an unknown format version.
    #[error("unsupported network format version {0} (this build reads 1 and {FORMAT_VERSION})")]
    UnsupportedVersion(u32),
    /// The architecture in the file is invalid or unsupported.
    #[error("invalid network architecture: {0}")]
    Architecture(String),
    /// The architecture does not have the expected hash: either the file's
    /// header was altered, or it is not the network that was asked for.
    #[error("architecture hash mismatch: expected {expected}, found {found}")]
    HashMismatch {
        /// Hash that was expected (recorded in the file, or requested).
        expected: String,
        /// Hash of the architecture actually there.
        found: String,
    },
    /// The weights do not match the checksum of the header.
    #[error("network weights are corrupted (checksum mismatch)")]
    Corrupted,
    /// Truncated or oversized file.
    #[error("network file has {actual} bytes, expected {expected}")]
    Length {
        /// Size of the file.
        actual: usize,
        /// Size implied by the header.
        expected: usize,
    },
    /// A tensor does not have the size the architecture implies.
    #[error("network shape mismatch: {0}")]
    Shape(String),
    /// Weights too large for the integer arithmetic of the architecture.
    #[error("network weights out of range: {0}")]
    Range(String),
}
