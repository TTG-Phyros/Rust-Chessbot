//! Training hyperparameters.

use chess_nnue_spec::ValidationError;
use serde::{Deserialize, Serialize};

/// How a network is trained. Every field has a default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Hyperparameters {
    /// Superbatches to train (a checkpoint is saved after each).
    pub superbatches: u64,
    /// Positions per optimizer step.
    pub batch_size: usize,
    /// Steps per superbatch.
    pub batches_per_superbatch: usize,
    /// AdamW learning rate at the start.
    pub learning_rate: f64,
    /// The learning rate is multiplied by `lrDropFactor` every
    /// `lrDropEvery` superbatches.
    pub lr_drop_every: u64,
    /// See `lrDropEvery`.
    pub lr_drop_factor: f64,
    /// Weight of the game result in the target; the rest is the search
    /// score: `(1 − wdl)·σ(score / scoreScale) + wdl·result`.
    pub wdl: f32,
    /// Centipawns per unit of the sigmoid of the target and the loss.
    pub score_scale: f32,
    /// Positions shuffled together by the loader (32 bytes of memory each).
    pub shuffle_records: usize,
}

impl Default for Hyperparameters {
    fn default() -> Self {
        Self {
            superbatches: 40,
            batch_size: 16_384,
            batches_per_superbatch: 200,
            learning_rate: 1e-3,
            lr_drop_every: 15,
            lr_drop_factor: 0.3,
            wdl: 0.25,
            score_scale: 400.0,
            shuffle_records: 2_000_000,
        }
    }
}

impl Hyperparameters {
    /// Range errors, with paths below `prefix` (e.g. `/hyperparameters`).
    #[must_use]
    pub fn validate(&self, prefix: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut check = |ok: bool, field: &str, message: &str| {
            if !ok {
                errors.push(ValidationError {
                    path: format!("{prefix}/{field}"),
                    message: message.into(),
                });
            }
        };
        check(
            (1..=100_000).contains(&self.superbatches),
            "superbatches",
            "must be between 1 and 100000",
        );
        check(
            (64..=65_536).contains(&self.batch_size),
            "batchSize",
            "must be between 64 and 65536",
        );
        check(
            (1..=100_000).contains(&self.batches_per_superbatch),
            "batchesPerSuperbatch",
            "must be between 1 and 100000",
        );
        check(
            self.learning_rate > 0.0 && self.learning_rate <= 1.0,
            "learningRate",
            "must be in (0, 1]",
        );
        check(self.lr_drop_every >= 1, "lrDropEvery", "must be at least 1");
        check(
            self.lr_drop_factor > 0.0 && self.lr_drop_factor <= 1.0,
            "lrDropFactor",
            "must be in (0, 1]",
        );
        check(
            (0.0..=1.0).contains(&self.wdl),
            "wdl",
            "must be between 0 and 1",
        );
        check(
            (1.0..=10_000.0).contains(&self.score_scale),
            "scoreScale",
            "must be between 1 and 10000",
        );
        check(
            (self.batch_size..=50_000_000).contains(&self.shuffle_records),
            "shuffleRecords",
            "must be at least batchSize and at most 50000000",
        );
        errors
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn defaults_are_valid_and_errors_have_paths() {
        assert!(Hyperparameters::default().validate("/h").is_empty());
        let bad: Hyperparameters =
            serde_json::from_str(r#"{"batchSize": 10, "wdl": 2, "shuffleRecords": 5}"#).unwrap();
        let paths: Vec<String> = bad.validate("/h").into_iter().map(|e| e.path).collect();
        assert_eq!(paths, ["/h/batchSize", "/h/wdl", "/h/shuffleRecords"]);
        assert!(serde_json::from_str::<Hyperparameters>(r#"{"batch": 1}"#).is_err());
    }
}
