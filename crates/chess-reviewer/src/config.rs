//! Configuration of the reviews: built-in defaults, then the TOML file
//! (`config/review.toml`), then `CHESS_REVIEW_*` environment variables
//! (highest priority; `__` separates sections, e.g.
//! `CHESS_REVIEW_CHESSCOM__MAX_RETRIES=3`).

use std::path::PathBuf;
use std::time::Duration;

use chess_chesscom::ClientConfig;
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
    /// Chess.com API client.
    pub chesscom: ChessComConfig,
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
            chesscom: ChessComConfig::default(),
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
    fn defaults_have_no_user_agent() {
        let config: ReviewConfig = ReviewConfig::figment("does-not-exist.toml")
            .extract()
            .unwrap();
        assert!(config.user_agent.is_empty());
        assert_eq!(config.chesscom.max_retries, 5);
        assert_eq!(
            config.client_config().min_interval,
            Duration::from_millis(200)
        );
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
            let config: ReviewConfig = ReviewConfig::figment("review.toml").extract()?;
            assert_eq!(config.user_agent, "app/1 (me@example.com)");
            assert_eq!(config.chesscom.max_retries, 7);
            assert_eq!(config.recent_ttl(), 60);
            Ok(())
        });
    }
}
