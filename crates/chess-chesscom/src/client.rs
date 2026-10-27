//! HTTP client: one request at a time, retries with exponential backoff,
//! conditional requests.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{
    ETAG, HeaderMap, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, RETRY_AFTER,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::model::{ApiGame, ArchiveList, Month, MonthlyGames};

/// Settings of a [`Client`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// Root of the API.
    pub base_url: String,
    /// `User-Agent` of every request: the application and a way to contact
    /// its author, as Chess.com asks (e.g. `my-app/1.0 (me@example.com)`).
    pub user_agent: String,
    /// Minimal time between the starts of two requests.
    pub min_interval: Duration,
    /// Attempts after the first one for rate-limited or failed requests.
    pub max_retries: u32,
    /// Wait before the first retry; doubles at each retry.
    pub initial_backoff: Duration,
    /// Longest wait between two attempts.
    pub max_backoff: Duration,
    /// Time limit of one attempt.
    pub timeout: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.chess.com/pub".into(),
            user_agent: String::new(),
            min_interval: Duration::from_millis(200),
            max_retries: 5,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            timeout: Duration::from_secs(30),
        }
    }
}

/// What can go wrong with the API.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No `User-Agent` was configured.
    #[error("no User-Agent configured for the Chess.com API (set CHESS_REVIEW_USER_AGENT)")]
    MissingUserAgent,
    /// Not a possible Chess.com username.
    #[error("invalid Chess.com username {0:?}")]
    InvalidUsername(String),
    /// Chess.com knows no such player.
    #[error("no Chess.com player named {0:?}")]
    PlayerNotFound(String),
    /// An unexpected HTTP status, after the retries.
    #[error("Chess.com answered {status} for {url}")]
    Status {
        /// The status.
        status: u16,
        /// The URL requested.
        url: String,
    },
    /// The request failed (network, TLS, time limit), after the retries.
    #[error("cannot reach Chess.com: {0}")]
    Network(#[source] reqwest::Error),
    /// The response is not what the API documents.
    #[error("unexpected response from {url}: {message}")]
    Decode {
        /// The URL requested.
        url: String,
        /// Why it could not be read.
        message: String,
    },
}

/// Cache validators of a response: sent back with the next request for the
/// same URL, they let the API answer `304 Not Modified`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validators {
    /// `ETag` header.
    pub etag: Option<String>,
    /// `Last-Modified` header.
    pub last_modified: Option<String>,
}

/// Result of a conditional request.
#[derive(Debug, Clone, PartialEq)]
pub enum Fetched<T> {
    /// A new or changed resource, with its validators.
    Fresh(T, Validators),
    /// The cached copy is still current.
    NotModified,
}

impl<T> Fetched<T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> Fetched<U> {
        match self {
            Self::Fresh(value, validators) => Fetched::Fresh(f(value), validators),
            Self::NotModified => Fetched::NotModified,
        }
    }
}

/// Client of the PubAPI. Cheap to share behind an `Arc`: every request,
/// whoever sends it, waits for the previous one to finish.
#[derive(Debug)]
pub struct Client {
    http: reqwest::Client,
    config: ClientConfig,
    /// Start of the last request; held for the whole of each request so
    /// that they run one after the other.
    gate: Mutex<Option<Instant>>,
}

impl Client {
    /// A client with these settings.
    ///
    /// # Errors
    ///
    /// [`Error::MissingUserAgent`] when the `User-Agent` is blank, or
    /// [`Error::Network`] when the HTTP client cannot be built.
    pub fn new(config: ClientConfig) -> Result<Self, Error> {
        if config.user_agent.trim().is_empty() {
            return Err(Error::MissingUserAgent);
        }
        let http = reqwest::Client::builder()
            .user_agent(config.user_agent.trim())
            .timeout(config.timeout)
            .gzip(true)
            .build()
            .map_err(Error::Network)?;
        Ok(Self {
            http,
            config,
            gate: Mutex::new(None),
        })
    }

    /// Months for which `username` has archived games, oldest first.
    ///
    /// # Errors
    ///
    /// [`Error::PlayerNotFound`] for an unknown player; network, status and
    /// decoding errors otherwise.
    pub async fn archives(
        &self,
        username: &str,
        validators: &Validators,
    ) -> Result<Fetched<Vec<Month>>, Error> {
        let url = format!("{}/games/archives", self.player_url(username)?);
        let fetched = self
            .get_json::<ArchiveList>(&url, validators)
            .await
            .map_err(|err| match err {
                Error::Status { status: 404, .. } => Error::PlayerNotFound(username.to_owned()),
                err => err,
            })?;
        Ok(fetched.map(|list| {
            list.archives
                .iter()
                .filter_map(|url| Month::from_archive_url(url))
                .collect()
        }))
    }

    /// Games of `username` that ended in `month`.
    ///
    /// # Errors
    ///
    /// Network, status and decoding errors.
    pub async fn month(
        &self,
        username: &str,
        month: Month,
        validators: &Validators,
    ) -> Result<Fetched<Vec<ApiGame>>, Error> {
        let url = format!(
            "{}/games/{:04}/{:02}",
            self.player_url(username)?,
            month.year,
            month.month
        );
        let fetched = self.get_json::<MonthlyGames>(&url, validators).await?;
        Ok(fetched.map(|monthly| monthly.games))
    }

    fn player_url(&self, username: &str) -> Result<String, Error> {
        if !crate::is_valid_username(username) {
            return Err(Error::InvalidUsername(username.to_owned()));
        }
        Ok(format!(
            "{}/player/{}",
            self.config.base_url.trim_end_matches('/'),
            username.to_ascii_lowercase()
        ))
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        validators: &Validators,
    ) -> Result<Fetched<T>, Error> {
        match self.get(url, validators).await? {
            Fetched::Fresh(body, validators) => {
                let value = serde_json::from_slice(&body).map_err(|err| Error::Decode {
                    url: url.to_owned(),
                    message: err.to_string(),
                })?;
                Ok(Fetched::Fresh(value, validators))
            }
            Fetched::NotModified => Ok(Fetched::NotModified),
        }
    }

    /// GET with retries; the body of a `200`, or `NotModified` for a `304`.
    async fn get(&self, url: &str, validators: &Validators) -> Result<Fetched<Vec<u8>>, Error> {
        let mut last_start = self.gate.lock().await;
        let mut attempt = 0;
        loop {
            if let Some(start) = *last_start {
                tokio::time::sleep_until(start + self.config.min_interval).await;
            }
            *last_start = Some(Instant::now());

            let mut request = self.http.get(url);
            if let Some(etag) = &validators.etag {
                request = request.header(IF_NONE_MATCH, etag);
            }
            if let Some(date) = &validators.last_modified {
                request = request.header(IF_MODIFIED_SINCE, date);
            }
            // A failed attempt worth retrying, with the server's `Retry-After`.
            let (error, retry_after) = match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status == StatusCode::NOT_MODIFIED {
                        return Ok(Fetched::NotModified);
                    }
                    if status.is_success() {
                        let validators = validators_of(response.headers());
                        match response.bytes().await {
                            Ok(body) => return Ok(Fetched::Fresh(body.to_vec(), validators)),
                            Err(err) => (Error::Network(err), None),
                        }
                    } else if is_retryable(status) {
                        let wait = retry_after(response.headers());
                        (status_error(status, url), wait)
                    } else {
                        return Err(status_error(status, url));
                    }
                }
                Err(err) if err.is_builder() => return Err(Error::Network(err)),
                Err(err) => (Error::Network(err), None),
            };
            if attempt >= self.config.max_retries {
                return Err(error);
            }
            let delay = backoff(&self.config, attempt, retry_after, jitter());
            tracing::warn!(%url, %error, attempt, ?delay, "Chess.com request failed, retrying");
            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }
}

fn status_error(status: StatusCode, url: &str) -> Error {
    Error::Status {
        status: status.as_u16(),
        url: url.to_owned(),
    }
}

/// Rate limiting and transient server errors.
fn is_retryable(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::REQUEST_TIMEOUT
        || matches!(status.as_u16(), 500 | 502 | 503 | 504)
}

/// `Retry-After` in seconds (the HTTP-date form is not used by Chess.com).
fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let seconds: u64 = headers
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_secs(seconds))
}

fn validators_of(headers: &HeaderMap) -> Validators {
    let text = |name| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    Validators {
        etag: text(ETAG),
        last_modified: text(LAST_MODIFIED),
    }
}

/// Wait before retry number `attempt` (0-based): the server's `Retry-After`
/// if it gave one, else `initial_backoff * 2^attempt` scaled by `jitter`
/// (in `[0.5, 1]`, so that clients do not retry in lockstep); never more
/// than `max_backoff`.
fn backoff(
    config: &ClientConfig,
    attempt: u32,
    retry_after: Option<Duration>,
    jitter: f64,
) -> Duration {
    let delay = retry_after.unwrap_or_else(|| {
        config
            .initial_backoff
            .saturating_mul(1 << attempt.min(16))
            .mul_f64(jitter.clamp(0.5, 1.0))
    });
    delay.min(config.max_backoff)
}

/// A random factor in `[0.5, 1]` (from the randomly keyed std hasher).
fn jitter() -> f64 {
    let bits = RandomState::new().build_hasher().finish() >> 11;
    // 53 random bits: exact in an f64.
    #[allow(clippy::cast_precision_loss)]
    let unit = bits as f64 / (1u64 << 53) as f64;
    0.5 + unit / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_is_capped() {
        let config = ClientConfig::default();
        assert_eq!(backoff(&config, 0, None, 1.0), Duration::from_secs(1));
        assert_eq!(backoff(&config, 3, None, 1.0), Duration::from_secs(8));
        assert_eq!(backoff(&config, 3, None, 0.5), Duration::from_secs(4));
        assert_eq!(backoff(&config, 20, None, 1.0), config.max_backoff);
        assert_eq!(
            backoff(&config, 0, Some(Duration::from_secs(7)), 0.5),
            Duration::from_secs(7)
        );
        assert_eq!(
            backoff(&config, 0, Some(Duration::from_secs(3600)), 1.0),
            config.max_backoff
        );
        let j = jitter();
        assert!((0.5..=1.0).contains(&j));
    }

    #[test]
    fn retryable_statuses() {
        assert!(is_retryable(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable(StatusCode::BAD_GATEWAY));
        assert!(!is_retryable(StatusCode::NOT_FOUND));
        assert!(!is_retryable(StatusCode::GONE));
    }

    #[test]
    fn requires_a_user_agent() {
        assert!(matches!(
            Client::new(ClientConfig::default()),
            Err(Error::MissingUserAgent)
        ));
        let client = Client::new(ClientConfig {
            user_agent: "test".into(),
            ..ClientConfig::default()
        })
        .unwrap();
        assert!(matches!(
            client.player_url("bad/name"),
            Err(Error::InvalidUsername(_))
        ));
        assert_eq!(
            client.player_url("Hikaru").unwrap(),
            "https://api.chess.com/pub/player/hikaru"
        );
    }
}
