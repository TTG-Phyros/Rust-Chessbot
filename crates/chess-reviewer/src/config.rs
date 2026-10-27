//! Configuration of the reviews: built-in defaults, then the TOML file
//! (`config/review.toml`), then `CHESS_REVIEW_*` environment variables
//! (highest priority; `__` separates sections, e.g.
//! `CHESS_REVIEW_CHESSCOM__MAX_RETRIES=3`).

use std::path::PathBuf;
use std::time::Duration;

use chess_chesscom::ClientConfig;
use chess_review::{Budget, EngineSpec, SearchSettings};
use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

/// Environment variable naming an alternative config file.
pub const CONFIG_PATH_ENV: &str = "CHESS_REVIEW_CONFIG";
/// Config file read when [`CONFIG_PATH_ENV`] is unset (ignored if missing).
pub const DEFAULT_CONFIG_PATH: &str = "config/review.toml";

/// Settings of the reviews.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewConfig {
    /// `User-Agent` sent to Chess.com: the application and a contact
    /// (`chess-bot/0.1 (you@example.com)`). No default: Chess.com asks to
    /// be able to reach whoever sends the requests.
    pub user_agent: String,
    /// SQLite database caching games and analyses.
    pub cache: PathBuf,
    /// Most games one review may ask for.
    pub max_games: usize,
    /// Chess.com API client.
    pub chesscom: ChessComConfig,
    /// Engine searches.
    pub analysis: AnalysisConfig,
    /// Stockfish, the default engine.
    pub stockfish: StockfishConfig,
    /// Our own engine.
    pub native: NativeConfig,
}

/// Which engine analyses the games.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    /// Stockfish over UCI: the reference.
    #[default]
    Stockfish,
    /// Our engine (NNUE when a network exists, classical otherwise).
    Native,
}

impl std::str::FromStr for EngineKind {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "stockfish" => Ok(Self::Stockfish),
            "native" => Ok(Self::Native),
            _ => Err(format!("unknown engine {text:?} (stockfish or native)")),
        }
    }
}

/// Settings of the searches: named presets the user picks from (a request
/// may also set its own depth, time and lines).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisConfig {
    /// Engine used when the request names none.
    pub engine: EngineKind,
    /// Preset used when the request names none (one of `presets`).
    pub preset: String,
    /// The presets offered, fastest first.
    pub presets: Vec<Preset>,
    /// Games analysed at once, one engine each; 0 for half the CPU cores.
    pub workers: usize,
}

/// Named search settings, e.g. "fast": depth 12, 2 lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    /// Identifier used by requests (`fast`).
    pub name: String,
    /// Shown to the user (`Rapide`).
    pub label: String,
    /// Depth of each search, unless `movetime_ms` is set.
    #[serde(default)]
    pub depth: Option<u32>,
    /// Time of each search, in milliseconds (instead of a depth).
    #[serde(default)]
    pub movetime_ms: Option<u64>,
    /// Best lines per position (at least 2 to recognise only moves).
    pub multipv: usize,
}

impl Preset {
    /// Its search settings (depth 16 if it sets neither depth nor time).
    #[must_use]
    pub fn settings(&self) -> SearchSettings {
        SearchSettings {
            budget: match (self.depth, self.movetime_ms) {
                (Some(depth), _) => Budget::Depth(depth),
                (None, Some(ms)) => Budget::MoveTimeMs(ms),
                (None, None) => Budget::Depth(16),
            },
            multipv: self.multipv.max(1),
        }
    }
}

/// Stockfish processes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StockfishConfig {
    /// The binary (looked up on the `PATH` without a directory).
    pub binary: PathBuf,
    /// Search threads of each process.
    pub threads: usize,
    /// Hash table of each process, in MiB.
    pub hash_mb: usize,
}

/// Our engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NativeConfig {
    /// Network used when it exists (classical evaluation otherwise).
    pub eval_file: PathBuf,
    /// Transposition table of each search, in MiB.
    pub hash_mb: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        let preset = |name: &str, label: &str, depth, multipv| Preset {
            name: name.into(),
            label: label.into(),
            depth: Some(depth),
            movetime_ms: None,
            multipv,
        };
        Self {
            engine: EngineKind::Stockfish,
            preset: "deep".into(),
            presets: vec![
                preset("fast", "Rapide", 12, 2),
                preset("balanced", "Équilibré", 14, 2),
                preset("deep", "Approfondi", 16, 3),
            ],
            workers: 0,
        }
    }
}

impl Default for StockfishConfig {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("stockfish"),
            threads: 1,
            hash_mb: 64,
        }
    }
}

impl Default for NativeConfig {
    fn default() -> Self {
        Self {
            eval_file: PathBuf::from("data/weights/main.nnue"),
            hash_mb: 64,
        }
    }
}

/// Settings of the Chess.com client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChessComConfig {
    /// Root of the API.
    pub base_url: String,
    /// Minimal time between two requests, in milliseconds.
    pub min_interval_ms: u64,
    /// Retries of a rate-limited or failed request.
    pub max_retries: u32,
    /// Wait before the first retry, in milliseconds (doubles each time).
    pub initial_backoff_ms: u64,
    /// Longest wait between two attempts, in milliseconds.
    pub max_backoff_ms: u64,
    /// Time limit of one request, in seconds.
    pub timeout_secs: u64,
    /// Age, in seconds, after which the list of months and the current
    /// month are revalidated (past months are never downloaded twice).
    pub recent_ttl_secs: u64,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            user_agent: String::new(),
            cache: PathBuf::from("data/review/cache.sqlite"),
            max_games: 200,
            chesscom: ChessComConfig::default(),
            analysis: AnalysisConfig::default(),
            stockfish: StockfishConfig::default(),
            native: NativeConfig::default(),
        }
    }
}

impl Default for ChessComConfig {
    fn default() -> Self {
        let client = ClientConfig::default();
        let millis = |duration: Duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Self {
            base_url: client.base_url,
            min_interval_ms: millis(client.min_interval),
            max_retries: client.max_retries,
            initial_backoff_ms: millis(client.initial_backoff),
            max_backoff_ms: millis(client.max_backoff),
            timeout_secs: client.timeout.as_secs(),
            recent_ttl_secs: 600,
        }
    }
}

impl ReviewConfig {
    /// Loads the configuration from defaults, the TOML file and the environment.
    ///
    /// # Errors
    ///
    /// When the file or a variable holds an invalid value.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        let path = std::env::var(CONFIG_PATH_ENV).unwrap_or_else(|_| DEFAULT_CONFIG_PATH.into());
        Self::figment(&path).extract().map_err(Box::new)
    }

    fn figment(path: &str) -> Figment {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Toml::file(path))
            .merge(Env::prefixed("CHESS_REVIEW_").split("__"))
    }

    /// Settings of the Chess.com client.
    #[must_use]
    pub fn client_config(&self) -> ClientConfig {
        let chesscom = &self.chesscom;
        ClientConfig {
            base_url: chesscom.base_url.clone(),
            user_agent: self.user_agent.clone(),
            min_interval: Duration::from_millis(chesscom.min_interval_ms),
            max_retries: chesscom.max_retries,
            initial_backoff: Duration::from_millis(chesscom.initial_backoff_ms),
            max_backoff: Duration::from_millis(chesscom.max_backoff_ms),
            timeout: Duration::from_secs(chesscom.timeout_secs),
        }
    }

    /// How to start the engine `kind`.
    #[must_use]
    pub fn engine_spec(&self, kind: EngineKind) -> EngineSpec {
        match kind {
            EngineKind::Stockfish => EngineSpec::Stockfish {
                binary: self.stockfish.binary.clone(),
                threads: self.stockfish.threads.max(1),
                hash_mb: self.stockfish.hash_mb.max(1),
            },
            EngineKind::Native => EngineSpec::Native {
                eval_file: Some(self.native.eval_file.clone()),
                hash_mb: self.native.hash_mb.max(1),
            },
        }
    }

    /// The preset `name`.
    #[must_use]
    pub fn preset(&self, name: &str) -> Option<&Preset> {
        self.analysis
            .presets
            .iter()
            .find(|preset| preset.name.eq_ignore_ascii_case(name))
    }

    /// The default preset (the first one if `analysis.preset` names none).
    #[must_use]
    pub fn default_preset(&self) -> Option<&Preset> {
        self.preset(&self.analysis.preset)
            .or_else(|| self.analysis.presets.first())
    }

    /// Default search settings: the default preset's (depth 16, 3 lines
    /// without any preset).
    #[must_use]
    pub fn search_settings(&self) -> SearchSettings {
        self.default_preset()
            .map_or_else(SearchSettings::default, Preset::settings)
    }

    /// Games analysed at once.
    #[must_use]
    pub fn workers(&self) -> usize {
        match self.analysis.workers {
            0 => std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1)),
            n => n,
        }
    }

    /// Revalidation delay of recent data, in seconds.
    #[must_use]
    pub fn recent_ttl(&self) -> i64 {
        i64::try_from(self.chesscom.recent_ttl_secs).unwrap_or(i64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::result_large_err)] // The closure signature is imposed by figment's Jail.
    fn defaults_have_no_user_agent() {
        // In a jail without variables: the other test (or the developer's
        // shell) may set CHESS_REVIEW_* ones.
        figment::Jail::expect_with(|jail| {
            jail.clear_env();
            let config: ReviewConfig = ReviewConfig::figment("does-not-exist.toml").extract()?;
            assert!(config.user_agent.is_empty());
            assert_eq!(config.chesscom.max_retries, 5);
            assert_eq!(
                config.client_config().min_interval,
                Duration::from_millis(200)
            );
            Ok(())
        });
    }

    #[test]
    #[allow(clippy::result_large_err)] // The closure signature is imposed by figment's Jail.
    fn the_repository_config_is_valid() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/review.toml");
        figment::Jail::expect_with(|jail| {
            jail.clear_env();
            let config: ReviewConfig = ReviewConfig::figment(path).extract()?;
            // Same values as the built-in defaults, which the file documents.
            assert_eq!(config, ReviewConfig::default());
            assert_eq!(config.analysis.presets.len(), 3);
            Ok(())
        });
    }

    #[test]
    #[allow(clippy::result_large_err)] // The closure signature is imposed by figment's Jail.
    fn env_overrides_file() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "review.toml",
                "user_agent = \"from-file\"\n[chesscom]\nmax_retries = 2\nrecent_ttl_secs = 60",
            )?;
            jail.set_env("CHESS_REVIEW_USER_AGENT", "app/1 (me@example.com)");
            jail.set_env("CHESS_REVIEW_CHESSCOM__MAX_RETRIES", "7");
            jail.set_env("CHESS_REVIEW_ANALYSIS__ENGINE", "native");
            jail.set_env("CHESS_REVIEW_ANALYSIS__PRESET", "FAST");
            let config: ReviewConfig = ReviewConfig::figment("review.toml").extract()?;
            assert_eq!(config.user_agent, "app/1 (me@example.com)");
            assert_eq!(config.chesscom.max_retries, 7);
            assert_eq!(config.recent_ttl(), 60);
            assert_eq!(config.analysis.engine, EngineKind::Native);
            assert_eq!(config.search_settings().budget, Budget::Depth(12));
            assert_eq!(
                config.default_preset().map(|p| p.label.as_str()),
                Some("Rapide")
            );
            assert_eq!(config.preset("nope"), None);
            assert!(matches!(
                config.engine_spec(EngineKind::Stockfish),
                EngineSpec::Stockfish { threads: 1, .. }
            ));
            Ok(())
        });
    }
}
