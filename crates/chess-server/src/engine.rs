//! Runs searches off the async runtime, a bounded number at a time, with
//! the NNUE when a network file is available (reloaded whenever training
//! exports a new one) and the classical evaluation otherwise.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chess_core::Position;
use chess_eval::EngineEval;
use chess_search::{SearchInfo, SearchLimits, SearchResult, Searcher, StopFlag};
use tokio::sync::Semaphore;

use crate::api::error::ApiError;
use crate::api::types::{EngineInfo, EngineLimits};

/// Hard caps on what one request may ask for.
const MAX_DEPTH: u32 = 64;
const MAX_MOVETIME: Duration = Duration::from_secs(60);
/// Thinking time when the request sets no limit.
const DEFAULT_MOVETIME: Duration = Duration::from_secs(1);
/// Longest "infinite" analysis.
pub(crate) const MAX_ANALYSIS_TIME: Duration = Duration::from_secs(600);

/// The evaluator in use and the file it came from.
#[derive(Debug, Default)]
struct Network {
    eval: EngineEval,
    /// Modification time of the loaded file (`None`: classical evaluation).
    loaded: Option<SystemTime>,
}

/// Shared engine: each search gets its own [`Searcher`], and a semaphore
/// keeps the number of concurrent searches (each one uses a CPU core and a
/// transposition table) under control.
#[derive(Debug, Clone)]
pub struct EngineService {
    permits: Arc<Semaphore>,
    hash_mb: usize,
    eval_file: Option<PathBuf>,
    network: Arc<Mutex<Network>>,
}

impl EngineService {
    /// At most `max_searches` searches run at once, each with a `hash_mb`
    /// MiB transposition table, evaluated by the network in `eval_file`
    /// when it exists and is valid.
    #[must_use]
    pub fn new(max_searches: usize, hash_mb: usize, eval_file: Option<PathBuf>) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(max_searches.max(1))),
            hash_mb: hash_mb.max(1),
            eval_file,
            network: Arc::new(Mutex::new(Network::default())),
        }
    }

    /// The evaluator to use now: reloads the network if its file changed
    /// (one `stat` per search), keeps the previous one if the new file is
    /// unreadable (e.g. half-written), and uses the classical evaluation
    /// when there is no file.
    fn evaluator(&self) -> EngineEval {
        let mut network = self.network.lock().unwrap_or_else(PoisonError::into_inner);
        let modified = self.eval_file.as_ref().and_then(|path| {
            std::fs::metadata(path)
                .and_then(|meta| meta.modified())
                .ok()
        });
        match (modified, &self.eval_file) {
            (Some(modified), Some(path)) if network.loaded != Some(modified) => {
                match std::fs::read(path).map(|bytes| EngineEval::from_nnue_bytes(&bytes)) {
                    Ok(Ok(eval)) => {
                        tracing::info!(file = %path.display(), "network loaded");
                        *network = Network {
                            eval,
                            loaded: Some(modified),
                        };
                    }
                    Ok(Err(err)) => tracing::warn!(%err, file = %path.display(), "invalid network"),
                    Err(err) => tracing::warn!(%err, file = %path.display(), "unreadable network"),
                }
            }
            (None, _) if network.loaded.is_some() => {
                tracing::info!("network file gone: classical evaluation");
                *network = Network::default();
            }
            _ => {}
        }
        network.eval.clone()
    }

    /// Which evaluation searches use.
    pub(crate) fn info(&self) -> EngineInfo {
        let eval = self.evaluator();
        let network = self.network.lock().unwrap_or_else(PoisonError::into_inner);
        EngineInfo {
            evaluator: eval.name().to_owned(),
            weights_file: network
                .loaded
                .and(self.eval_file.as_ref())
                .map(|path| path.display().to_string()),
            weights_modified: network
                .loaded
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|age| age.as_secs()),
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
        let eval = self.evaluator();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut searcher = Searcher::new(eval, stop);
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
        Self::new(cores, chess_search::DEFAULT_HASH_MB, None)
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
