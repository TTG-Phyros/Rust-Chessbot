//! Runs searches off the async runtime, a bounded number at a time, with
//! the NNUE when a network file is available (reloaded whenever training
//! exports a new one) and the classical evaluation otherwise. A request may
//! instead pick a saved bot (its own network) or PeSTO.

use std::collections::HashMap;
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
/// Most lines a request may ask for.
pub(crate) const MAX_MULTIPV: u32 = 5;
/// Longest clock accepted (a day).
const MAX_CLOCK: Duration = Duration::from_secs(24 * 3600);
/// Kept in reserve on the clock for the round trip to the browser.
const CLOCK_OVERHEAD: Duration = Duration::from_millis(50);
/// Thinking time when the request sets no limit.
const DEFAULT_MOVETIME: Duration = Duration::from_secs(1);
/// Longest "infinite" analysis.
pub(crate) const MAX_ANALYSIS_TIME: Duration = Duration::from_secs(600);
/// Name of the classical evaluation, as a bot.
pub(crate) const CLASSIC: &str = "classic";

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
    /// Where saved bots keep their network (`<name>.nnue`).
    bots_dir: Option<PathBuf>,
    /// Networks of the bots played so far, with their modification time.
    bots: Arc<Mutex<HashMap<String, (SystemTime, EngineEval)>>>,
    /// Stockfish binary and its `id name`, when it runs.
    stockfish: Option<(PathBuf, String)>,
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
            bots_dir: None,
            bots: Arc::new(Mutex::new(HashMap::new())),
            stockfish: None,
        }
    }

    /// Lets requests play the bots saved in `dir`.
    #[must_use]
    pub fn with_bots(mut self, dir: PathBuf) -> Self {
        self.bots_dir = Some(dir);
        self
    }

    /// Uses the Stockfish at `binary` for comparisons, if it runs.
    #[must_use]
    pub fn with_stockfish(mut self, binary: PathBuf) -> Self {
        self.stockfish = match chess_stockfish::probe(&binary) {
            Some(version) => {
                tracing::info!(%version, "Stockfish found: comparisons available");
                Some((binary, version))
            }
            None => {
                tracing::info!(binary = %binary.display(), "Stockfish not found: no comparisons");
                None
            }
        };
        self
    }

    /// The Stockfish binary and its version, if there is one.
    pub(crate) fn stockfish(&self) -> Option<&(PathBuf, String)> {
        self.stockfish.as_ref()
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

    /// The evaluation of `bot`: the configured network when `None`, PeSTO
    /// for `classic`, otherwise the network of that saved bot (cached,
    /// reloaded if the file changes).
    ///
    /// # Errors
    ///
    /// 422 for an invalid name, 404 for a bot that does not exist.
    pub(crate) fn evaluator_for(&self, bot: Option<&str>) -> Result<EngineEval, ApiError> {
        let Some(name) = bot.filter(|name| !name.is_empty()) else {
            return Ok(self.evaluator());
        };
        if name.eq_ignore_ascii_case(CLASSIC) {
            return Ok(EngineEval::default());
        }
        crate::api::check_name(name)?;
        let missing = || ApiError::not_found(format!("no bot named {name}"));
        let path = self
            .bots_dir
            .as_ref()
            .ok_or_else(missing)?
            .join(format!("{name}.nnue"));
        let modified = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .map_err(|_| missing())?;
        let mut bots = self.bots.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((loaded, eval)) = bots.get(name)
            && *loaded == modified
        {
            return Ok(eval.clone());
        }
        let eval = std::fs::read(&path)
            .map_err(|err| ApiError::internal(format!("cannot read {}: {err}", path.display())))
            .and_then(|bytes| {
                EngineEval::from_nnue_bytes(&bytes)
                    .map_err(|err| ApiError::internal(format!("bot {name}: {err}")))
            })?;
        tracing::info!(bot = name, "bot network loaded");
        bots.insert(name.to_owned(), (modified, eval.clone()));
        Ok(eval)
    }

    /// Which evaluation searches use by default.
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
            stockfish: self.stockfish.as_ref().map(|(_, version)| version.clone()),
        }
    }

    /// Searches `position` with `bot` (see [`Self::evaluator_for`]) on a
    /// blocking thread; `on_info` is called after each iteration (from that
    /// thread).
    pub(crate) async fn search(
        &self,
        bot: Option<&str>,
        position: Position,
        limits: SearchLimits,
        stop: StopFlag,
        on_info: impl FnMut(&SearchInfo) + Send + 'static,
    ) -> Result<SearchResult, ApiError> {
        let eval = self.evaluator_for(bot)?;
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .map_err(|_| ApiError::unavailable("the engine is shutting down"))?;
        let hash_mb = self.hash_mb;
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
    let clock = search.white_time.is_some() || search.black_time.is_some();
    if search.depth.is_none() && search.movetime.is_none() && search.nodes.is_none() && !clock {
        search.movetime = Some(DEFAULT_MOVETIME);
    }
    search
}

/// Search limits for an analysis: the requested ones (clocks ignored),
/// capped, or until stopped (at most [`MAX_ANALYSIS_TIME`]).
pub(crate) fn analysis_limits(limits: EngineLimits) -> SearchLimits {
    let mut search = SearchLimits {
        white_time: None,
        black_time: None,
        white_increment: None,
        black_increment: None,
        move_overhead: Duration::ZERO,
        ..capped(limits)
    };
    if search.movetime.is_none() {
        search.movetime = Some(MAX_ANALYSIS_TIME);
    }
    search
}

fn capped(limits: EngineLimits) -> SearchLimits {
    let clock = |ms: Option<u64>| ms.map(|ms| Duration::from_millis(ms).min(MAX_CLOCK));
    SearchLimits {
        depth: limits.depth.map(|depth| depth.clamp(1, MAX_DEPTH)),
        movetime: limits
            .movetime_ms
            .map(|ms| Duration::from_millis(ms.max(1)).min(MAX_MOVETIME)),
        nodes: limits.nodes.map(|nodes| nodes.max(1)),
        multipv: limits.multipv.unwrap_or(1).clamp(1, MAX_MULTIPV) as usize,
        white_time: clock(limits.white_time_ms),
        black_time: clock(limits.black_time_ms),
        white_increment: clock(limits.white_increment_ms),
        black_increment: clock(limits.black_increment_ms),
        move_overhead: CLOCK_OVERHEAD,
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
        assert_eq!(none.multipv, 1);
        let huge = move_limits(EngineLimits {
            depth: Some(1000),
            movetime_ms: Some(10_000_000),
            multipv: Some(40),
            ..EngineLimits::default()
        });
        assert_eq!(huge.depth, Some(MAX_DEPTH));
        assert_eq!(huge.movetime, Some(MAX_MOVETIME));
        assert_eq!(huge.multipv, MAX_MULTIPV as usize);
        let depth_only = move_limits(EngineLimits {
            depth: Some(6),
            ..EngineLimits::default()
        });
        assert_eq!(depth_only.movetime, None);
        let analysis = analysis_limits(EngineLimits::default());
        assert_eq!(analysis.movetime, Some(MAX_ANALYSIS_TIME));
    }

    #[test]
    fn clocks_replace_the_default_time() {
        let limits = move_limits(EngineLimits {
            white_time_ms: Some(180_000),
            black_time_ms: Some(175_000),
            white_increment_ms: Some(2_000),
            black_increment_ms: Some(2_000),
            ..EngineLimits::default()
        });
        assert_eq!(limits.movetime, None);
        assert_eq!(limits.white_time, Some(Duration::from_secs(180)));
        assert_eq!(limits.black_increment, Some(Duration::from_secs(2)));
        assert!(limits.is_timed());
        // An analysis has no clock.
        let analysis = analysis_limits(EngineLimits {
            white_time_ms: Some(1_000),
            ..EngineLimits::default()
        });
        assert_eq!(analysis.white_time, None);
    }

    #[test]
    fn bots_are_looked_up_by_name() {
        let service = EngineService::default().with_bots(std::env::temp_dir().join("no-bots-here"));
        assert!(service.evaluator_for(None).is_ok());
        assert_eq!(
            service
                .evaluator_for(Some("classic"))
                .map(|eval| eval.name())
                .ok(),
            Some("classic")
        );
        assert!(service.evaluator_for(Some("../etc")).is_err());
        assert!(service.evaluator_for(Some("ghost")).is_err());
    }
}
