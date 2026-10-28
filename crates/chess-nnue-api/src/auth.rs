//! API keys and rate limiting of `/api/v1`.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};

use crate::problem::ApiProblem;
use crate::{Auth, NnueApi};

/// Header carrying the API key (`Authorization: Bearer <key>` works too).
pub(crate) const API_KEY_HEADER: &str = "x-api-key";

/// Who sent a request: `key-<n>` (the n-th configured key) or `anonymous`
/// when authentication is disabled. Scopes rate limits and idempotency keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Principal(pub(crate) String);

/// The key a request presents.
fn presented_key(headers: &HeaderMap) -> Option<&str> {
    if let Some(key) = headers.get(API_KEY_HEADER) {
        return key.to_str().ok();
    }
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Equality that takes the same time wherever the strings differ (and
/// whatever their lengths: digests are compared).
pub(crate) fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (Sha256::digest(a.as_bytes()), Sha256::digest(b.as_bytes()));
    a.iter().zip(b.iter()).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Checks the API key and the rate limit, then records the principal.
pub(crate) async fn layer(
    State(api): State<NnueApi>,
    mut request: Request,
    next: Next,
) -> Response {
    let principal = match &api.config.auth {
        Auth::Disabled => "anonymous".to_owned(),
        Auth::Keys(keys) => {
            let presented = presented_key(request.headers());
            // Every key is compared: no early exit on the right one.
            let matched = presented.and_then(|presented| {
                keys.iter().enumerate().fold(None, |found, (i, key)| {
                    if same_secret(key, presented) {
                        Some(i)
                    } else {
                        found
                    }
                })
            });
            match matched {
                Some(index) => format!("key-{index}"),
                None => {
                    let detail = if presented.is_some() {
                        "invalid API key"
                    } else {
                        "send an API key in the X-API-Key header"
                    };
                    let mut response = ApiProblem::new(
                        StatusCode::UNAUTHORIZED,
                        "unauthorized",
                        "Authentication required",
                    )
                    .detail(detail)
                    .into_response();
                    response.headers_mut().insert(
                        header::WWW_AUTHENTICATE,
                        HeaderValue::from_static("ApiKey header=\"X-API-Key\""),
                    );
                    return response;
                }
            }
        }
    };
    if let Err(wait) = api.limiter.take(&principal) {
        let mut problem = ApiProblem::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate-limited",
            "Too many requests",
        )
        .detail(format!(
            "at most {} requests per minute: retry in {wait} s",
            api.config.rate_limit
        ));
        problem.retry_after = Some(wait);
        return problem.into_response();
    }
    request.extensions_mut().insert(Principal(principal));
    next.run(request).await
}

/// Token buckets, one per principal: `per_minute` requests in a burst, then
/// `per_minute / 60` per second.
#[derive(Debug)]
pub(crate) struct RateLimiter {
    per_minute: u32,
    buckets: Mutex<HashMap<String, (f64, Instant)>>,
}

impl RateLimiter {
    pub(crate) fn new(per_minute: u32) -> Self {
        Self {
            per_minute,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Takes a token for `principal`, or says how many seconds until one.
    pub(crate) fn take(&self, principal: &str) -> Result<(), u64> {
        self.take_at(principal, Instant::now())
    }

    fn take_at(&self, principal: &str, now: Instant) -> Result<(), u64> {
        if self.per_minute == 0 {
            return Ok(());
        }
        let capacity = f64::from(self.per_minute);
        let per_second = capacity / 60.0;
        let mut buckets = self.buckets.lock().unwrap_or_else(PoisonError::into_inner);
        let (tokens, last) = buckets
            .entry(principal.to_owned())
            .or_insert((capacity, now));
        *tokens = (*tokens + now.saturating_duration_since(*last).as_secs_f64() * per_second)
            .min(capacity);
        *last = now;
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            Ok(())
        } else {
            Err(((1.0 - *tokens) / per_second).ceil().max(1.0) as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn secrets_compare_by_value() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "abcd"));
    }

    #[test]
    fn buckets_refill_over_time() {
        let limiter = RateLimiter::new(60);
        let start = Instant::now();
        for _ in 0..60 {
            assert_eq!(limiter.take_at("a", start), Ok(()));
        }
        assert_eq!(limiter.take_at("a", start), Err(1));
        // Another principal has its own bucket.
        assert_eq!(limiter.take_at("b", start), Ok(()));
        // One token per second.
        assert_eq!(limiter.take_at("a", start + Duration::from_secs(1)), Ok(()));
        assert!(
            limiter
                .take_at("a", start + Duration::from_secs(1))
                .is_err()
        );
        assert_eq!(RateLimiter::new(0).take_at("a", start), Ok(()));
    }
}
