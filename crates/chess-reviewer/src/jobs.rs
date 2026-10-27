//! Reviews of a player's games run as background jobs: fetch the games,
//! then analyse those not analysed yet on a pool of engines, publishing
//! the progress as it goes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use chess_chesscom::{Client, GameFilter, PlayedGame, is_valid_username};
use chess_review::{
    AnalysisEngine, Budget, EngineSpec, GameAnalysis, SearchSettings, analyse_game,
};
use serde::Serialize;
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinSet;

use crate::config::EngineKind;
use crate::store::analysis_key;
use crate::{Error, ReviewConfig, Store, SyncEvent, sync_player};

/// Finished jobs stay visible this long.
const RETENTION: Duration = Duration::from_secs(3600);
/// Most lines per position a request may ask for.
pub const MAX_MULTIPV: usize = 5;
/// Deepest search a request may ask for.
pub const MAX_DEPTH: u32 = 40;
/// Longest search per position a request may ask for, in milliseconds.
pub const MAX_MOVETIME_MS: u64 = 60_000;
/// Most nodes per position a request may ask for.
pub const MAX_NODES: u64 = 100_000_000;

/// What to review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRequest {
    /// Chess.com username.
    pub username: String,
    /// Which of their games.
    pub filter: GameFilter,
    /// Engine; the configured one if `None`.
    pub engine: Option<EngineKind>,
    /// Search settings; the configured ones if `None`.
    pub settings: Option<SearchSettings>,
}

/// Stage of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    /// Reading the player's archives.
    Fetching,
    /// Analysing the games.
    Analysing,
    /// Every game is analysed (or failed).
    Done,
    /// The games could not be fetched, or the engine started.
    Failed,
    /// Stopped on request.
    Cancelled,
}

impl JobState {
    /// Whether the job is over.
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

/// Progress of a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobProgress {
    /// Stage.
    pub state: JobState,
    /// Months of the archive read so far.
    pub months_done: usize,
    /// Months that may be read.
    pub months_total: usize,
    /// Games of the review, most recent first (known once fetched).
    pub games: Vec<String>,
    /// Games already analysed with these settings (from the cache).
    pub games_cached: usize,
    /// Games analysed by this job.
    pub games_done: usize,
    /// Games that could not be analysed (unreadable PGN, engine crash).
    pub games_failed: usize,
    /// Positions analysed so far, among those of the games to analyse.
    pub positions_done: usize,
    /// Positions of the games to analyse.
    pub positions_total: usize,
    /// Engine analysing the games (its id).
    pub engine: Option<String>,
    /// Why the job failed.
    pub error: Option<String>,
}

impl JobProgress {
    fn new() -> Self {
        Self {
            state: JobState::Fetching,
            months_done: 0,
            months_total: 0,
            games: Vec::new(),
            games_cached: 0,
            games_done: 0,
            games_failed: 0,
            positions_done: 0,
            positions_total: 0,
            engine: None,
            error: None,
        }
    }
}

/// A running or finished review.
#[derive(Debug)]
pub struct Job {
    /// Identifier.
    pub id: u64,
    /// Player reviewed.
    pub username: String,
    cancelled: AtomicBool,
    progress: watch::Sender<JobProgress>,
    finished: Mutex<Option<Instant>>,
}

impl Job {
    /// Current progress.
    #[must_use]
    pub fn progress(&self) -> JobProgress {
        self.progress.borrow().clone()
    }

    /// Progress updates, starting with the current one.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<JobProgress> {
        self.progress.subscribe()
    }

    /// Asks the job to stop (analyses in progress stop at their next position).
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn update(&self, change: impl FnOnce(&mut JobProgress)) {
        self.progress.send_modify(change);
    }

    fn finish(&self, state: JobState, error: Option<String>) {
        self.update(|progress| {
            progress.state = state;
            progress.error = error;
        });
        *self.finished.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }

    fn finished_since(&self) -> Option<Duration> {
        self.finished
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .map(|at| at.elapsed())
    }
}

/// Engines of one kind, kept between games (a Stockfish process takes a
/// moment to start).
struct EnginePool {
    spec: EngineSpec,
    idle: Mutex<Vec<Box<dyn AnalysisEngine>>>,
    /// Our engine is started afresh for each game, so that a new network
    /// is picked up as soon as training exports it.
    reuse: bool,
}

impl EnginePool {
    fn take(&self) -> anyhow::Result<Box<dyn AnalysisEngine>> {
        let idle = self
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        idle.map_or_else(|| self.spec.spawn(), Ok)
    }

    fn give_back(&self, engine: Box<dyn AnalysisEngine>) {
        if self.reuse {
            self.idle
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(engine);
        }
    }
}

/// Runs the reviews: one Chess.com client, the cache, a bounded number of
/// engines working at once (shared by all jobs).
pub struct ReviewService {
    config: ReviewConfig,
    client: Option<Client>,
    store: Store,
    workers: Arc<Semaphore>,
    pools: HashMap<EngineKind, Arc<EnginePool>>,
    jobs: Mutex<HashMap<u64, Arc<Job>>>,
    next_id: AtomicU64,
}

impl std::fmt::Debug for ReviewService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReviewService")
            .field("cache", &self.config.cache)
            .field("chesscom", &self.client.is_some())
            .field("workers", &self.config.workers())
            .finish_non_exhaustive()
    }
}

impl ReviewService {
    /// Opens the cache of `config`. Without a `User-Agent`, Chess.com is
    /// not contacted (reviews cannot start; cached ones stay readable).
    ///
    /// # Errors
    ///
    /// When the cache cannot be opened.
    pub fn new(config: ReviewConfig) -> Result<Self, Error> {
        let store = Store::open(&config.cache)?;
        Self::with_store(config, store)
    }

    /// Same as [`ReviewService::new`] with an already opened cache.
    ///
    /// # Errors
    ///
    /// When the Chess.com client cannot be built.
    pub fn with_store(config: ReviewConfig, store: Store) -> Result<Self, Error> {
        let client = if config.user_agent.trim().is_empty() {
            tracing::warn!("no CHESS_REVIEW_USER_AGENT: Chess.com reviews are disabled");
            None
        } else {
            Some(Client::new(config.client_config())?)
        };
        let pools = [EngineKind::Stockfish, EngineKind::Native]
            .into_iter()
            .map(|kind| {
                let pool = EnginePool {
                    spec: config.engine_spec(kind),
                    idle: Mutex::new(Vec::new()),
                    reuse: kind == EngineKind::Stockfish,
                };
                (kind, Arc::new(pool))
            })
            .collect();
        Ok(Self {
            workers: Arc::new(Semaphore::new(config.workers())),
            client,
            store,
            pools,
            config,
            jobs: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        })
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &ReviewConfig {
        &self.config
    }

    /// The cache.
    #[must_use]
    pub const fn store(&self) -> &Store {
        &self.store
    }

    /// Whether Chess.com can be reached (a `User-Agent` is configured).
    #[must_use]
    pub const fn chesscom_enabled(&self) -> bool {
        self.client.is_some()
    }

    /// The job `id`, while it is kept.
    #[must_use]
    pub fn job(&self, id: u64) -> Option<Arc<Job>> {
        self.jobs().get(&id).cloned()
    }

    fn jobs(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Arc<Job>>> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Checks `request` and starts reviewing in the background.
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] without a `User-Agent`, [`Error::Invalid`]
    /// for a request out of bounds.
    pub fn start(self: &Arc<Self>, request: JobRequest) -> Result<Arc<Job>, Error> {
        if self.client.is_none() {
            return Err(Error::Unavailable(
                "Chess.com reviews need a User-Agent: set CHESS_REVIEW_USER_AGENT".into(),
            ));
        }
        if !is_valid_username(&request.username) {
            return Err(Error::Invalid(format!(
                "invalid Chess.com username {:?}",
                request.username
            )));
        }
        let max = self.config.max_games;
        if !(1..=max).contains(&request.filter.max_games) {
            return Err(Error::Invalid(format!("ask for 1 to {max} games")));
        }
        let settings = request
            .settings
            .unwrap_or_else(|| self.config.search_settings());
        check_settings(&settings)?;

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let job = Arc::new(Job {
            id,
            username: request.username.clone(),
            cancelled: AtomicBool::new(false),
            progress: watch::Sender::new(JobProgress::new()),
            finished: Mutex::new(None),
        });
        {
            let mut jobs = self.jobs();
            jobs.retain(|_, job| job.finished_since().is_none_or(|age| age < RETENTION));
            jobs.insert(id, Arc::clone(&job));
        }
        let kind = request.engine.unwrap_or(self.config.analysis.engine);
        let service = Arc::clone(self);
        let running = Arc::clone(&job);
        tokio::spawn(async move {
            match service.run(&running, &request, kind, settings).await {
                Ok(()) if running.is_cancelled() => running.finish(JobState::Cancelled, None),
                Ok(()) => running.finish(JobState::Done, None),
                Err(err) => {
                    tracing::warn!(job = running.id, %err, "review failed");
                    running.finish(JobState::Failed, Some(err.to_string()));
                }
            }
        });
        Ok(job)
    }

    async fn run(
        self: &Arc<Self>,
        job: &Arc<Job>,
        request: &JobRequest,
        kind: EngineKind,
        settings: SearchSettings,
    ) -> Result<(), Error> {
        let Some(client) = &self.client else {
            return Ok(());
        };
        let games = sync_player(
            client,
            &self.store,
            &request.username,
            &request.filter,
            self.config.recent_ttl(),
            |event| match event {
                SyncEvent::Months { total, .. } => job.update(|p| p.months_total = total),
                SyncEvent::Month { index, .. } => job.update(|p| p.months_done = index),
            },
        )
        .await?;
        job.update(|p| {
            p.games = games.iter().map(|game| game.id().to_owned()).collect();
            p.state = JobState::Analysing;
        });
        if games.is_empty() || job.is_cancelled() {
            return Ok(());
        }

        let pool = Arc::clone(&self.pools[&kind]);
        let engine = engine_id(&pool).await?;
        let key = analysis_key(&engine, &settings);
        job.update(|p| p.engine = Some(engine));

        let mut pending = Vec::new();
        for api in &games {
            if self.store.analysis(api.id(), &key)?.is_some() {
                job.update(|p| p.games_cached += 1);
                continue;
            }
            match PlayedGame::from_api(api) {
                Ok(game) => pending.push(game),
                Err(err) => {
                    tracing::warn!(game = api.id(), %err, "unreadable game");
                    job.update(|p| p.games_failed += 1);
                }
            }
        }
        let positions: usize = pending.iter().map(|g| g.game.moves().len() + 1).sum();
        job.update(|p| p.positions_total = positions);

        let mut tasks = JoinSet::new();
        for game in pending {
            let (service, job, pool) = (Arc::clone(self), Arc::clone(job), Arc::clone(&pool));
            tasks.spawn(async move {
                let result = service.analyse(&job, &pool, game.clone(), settings).await;
                (game.id, result)
            });
        }
        while let Some(joined) = tasks.join_next().await {
            let outcome = match joined {
                Ok((id, Ok(Some(analysis)))) => self
                    .store
                    .save_analysis(&id, &analysis, crate::now())
                    .map_err(|err| err.to_string()),
                Ok((_, Ok(None))) => continue,
                Ok((_, Err(err))) => Err(format!("{err:#}")),
                Err(err) => Err(err.to_string()),
            };
            match outcome {
                Ok(()) => job.update(|p| p.games_done += 1),
                Err(err) => {
                    tracing::warn!(job = job.id, %err, "analysis failed");
                    job.update(|p| p.games_failed += 1);
                }
            }
        }
        Ok(())
    }

    /// Analyses `game` once a worker is free; `None` when cancelled.
    async fn analyse(
        &self,
        job: &Arc<Job>,
        pool: &Arc<EnginePool>,
        game: PlayedGame,
        settings: SearchSettings,
    ) -> anyhow::Result<Option<GameAnalysis>> {
        let _permit = Arc::clone(&self.workers).acquire_owned().await?;
        if job.is_cancelled() {
            return Ok(None);
        }
        let (job, pool) = (Arc::clone(job), Arc::clone(pool));
        tokio::task::spawn_blocking(move || {
            let mut engine = pool.take()?;
            let analysis = analyse_game(
                engine.as_mut(),
                game.game.start(),
                game.game.moves(),
                &settings,
                |_, _| {
                    job.update(|p| p.positions_done += 1);
                    !job.is_cancelled()
                },
            )?;
            // A failed engine is dropped (the process is killed).
            pool.give_back(engine);
            Ok(analysis)
        })
        .await?
    }
}

/// Identity of the engines of `pool`, read from one of them.
async fn engine_id(pool: &Arc<EnginePool>) -> Result<String, Error> {
    let pool = Arc::clone(pool);
    tokio::task::spawn_blocking(move || {
        let engine = pool.take()?;
        let id = engine.id().to_owned();
        pool.give_back(engine);
        Ok::<_, anyhow::Error>(id)
    })
    .await
    .map_err(|err| Error::Engine(err.to_string()))?
    .map_err(|err| Error::Engine(format!("{err:#}")))
}

fn check_settings(settings: &SearchSettings) -> Result<(), Error> {
    if !(1..=MAX_MULTIPV).contains(&settings.multipv) {
        return Err(Error::Invalid(format!("multipv: 1 to {MAX_MULTIPV}")));
    }
    let valid = match settings.budget {
        Budget::Depth(depth) => (1..=MAX_DEPTH).contains(&depth),
        Budget::MoveTimeMs(ms) => (10..=MAX_MOVETIME_MS).contains(&ms),
        Budget::Nodes(nodes) => (1_000..=MAX_NODES).contains(&nodes),
    };
    if valid {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "search budget out of bounds: depth 1 to {MAX_DEPTH}, 10 to {MAX_MOVETIME_MS} ms, \
             or 1000 to {MAX_NODES} nodes"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_bounded() {
        let ok = SearchSettings {
            budget: Budget::Depth(16),
            multipv: 3,
        };
        assert!(check_settings(&ok).is_ok());
        assert!(check_settings(&SearchSettings { multipv: 0, ..ok }).is_err());
        assert!(check_settings(&SearchSettings { multipv: 6, ..ok }).is_err());
        assert!(
            check_settings(&SearchSettings {
                budget: Budget::MoveTimeMs(5),
                ..ok
            })
            .is_err()
        );
        assert!(
            check_settings(&SearchSettings {
                budget: Budget::Depth(99),
                ..ok
            })
            .is_err()
        );
    }
}
