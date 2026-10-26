//! Configuration: built-in defaults, then an optional TOML file, then
//! `CHESS_TRAIN_*` environment variables (nested keys use `__`, e.g.
//! `CHESS_TRAIN_SELFPLAY__NODES=8000`). Command-line flags override all.

use std::path::PathBuf;

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

use crate::arena::ArenaConfig;
use crate::evaluation::EvaluationConfig;
use crate::selfplay::SelfplayConfig;
use crate::trainer::TrainConfig;

/// Environment variable naming the configuration file.
pub(crate) const CONFIG_PATH_ENV: &str = "CHESS_TRAIN_CONFIG";
/// Configuration file read when [`CONFIG_PATH_ENV`] is unset (ignored if missing).
pub(crate) const DEFAULT_CONFIG_PATH: &str = "chess-train.toml";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    /// Name of the run: prefixes dataset files, names checkpoints and weights.
    pub(crate) run: String,
    /// Weights, checkpoints and metrics.
    pub(crate) data_dir: PathBuf,
    /// Dataset files (streamed during training: put them on a fast disk).
    pub(crate) dataset_dir: PathBuf,
    /// Pipeline: positions to generate before training starts.
    pub(crate) min_positions: u64,
    pub(crate) selfplay: SelfplayConfig,
    pub(crate) train: TrainConfig,
    pub(crate) evaluation: EvaluationConfig,
    pub(crate) arena: ArenaConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            run: "main".into(),
            data_dir: PathBuf::from("data"),
            dataset_dir: PathBuf::from("data/datasets"),
            min_positions: 2_000_000,
            selfplay: SelfplayConfig::default(),
            train: TrainConfig::default(),
            evaluation: EvaluationConfig::default(),
            arena: ArenaConfig::default(),
        }
    }
}

impl Config {
    pub(crate) fn load(path: Option<&str>) -> Result<Self, Box<figment::Error>> {
        let path = path
            .map(str::to_owned)
            .or_else(|| std::env::var(CONFIG_PATH_ENV).ok())
            .unwrap_or_else(|| DEFAULT_CONFIG_PATH.into());
        Self::figment(&path).extract().map_err(Box::new)
    }

    fn figment(path: &str) -> Figment {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Toml::file(path))
            .merge(
                Env::prefixed("CHESS_TRAIN_")
                    .split("__")
                    .ignore(&["config", "device"]),
            )
    }

    pub(crate) fn checkpoint_dir(&self) -> PathBuf {
        self.data_dir.join("checkpoints")
    }

    pub(crate) fn weights_dir(&self) -> PathBuf {
        self.data_dir.join("weights")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::result_large_err)] // The closure signature is imposed by figment's Jail.
    fn file_and_environment_layers() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "train.toml",
                "run = \"exp1\"\n[selfplay]\nnodes = 1234\n[train]\nbatch_size = 512\n",
            )?;
            jail.set_env("CHESS_TRAIN_SELFPLAY__NODES", "999");
            jail.set_env("CHESS_TRAIN_DATASET_DIR", "/datasets");
            jail.set_env("CHESS_TRAIN_DEVICE", "cpu");
            let config: Config = Config::figment("train.toml").extract()?;
            assert_eq!(config.run, "exp1");
            assert_eq!(config.selfplay.nodes, 999);
            assert_eq!(config.train.batch_size, 512);
            assert_eq!(config.dataset_dir, PathBuf::from("/datasets"));
            assert_eq!(
                config.selfplay.max_plies,
                SelfplayConfig::default().max_plies
            );
            Ok(())
        });
    }
}
