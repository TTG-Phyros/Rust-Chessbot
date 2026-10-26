//! Networks: named training runs, each with its own hyperparameters saved
//! in `<data_dir>/runs/<name>.json`, so that a run always resumes with the
//! settings it was created with.
//!
//! A network owns `checkpoints/<name>/`, `weights/<name>.nnue` (latest
//! export), `weights/<name>/` (snapshots and their ranking) and the dataset
//! files `<name>-NNNN` it generated.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::trainer::write_atomically;

/// The settings worth varying between networks; everything else comes from
/// the daemon's configuration. Written in camelCase: `chess-server` reads
/// these files as they are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct NetworkParams {
    /// Nodes per move in self-play: stronger games, generated more slowly.
    pub(crate) selfplay_nodes: u64,
    pub(crate) learning_rate: f64,
    pub(crate) lr_drop_every: u64,
    pub(crate) lr_drop_factor: f64,
    /// Weight of the game result in the target.
    pub(crate) wdl: f32,
    pub(crate) batch_size: usize,
    pub(crate) batches_per_superbatch: usize,
    /// 0 = until stopped.
    pub(crate) superbatches: u64,
    pub(crate) export_every: u64,
    pub(crate) min_positions: u64,
    pub(crate) max_reuse: u64,
    /// Games of each measurement against the classical evaluation.
    pub(crate) eval_games: u32,
    /// Other networks whose self-play games this one also trains on.
    pub(crate) datasets_from: Vec<String>,
}

impl Default for NetworkParams {
    fn default() -> Self {
        Self::of(&Config::default())
    }
}

impl NetworkParams {
    /// The parameters `config` would train with.
    pub(crate) fn of(config: &Config) -> Self {
        Self {
            selfplay_nodes: config.selfplay.nodes,
            learning_rate: config.train.learning_rate,
            lr_drop_every: config.train.lr_drop_every,
            lr_drop_factor: config.train.lr_drop_factor,
            wdl: config.train.wdl,
            batch_size: config.train.batch_size,
            batches_per_superbatch: config.train.batches_per_superbatch,
            superbatches: config.train.superbatches,
            export_every: config.train.export_every,
            min_positions: config.min_positions,
            max_reuse: config.max_reuse,
            eval_games: config.evaluation.games,
            datasets_from: config.datasets_from.clone(),
        }
    }

    pub(crate) fn apply(&self, config: &mut Config) {
        config.selfplay.nodes = self.selfplay_nodes;
        config.train.learning_rate = self.learning_rate;
        config.train.lr_drop_every = self.lr_drop_every;
        config.train.lr_drop_factor = self.lr_drop_factor;
        config.train.wdl = self.wdl;
        config.train.batch_size = self.batch_size;
        config.train.batches_per_superbatch = self.batches_per_superbatch;
        config.train.superbatches = self.superbatches;
        config.train.export_every = self.export_every;
        config.min_positions = self.min_positions;
        config.max_reuse = self.max_reuse;
        config.evaluation.games = self.eval_games;
        config.datasets_from.clone_from(&self.datasets_from);
    }

    /// Why these parameters cannot train, if they cannot.
    pub(crate) fn validate(&self) -> Result<(), String> {
        fn check(ok: bool, message: &str) -> Result<(), String> {
            if ok { Ok(()) } else { Err(message.to_owned()) }
        }
        check(
            (100..=1_000_000).contains(&self.selfplay_nodes),
            "self-play nodes must be between 100 and 1 000 000",
        )?;
        check(
            self.learning_rate > 0.0 && self.learning_rate <= 1.0,
            "the learning rate must be in ]0, 1]",
        )?;
        check(
            self.lr_drop_every >= 1,
            "the learning rate drop interval must be at least 1",
        )?;
        check(
            self.lr_drop_factor > 0.0 && self.lr_drop_factor <= 1.0,
            "the learning rate drop factor must be in ]0, 1]",
        )?;
        check(
            (0.0..=1.0).contains(&self.wdl),
            "the result weight (wdl) must be in [0, 1]",
        )?;
        check(
            (64..=65_536).contains(&self.batch_size),
            "the batch size must be between 64 and 65 536",
        )?;
        check(
            (1..=100_000).contains(&self.batches_per_superbatch),
            "batches per superbatch must be between 1 and 100 000",
        )?;
        check(
            self.export_every >= 1,
            "the export interval must be at least 1",
        )?;
        check(
            (2..=10_000).contains(&self.eval_games),
            "evaluation games must be between 2 and 10 000",
        )?;
        for name in &self.datasets_from {
            validate_name(name)?;
        }
        Ok(())
    }
}

/// Name of the classical evaluation in matches; no network may take it.
pub(crate) const CLASSIC: &str = "classic";

/// Network and bot names: 1 to 32 letters, digits, `-` or `_`, starting with
/// a letter or a digit (they name files and directories).
pub(crate) fn validate_name(name: &str) -> Result<(), String> {
    let valid = (1..=32).contains(&name.len())
        && name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !valid {
        return Err(format!(
            "invalid name {name:?}: 1 to 32 letters, digits, '-' or '_', starting with a letter or a digit"
        ));
    }
    if name.eq_ignore_ascii_case(CLASSIC) {
        return Err(format!(
            "{CLASSIC:?} is reserved for the classical evaluation"
        ));
    }
    if name.to_ascii_lowercase().starts_with("stockfish") {
        return Err("names starting with \"stockfish\" are reserved for the Stockfish bots".into());
    }
    Ok(())
}

fn params_path(config: &Config, name: &str) -> PathBuf {
    config.runs_dir().join(format!("{name}.json"))
}

pub(crate) fn load_params(config: &Config, name: &str) -> Result<Option<NetworkParams>> {
    let path = params_path(config, name);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .with_context(|| format!("invalid network settings in {}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("cannot read {}", path.display())),
    }
}

pub(crate) fn save_params(config: &Config, name: &str, params: &NetworkParams) -> Result<()> {
    write_atomically(
        &params_path(config, name),
        &serde_json::to_vec_pretty(params)?,
    )
}

/// The configuration that trains network `name`: `base` with the network's
/// parameters. A network without settings yet (e.g. created from the command
/// line) gets those of `base`, saved for the next runs.
pub(crate) fn config_for(base: &Config, name: &str) -> Result<Config> {
    let mut config = base.clone();
    config.run = name.to_owned();
    match load_params(base, name)? {
        Some(params) => params.apply(&mut config),
        None => save_params(base, name, &NetworkParams::of(&config))?,
    }
    Ok(config)
}

/// Every network with settings, a checkpoint or an exported network.
pub(crate) fn list(config: &Config) -> Vec<String> {
    let stems = |dir: PathBuf, extension: Option<&str>| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(move |path| match extension {
                Some(extension) => path.extension().is_some_and(|ext| ext == extension),
                None => path.is_dir() && path.extension().is_none(),
            })
            .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
            .collect::<Vec<_>>()
    };
    let mut names: Vec<String> = stems(config.runs_dir(), Some("json"))
        .into_iter()
        .chain(stems(config.checkpoint_dir(), None))
        .chain(stems(config.weights_dir(), Some("nnue")))
        .filter(|name| validate_name(name).is_ok())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Deletes the settings, checkpoint and weights of `name`. Its dataset files
/// stay: other networks may train on them.
pub(crate) fn delete(config: &Config, name: &str) -> Result<()> {
    let remove_file = |path: PathBuf| match std::fs::remove_file(&path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            Err(err).with_context(|| format!("cannot delete {}", path.display()))
        }
        _ => Ok(()),
    };
    let remove_dir = |path: PathBuf| match std::fs::remove_dir_all(&path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            Err(err).with_context(|| format!("cannot delete {}", path.display()))
        }
        _ => Ok(()),
    };
    remove_dir(config.checkpoint_dir().join(name))?;
    remove_dir(config.weights_dir().join(name))?;
    remove_file(config.weights_dir().join(format!("{name}.nnue")))?;
    remove_file(params_path(config, name))?;
    tracing::info!(network = name, "network deleted");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir;

    #[test]
    fn names_are_restricted() {
        for name in ["main", "exp-2", "lr_3e4", "A1"] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
        for name in [
            "",
            "-x",
            "_x",
            "a/b",
            "..",
            "a b",
            "classic",
            "Classic",
            "stockfish-3",
            "Stockfish2",
            &"x".repeat(33),
        ] {
            assert!(validate_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn params_round_trip_and_validate() {
        let config = Config::default();
        let params = NetworkParams::of(&config);
        assert_eq!(params, NetworkParams::default());
        assert_eq!(params.validate(), Ok(()));
        let mut changed = config.clone();
        let custom = NetworkParams {
            selfplay_nodes: 8_000,
            learning_rate: 3e-4,
            datasets_from: vec!["main".into()],
            ..params.clone()
        };
        custom.apply(&mut changed);
        assert_eq!(changed.selfplay.nodes, 8_000);
        assert_eq!(NetworkParams::of(&changed), custom);

        let broken = NetworkParams {
            learning_rate: 0.0,
            ..params.clone()
        };
        assert!(broken.validate().is_err());
        let bad_source = NetworkParams {
            datasets_from: vec!["../x".into()],
            ..params
        };
        assert!(bad_source.validate().is_err());
        // Settings written before a field existed get its default.
        let old: NetworkParams = serde_json::from_str(r#"{"selfplayNodes": 1234}"#).unwrap();
        assert_eq!(old.selfplay_nodes, 1234);
        assert_eq!(old.batch_size, Config::default().train.batch_size);
    }

    #[test]
    fn networks_are_configured_listed_and_deleted() {
        let dir = test_dir("network");
        let base = Config {
            data_dir: dir.clone(),
            ..Config::default()
        };
        // First use: the base settings are recorded.
        let config = config_for(&base, "first").unwrap();
        assert_eq!(config.run, "first");
        assert_eq!(
            load_params(&base, "first").unwrap(),
            Some(NetworkParams::of(&base))
        );
        let custom = NetworkParams {
            selfplay_nodes: 777,
            ..NetworkParams::default()
        };
        save_params(&base, "second", &custom).unwrap();
        assert_eq!(config_for(&base, "second").unwrap().selfplay.nodes, 777);

        std::fs::create_dir_all(base.checkpoint_dir().join("third")).unwrap();
        std::fs::create_dir_all(base.weights_dir().join("first")).unwrap();
        std::fs::write(base.weights_dir().join("first.nnue"), b"").unwrap();
        assert_eq!(list(&base), ["first", "second", "third"]);

        delete(&base, "first").unwrap();
        assert_eq!(list(&base), ["second", "third"]);
        assert!(!base.weights_dir().join("first").exists());
        // Deleting what is not there is not an error.
        delete(&base, "first").unwrap();
    }
}
