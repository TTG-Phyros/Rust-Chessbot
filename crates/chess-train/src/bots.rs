//! Bots: trained networks saved under a name (`<data_dir>/bots/<name>.nnue`
//! and its description `<name>.json`), out of reach of the training that
//! produced them, to play tournaments.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chess_eval::EngineEval;
use serde::{Deserialize, Serialize};

use crate::arena::{self, Contestant};
use crate::config::Config;
use crate::metrics::now;
use crate::network::{self, CLASSIC, NetworkParams};
use crate::ranking::{self, snapshot_name};
use crate::stockfish;
use crate::trainer::write_atomically;

/// Description of a bot. Written in camelCase: `chess-server` reads it as it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BotInfo {
    pub(crate) name: String,
    /// Network it was saved from.
    pub(crate) network: String,
    /// Snapshot saved; absent: the latest export at the time.
    pub(crate) superbatch: Option<u64>,
    /// Elo of the snapshot against the classical evaluation, when it was measured.
    pub(crate) elo: Option<f64>,
    pub(crate) elo_error: Option<f64>,
    /// Hyperparameters of the network it comes from.
    pub(crate) params: Option<NetworkParams>,
    /// Unix seconds.
    pub(crate) created_at: u64,
}

/// What to save: a snapshot of `network`, or its latest export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveBotRequest {
    pub(crate) name: String,
    pub(crate) network: String,
    #[serde(default)]
    pub(crate) superbatch: Option<u64>,
}

fn weights_path(config: &Config, name: &str) -> PathBuf {
    config.bots_dir().join(format!("{name}.nnue"))
}

fn info_path(config: &Config, name: &str) -> PathBuf {
    config.bots_dir().join(format!("{name}.json"))
}

pub(crate) fn exists(config: &Config, name: &str) -> bool {
    weights_path(config, name).exists()
}

/// Network file the request designates, if it exists.
pub(crate) fn source(config: &Config, request: &SaveBotRequest) -> Option<PathBuf> {
    let path = match request.superbatch {
        Some(superbatch) => config
            .weights_dir()
            .join(&request.network)
            .join(snapshot_name(superbatch)),
        None => config
            .weights_dir()
            .join(format!("{}.nnue", request.network)),
    };
    path.exists().then_some(path)
}

/// Copies the designated network (which must exist) to a new bot.
pub(crate) fn save(config: &Config, request: &SaveBotRequest) -> Result<BotInfo> {
    let Some(source) = source(config, request) else {
        bail!("no such network file");
    };
    let bytes =
        std::fs::read(&source).with_context(|| format!("cannot read {}", source.display()))?;
    // Refuse a file the engine could not load.
    EngineEval::from_nnue_bytes(&bytes)
        .with_context(|| format!("{} is not a valid network", source.display()))?;
    let measured = request.superbatch.and_then(|superbatch| {
        ranking::load(&config.weights_dir().join(&request.network))?
            .into_iter()
            .find(|ranked| ranked.superbatch == superbatch)
    });
    let info = BotInfo {
        name: request.name.clone(),
        network: request.network.clone(),
        superbatch: request.superbatch,
        elo: measured.as_ref().map(|ranked| ranked.elo),
        elo_error: measured.as_ref().map(|ranked| ranked.elo_error),
        params: network::load_params(config, &request.network)
            .ok()
            .flatten(),
        created_at: now(),
    };
    write_atomically(&weights_path(config, &request.name), &bytes)?;
    write_atomically(
        &info_path(config, &request.name),
        &serde_json::to_vec_pretty(&info)?,
    )?;
    tracing::info!(bot = %info.name, network = %info.network, superbatch = ?info.superbatch, "bot saved");
    Ok(info)
}

pub(crate) fn delete(config: &Config, name: &str) -> Result<()> {
    std::fs::remove_file(weights_path(config, name))
        .with_context(|| format!("cannot delete bot {name}"))?;
    let _ = std::fs::remove_file(info_path(config, name));
    tracing::info!(bot = name, "bot deleted");
    Ok(())
}

/// A bot ready to play: [`CLASSIC`], a Stockfish level (`stockfish-N`) or
/// a saved network.
pub(crate) fn contestant(config: &Config, name: &str) -> Result<Contestant> {
    if name.eq_ignore_ascii_case(CLASSIC) {
        return Ok(Contestant::Engine(EngineEval::default()));
    }
    if stockfish::level_of(name).is_some() {
        return Contestant::load(name, &config.arena);
    }
    arena::load_evaluator(&weights_path(config, name).display().to_string()).map(Contestant::Engine)
}

#[cfg(test)]
mod tests {
    use chess_eval::nnue::NnueWeights;

    use super::*;
    use crate::ranking::Ranked;
    use crate::test_dir;

    #[test]
    fn saves_snapshots_and_latest_exports() {
        let dir = test_dir("bots");
        let config = Config {
            data_dir: dir,
            ..Config::default()
        };
        let snapshots = config.weights_dir().join("main");
        std::fs::create_dir_all(&snapshots).unwrap();
        let net = NnueWeights::zeros().to_bytes();
        std::fs::write(snapshots.join(snapshot_name(10)), &net).unwrap();
        std::fs::write(config.weights_dir().join("main.nnue"), &net).unwrap();
        ranking::record(
            &snapshots,
            Ranked {
                superbatch: 10,
                elo: 42.0,
                elo_error: 30.0,
                games: 64,
                measured_at: 1,
            },
            5,
        )
        .unwrap();
        network::save_params(&config, "main", &NetworkParams::default()).unwrap();

        let snapshot = SaveBotRequest {
            name: "best".into(),
            network: "main".into(),
            superbatch: Some(10),
        };
        let info = save(&config, &snapshot).unwrap();
        assert_eq!((info.elo, info.superbatch), (Some(42.0), Some(10)));
        assert_eq!(info.params, Some(NetworkParams::default()));
        assert!(exists(&config, "best"));
        assert_eq!(contestant(&config, "best").unwrap().name(), "nnue");
        assert_eq!(contestant(&config, "classic").unwrap().name(), "classic");
        assert_eq!(
            contestant(&config, "stockfish-3").unwrap().name(),
            "stockfish-3"
        );

        let latest = SaveBotRequest {
            name: "latest".into(),
            network: "main".into(),
            superbatch: None,
        };
        assert_eq!(save(&config, &latest).unwrap().elo, None);
        let missing = SaveBotRequest {
            superbatch: Some(99),
            ..snapshot
        };
        assert!(source(&config, &missing).is_none());
        assert!(save(&config, &missing).is_err());

        delete(&config, "best").unwrap();
        assert!(!exists(&config, "best"));
        assert!(contestant(&config, "best").is_err());
    }
}
