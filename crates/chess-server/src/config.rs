//! Server configuration: built-in defaults, then an optional TOML file, then
//! `CHESS_SERVER_*` environment variables (highest priority).

use std::net::SocketAddr;
use std::path::PathBuf;

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

/// Environment variable naming an alternative config file.
pub const CONFIG_PATH_ENV: &str = "CHESS_SERVER_CONFIG";
/// Config file read when [`CONFIG_PATH_ENV`] is unset (ignored if missing).
pub const DEFAULT_CONFIG_PATH: &str = "chess-server.toml";

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Human-readable, for terminals.
    Pretty,
    /// One JSON object per line, for log collectors.
    Json,
}

/// Runtime configuration of the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Address the HTTP server listens on.
    pub bind: SocketAddr,
    /// Directory holding weights and training metrics.
    pub data_dir: PathBuf,
    /// Log output format.
    pub log_format: LogFormat,
    /// Maximum number of engine searches running at once (one CPU core each).
    pub max_searches: usize,
    /// Transposition table size of each search, in MiB.
    pub engine_hash_mb: usize,
    /// Network used by the engine (reloaded when it changes); classical
    /// evaluation while the file does not exist.
    pub eval_file: PathBuf,
    /// Base URL of the training control daemon (`chess-train serve`), e.g.
    /// `http://trainer:4000`; training cannot be started from the UI without it.
    pub trainer_url: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::from(([0, 0, 0, 0], 3000)),
            data_dir: PathBuf::from("data"),
            log_format: LogFormat::Pretty,
            max_searches: std::thread::available_parallelism().map_or(2, std::num::NonZero::get),
            engine_hash_mb: 16,
            eval_file: PathBuf::from("data/weights/main.nnue"),
            trainer_url: None,
        }
    }
}

impl ServerConfig {
    /// Loads the configuration from defaults, the TOML file and the environment.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        let path = std::env::var(CONFIG_PATH_ENV).unwrap_or_else(|_| DEFAULT_CONFIG_PATH.into());
        Self::figment(&path).extract().map_err(Box::new)
    }

    fn figment(path: &str) -> Figment {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Toml::file(path))
            .merge(Env::prefixed("CHESS_SERVER_"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_without_file() {
        let config: ServerConfig = ServerConfig::figment("does-not-exist.toml")
            .extract()
            .unwrap();
        assert_eq!(config.bind.port(), 3000);
    }

    #[test]
    #[allow(clippy::result_large_err)] // The closure signature is imposed by figment's Jail.
    fn env_overrides_file() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "server.toml",
                "bind = \"127.0.0.1:4000\"\nlog_format = \"json\"",
            )?;
            jail.set_env("CHESS_SERVER_BIND", "127.0.0.1:5000");
            let config: ServerConfig = ServerConfig::figment("server.toml").extract()?;
            assert_eq!(config.bind.port(), 5000);
            assert_eq!(config.log_format, LogFormat::Json);
            Ok(())
        });
    }
}
