//! Server configuration: built-in defaults, then an optional TOML file, then
//! `CHESS_SERVER_*` environment variables (highest priority). The NNUE lab's
//! settings also read `CHESS_API_*` and `CHESS_LAB_*` (`CHESS_API_KEYS`
//! sets `api_keys`, `CHESS_LAB_DIR` sets `lab_dir`, …), the names `.env`
//! uses.

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

/// Whether `/api/v1` asks for an API key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiAuth {
    /// A key of `api_keys` is required.
    #[default]
    Enabled,
    /// Anyone may call it (development only).
    Disabled,
}

/// A secret: read from the configuration, never printed.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            f.write_str("\"\"")
        } else {
            f.write_str("\"***\"")
        }
    }
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
    /// Stockfish binary, for comparisons with the engine on the Play page
    /// (none if it cannot be run).
    pub stockfish: PathBuf,
    /// Base URL of the training control daemon (`chess-train serve`), e.g.
    /// `http://trainer:4000`; training cannot be started from the UI without it.
    pub trainer_url: Option<String>,
    /// Authentication of the NNUE lab's API (`CHESS_API_AUTH`).
    pub api_auth: ApiAuth,
    /// API keys, comma-separated (`CHESS_API_KEYS`).
    pub api_keys: Secret,
    /// Requests per minute per API key, 0 for no limit (`CHESS_API_RATE_LIMIT`).
    pub api_rate_limit: u32,
    /// The lab's database and default file locations (`CHESS_LAB_DIR`).
    pub lab_dir: PathBuf,
    /// Datasets (default `<lab_dir>/datasets`; `CHESS_LAB_DATASETS_DIR`).
    pub lab_datasets_dir: Option<PathBuf>,
    /// Model files (default `<lab_dir>/models`; `CHESS_LAB_MODELS_DIR`).
    pub lab_models_dir: Option<PathBuf>,
    /// Checkpoints of training runs (default `<lab_dir>/runs`; `CHESS_LAB_RUNS_DIR`).
    pub lab_runs_dir: Option<PathBuf>,
    /// Largest dataset upload, in MiB (`CHESS_LAB_MAX_UPLOAD_MB`).
    pub lab_max_upload_mb: u64,
    /// Token of the lab's workers; without one the internal API is off
    /// (`CHESS_LAB_WORKER_TOKEN`).
    pub lab_worker_token: Secret,
}

impl ServerConfig {
    /// Where the lab's files are.
    #[must_use]
    pub fn lab_paths(&self) -> chess_jobs::LabPaths {
        let defaults = chess_jobs::LabPaths::under(&self.lab_dir);
        chess_jobs::LabPaths {
            datasets: self.lab_datasets_dir.clone().unwrap_or(defaults.datasets),
            models: self.lab_models_dir.clone().unwrap_or(defaults.models),
            runs: self.lab_runs_dir.clone().unwrap_or(defaults.runs),
        }
    }

    /// Settings of the lab's API.
    #[must_use]
    pub fn api_config(&self) -> chess_nnue_api::ApiConfig {
        chess_nnue_api::ApiConfig {
            auth: match self.api_auth {
                ApiAuth::Disabled => chess_nnue_api::Auth::Disabled,
                ApiAuth::Enabled => chess_nnue_api::Auth::Keys(
                    self.api_keys
                        .0
                        .split(',')
                        .map(str::trim)
                        .filter(|key| !key.is_empty())
                        .map(str::to_owned)
                        .collect(),
                ),
            },
            rate_limit: self.api_rate_limit,
            max_upload_bytes: self.lab_max_upload_mb.saturating_mul(1 << 20),
            worker_token: Some(self.lab_worker_token.0.trim().to_owned())
                .filter(|token| !token.is_empty()),
        }
    }
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
            stockfish: PathBuf::from("stockfish"),
            trainer_url: None,
            api_auth: ApiAuth::Enabled,
            api_keys: Secret::default(),
            api_rate_limit: 600,
            lab_dir: PathBuf::from("data/lab"),
            lab_datasets_dir: None,
            lab_models_dir: None,
            lab_runs_dir: None,
            lab_max_upload_mb: 4096,
            lab_worker_token: Secret::default(),
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
            .merge(
                Env::prefixed("CHESS_")
                    .filter(|key| key.starts_with("api_") || key.starts_with("lab_")),
            )
            .merge(Env::prefixed("CHESS_SERVER_"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::result_large_err)] // The closure signature is imposed by figment's Jail.
    fn defaults_apply_without_file() {
        // In a jail too: the other test sets variables of the process meanwhile.
        figment::Jail::expect_with(|_| {
            let config: ServerConfig = ServerConfig::figment("does-not-exist.toml").extract()?;
            assert_eq!(config.bind.port(), 3000);
            Ok(())
        });
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

    #[test]
    #[allow(clippy::result_large_err)] // The closure signature is imposed by figment's Jail.
    fn lab_settings_from_dotenv_names_and_secrets_stay_hidden() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("CHESS_API_KEYS", "alpha, beta,");
            jail.set_env("CHESS_API_AUTH", "disabled");
            jail.set_env("CHESS_LAB_DIR", "/lab");
            jail.set_env("CHESS_LAB_MODELS_DIR", "/data/models");
            jail.set_env("CHESS_LAB_WORKER_TOKEN", "s3cret");
            let config: ServerConfig = ServerConfig::figment("none.toml").extract()?;
            assert_eq!(config.api_auth, ApiAuth::Disabled);
            let paths = config.lab_paths();
            assert_eq!(paths.datasets, PathBuf::from("/lab/datasets"));
            assert_eq!(paths.models, PathBuf::from("/data/models"));
            let api = ServerConfig {
                api_auth: ApiAuth::Enabled,
                ..config.clone()
            }
            .api_config();
            assert_eq!(
                api.auth,
                chess_nnue_api::Auth::Keys(vec!["alpha".into(), "beta".into()])
            );
            assert_eq!(api.worker_token.as_deref(), Some("s3cret"));
            let printed = format!("{config:?}");
            assert!(
                !printed.contains("alpha") && !printed.contains("s3cret"),
                "{printed}"
            );
            Ok(())
        });
    }
}
