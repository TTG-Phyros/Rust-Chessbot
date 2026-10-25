//! Runs searches off the async runtime, a bounded number at a time.

use std::sync::Arc;
use std::time::Duration;

use chess_core::Position;
use chess_eval::ClassicEval;
use chess_search::{SearchInfo, SearchLimits, SearchResult, Searcher, StopFlag};
use tokio::sync::Semaphore;

use crate::api::error::ApiError;
use crate::api::types::EngineLimits;

/// Hard caps on what one request may ask for.
const MAX_DEPTH: u32 = 64;
const MAX_MOVETIME: Duration = Duration::from_secs(60);
/// Thinking time when the request sets no limit.
const DEFAULT_MOVETIME: Duration = Duration::from_secs(1);
/// Longest "infinite" analysis.
pub(crate) const MAX_ANALYSIS_TIME: Duration = Duration::from_secs(600);

/// Shared engine: each search gets its own [`Searcher`], and a semaphore
/// keeps the number of concurrent searches (each one uses a CPU core and a
/// transposition table) under control.
#[derive(Debug, Clone)]
pub struct EngineService {
    permits: Arc<Semaphore>,
    hash_mb: usize,
}

impl EngineService {
    /// At most `max_searches` searches run at once, each with a `hash_mb`
    /// MiB transposition table.
    #[must_use]
    pub fn new(max_searches: usize, hash_mb: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(max_searches.max(1))),
            hash_mb: hash_mb.max(1),
        }
    }

    /// Searches `position` on a blocking thread; `on_info` is called after
    /// each iteration (from that thread).
    pub(crate) async fn search(
        &self,
        position: Position,
        limits: SearchLimits,
        stop: StopFlag,
        on_info: impl FnMut(&SearchInfo) + Send + 'static,
    ) -> Result<SearchResult, ApiError> {
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .map_err(|_| ApiError::unavailable("the engine is shutting down"))?;
        let hash_mb = self.hash_mb;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut searcher = Searcher::new(ClassicEval, stop);
            searcher.set_hash_size(hash_mb);
            searcher.search(&position, &limits, on_info)
        })
        .await
        .map_err(|err| ApiError::internal(format!("search failed: {err}")))
    }
}

impl Default for EngineService {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
        Self::new(cores, chess_search::DEFAULT_HASH_MB)
    }
}

/// Search limits for a single engine move: the requested ones, capped, or
/// one second if none.
pub(crate) fn move_limits(limits: EngineLimits) -> SearchLimits {
    let mut search = capped(limits);
    if search.depth.is_none() && search.movetime.is_none() && search.nodes.is_none() {
        search.movetime = Some(DEFAULT_MOVETIME);
    }
    search
}

/// Search limits for an analysis: the requested ones, capped, or until
/// stopped (at most [`MAX_ANALYSIS_TIME`]).
pub(crate) fn analysis_limits(limits: EngineLimits) -> SearchLimits {
    let mut search = capped(limits);
    if search.movetime.is_none() {
        search.movetime = Some(MAX_ANALYSIS_TIME);
    }
    search
}

fn capped(limits: EngineLimits) -> SearchLimits {
    SearchLimits {
        depth: limits.depth.map(|depth| depth.clamp(1, MAX_DEPTH)),
        movetime: limits
            .movetime_ms
            .map(|ms| Duration::from_millis(ms.max(1)).min(MAX_MOVETIME)),
        nodes: limits.nodes.map(|nodes| nodes.max(1)),
        ..SearchLimits::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_capped_and_defaulted() {
        let none = move_limits(EngineLimits::default());
        assert_eq!(none.movetime, Some(DEFAULT_MOVETIME));
        let huge = move_limits(EngineLimits {
            depth: Some(1000),
            movetime_ms: Some(10_000_000),
            nodes: None,
        });
        assert_eq!(huge.depth, Some(MAX_DEPTH));
        assert_eq!(huge.movetime, Some(MAX_MOVETIME));
        let depth_only = move_limits(EngineLimits {
            depth: Some(6),
            ..EngineLimits::default()
        });
        assert_eq!(depth_only.movetime, None);
        let analysis = analysis_limits(EngineLimits::default());
        assert_eq!(analysis.movetime, Some(MAX_ANALYSIS_TIME));
    }
}
