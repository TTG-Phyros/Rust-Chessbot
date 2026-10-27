//! Reviews of a player's games run as background jobs: fetch the games,
//! then analyse those not analysed yet on a pool of engines, publishing
//! the progress as it goes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use chess_chesscom::{ApiGame, Client, GameFilter, PlayedGame, is_valid_username};
use chess_review::{
    AnalysisEngine, Budget, EngineSpec, GameAnalysis, SearchSettings, analyse_game, bench,
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
    /// Estimated time left, in seconds, while analysing: from the speed
    /// measured so far, or before that from earlier analyses or a benchmark.
    pub eta_secs: Option<u64>,
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
            eta_secs: None,
            error: None,
        }
    }
}

/// What the time left of a job is estimated from.
#[derive(Debug, Clone, Copy, Default)]
struct Timing {
    /// Start of the analyses.
    started: Option<Instant>,
    /// Engine time per position known beforehand (one worker).
    prior: Option<f64>,
    /// Games analysed at once.
    parallel: usize,
}

/// Positions to analyse before the measured speed replaces the prior one.
const MEASURED_AFTER: usize = 20;

/// Seconds left to analyse `total - done` positions: at the speed measured
/// over `elapsed` seconds once enough positions are done, else at `prior`
/// seconds per position on `parallel` workers.
#[must_use]
pub fn time_left(
    done: usize,
    total: usize,
    elapsed: f64,
    prior: Option<f64>,
    parallel: usize,
) -> Option<f64> {
    let left = total.saturating_sub(done);
    #[allow(clippy::cast_precision_loss)] // Position counts.
    let (done, left) = (done as f64, left as f64);
    #[allow(clippy::cast_precision_loss)]
    let parallel = parallel.max(1) as f64;
    let measured = done >= MEASURED_AFTER as f64 && elapsed > 0.0;
    match (measured, prior) {
        (true, _) => Some(left * elapsed / done),
        (false, Some(prior)) => Some(left * prior / parallel),
        (false, None) => None,
    }
}

/// Whole seconds, rounded up.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Non-negative, bounded.
fn seconds(time: f64) -> u64 {
    time.max(0.0).ceil().min(u64::MAX as f64) as u64
}

/// How long reviewing some games with some settings should take.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Estimate {
    /// The settings.
    pub settings: SearchSettings,
    /// Games already analysed with them (nothing to do).
    pub cached_games: usize,
    /// Games to analyse.
    pub pending_games: usize,
    /// Positions of the games to analyse.
    pub positions: usize,
    /// Engine time per position (one worker), in seconds.
    pub seconds_per_position: f64,
    /// Whether that time was measured on real analyses (else on a short
    /// benchmark, less accurate).
    pub measured: bool,
    /// Estimated duration of the review, in seconds, on the workers.
    pub seconds: u64,
}

/// Duration of analysing games of `lengths` positions, `per_position`
/// seconds each, on `workers` workers: the work shared out, but never less
/// than the longest game (one game runs on one worker).
#[must_use]
pub fn review_time(lengths: &[usize], per_position: f64, workers: usize) -> f64 {
    let total: usize = lengths.iter().sum();
    let longest = lengths.iter().copied().max().unwrap_or(0);
    let parallel = workers.min(lengths.len()).max(1);
    #[allow(clippy::cast_precision_loss)] // Position counts.
    let shared = total as f64 * per_position / parallel as f64;
    #[allow(clippy::cast_precision_loss)]
    let longest = longest as f64 * per_position;
    shared.max(longest)
}

/// Games of a review: which are cached, which remain to analyse.
struct Plan {
    cached: usize,
    pending: Vec<PlayedGame>,
    unreadable: usize,
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
    timing: Mutex<Timing>,
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

    fn timing(&self) -> std::sync::MutexGuard<'_, Timing> {
        self.timing.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The analyses start: `positions` to analyse on `parallel` workers,
    /// `prior` seconds per position if known.
    fn start_analyses(&self, positions: usize, prior: Option<f64>, parallel: usize) {
        *self.timing() = Timing {
            started: Some(Instant::now()),
            prior,
            parallel,
        };
        self.update(|p| {
            p.positions_total = positions;
            p.eta_secs = time_left(0, positions, 0.0, prior, parallel).map(seconds);
        });
    }

    /// One more position analysed.
    fn advance(&self) {
        let timing = *self.timing();
        let elapsed = timing.started.map_or(0.0, |at| at.elapsed().as_secs_f64());
        self.update(|p| {
            p.positions_done += 1;
            p.eta_secs = time_left(
                p.positions_done,
                p.positions_total,
                elapsed,
                timing.prior,
                timing.parallel,
            )
            .map(seconds);
        });
    }

    fn finish(&self, state: JobState, error: Option<String>) {
        self.update(|progress| {
            progress.state = state;
            progress.error = error;
            progress.eta_secs = None;
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

    /// The analysis of game `id` to show: the one made with the configured
    /// engine and default settings if there is one, the latest otherwise.
    ///
    /// # Errors
    ///
    /// When the cache fails.
    pub fn analysis_to_show(&self, id: &str) -> Result<Option<GameAnalysis>, Error> {
        let mut analyses = self.store.analyses(id)?;
        let settings = self.config.search_settings();
        let native = self.config.analysis.engine == EngineKind::Native;
        let preferred = analyses.iter().position(|analysis| {
            analysis.settings == settings && analysis.engine.starts_with("chess-bot") == native
        });
        Ok(match preferred {
            Some(index) => Some(analyses.swap_remove(index)),
            None => analyses.into_iter().next(),
        })
    }

    /// Profile of `username` over their cached games kept by `filter` that
    /// have an analysis (see [`Self::analysis_to_show`]); also returns how
    /// many of these games have none.
    ///
    /// # Errors
    ///
    /// When the cache fails.
    pub fn profile(
        &self,
        username: &str,
        filter: &GameFilter,
    ) -> Result<(crate::profile::Profile, u32), Error> {
        let mut games = Vec::new();
        let mut unanalysed = 0;
        for api in self.store.games(username, filter)? {
            let Some(analysis) = self.analysis_to_show(api.id())? else {
                unanalysed += 1;
                continue;
            };
            match PlayedGame::from_api(&api) {
                Ok(game) => games.push((game, analysis)),
                Err(err) => tracing::warn!(game = api.id(), %err, "unreadable game"),
            }
        }
        let profile = crate::profile::build_profile(
            username,
            &games,
            &self.config.classification,
            &self.config.profile,
        );
        Ok((profile, unanalysed))
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
        self.check_request(&request.username, &request.filter)?;
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
            timing: Mutex::new(Timing::default()),
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

        let plan = self.plan(&games, &key)?;
        job.update(|p| {
            p.games_cached = plan.cached;
            p.games_failed = plan.unreadable;
        });
        let positions: usize = plan.pending.iter().map(positions_of).sum();
        let parallel = self.config.workers().min(plan.pending.len());
        job.start_analyses(positions, self.known_speed(&key)?, parallel);

        let mut tasks = JoinSet::new();
        for game in plan.pending {
            let (service, job, pool) = (Arc::clone(self), Arc::clone(job), Arc::clone(&pool));
            tasks.spawn(async move {
                let result = service.analyse(&job, &pool, game.clone(), settings).await;
                (game.id, result)
            });
        }
        while let Some(joined) = tasks.join_next().await {
            let outcome = match joined {
                Ok((id, Ok(Some((analysis, seconds))))) => {
                    // Measured on real games: the estimates of the next reviews.
                    #[allow(clippy::cast_precision_loss)] // Position counts.
                    let positions = analysis.positions.len() as f64;
                    let key = analysis_key(&analysis.engine, &analysis.settings);
                    self.store
                        .save_analysis(&id, &analysis, crate::now())
                        .and_then(|()| self.store.record_speed(&key, positions, seconds))
                        .map_err(|err| err.to_string())
                }
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

    /// Analyses `game` once a worker is free, with the engine time it took
    /// (seconds); `None` when cancelled.
    async fn analyse(
        &self,
        job: &Arc<Job>,
        pool: &Arc<EnginePool>,
        game: PlayedGame,
        settings: SearchSettings,
    ) -> anyhow::Result<Option<(GameAnalysis, f64)>> {
        let _permit = Arc::clone(&self.workers).acquire_owned().await?;
        if job.is_cancelled() {
            return Ok(None);
        }
        let (job, pool) = (Arc::clone(job), Arc::clone(pool));
        tokio::task::spawn_blocking(move || {
            let mut engine = pool.take()?;
            let clock = Instant::now();
            let analysis = analyse_game(
                engine.as_mut(),
                game.game.start(),
                game.game.moves(),
                &settings,
                |_, _| {
                    job.advance();
                    !job.is_cancelled()
                },
            )?;
            let seconds = clock.elapsed().as_secs_f64();
            // A failed engine is dropped (the process is killed).
            pool.give_back(engine);
            Ok(analysis.map(|analysis| (analysis, seconds)))
        })
        .await?
    }

    fn check_request(&self, username: &str, filter: &GameFilter) -> Result<(), Error> {
        if self.client.is_none() {
            return Err(Error::Unavailable(
                "Chess.com reviews need a User-Agent: set CHESS_REVIEW_USER_AGENT".into(),
            ));
        }
        if !is_valid_username(username) {
            return Err(Error::Invalid(format!(
                "invalid Chess.com username {username:?}"
            )));
        }
        let max = self.config.max_games;
        if !(1..=max).contains(&filter.max_games) {
            return Err(Error::Invalid(format!("ask for 1 to {max} games")));
        }
        Ok(())
    }

    /// Which of `games` are analysed with the configuration `key`.
    fn plan(&self, games: &[ApiGame], key: &str) -> Result<Plan, Error> {
        let mut plan = Plan {
            cached: 0,
            pending: Vec::new(),
            unreadable: 0,
        };
        for api in games {
            if self.store.analysis(api.id(), key)?.is_some() {
                plan.cached += 1;
                continue;
            }
            match PlayedGame::from_api(api) {
                Ok(game) => plan.pending.push(game),
                Err(err) => {
                    tracing::warn!(game = api.id(), %err, "unreadable game");
                    plan.unreadable += 1;
                }
            }
        }
        Ok(plan)
    }

    /// How long reviewing the games of `username` kept by `filter` with
    /// `engine` would take, for each of `candidates` (settings). Brings the
    /// games into the cache (the review then starts at once); the speed of
    /// settings never used before is measured on a short benchmark (a few
    /// seconds at most, on a worker).
    ///
    /// # Errors
    ///
    /// As [`ReviewService::start`], plus Chess.com and engine failures.
    pub async fn estimate(
        &self,
        username: &str,
        filter: &GameFilter,
        engine: Option<EngineKind>,
        candidates: &[SearchSettings],
    ) -> Result<Vec<Estimate>, Error> {
        self.check_request(username, filter)?;
        for settings in candidates {
            check_settings(settings)?;
        }
        let Some(client) = &self.client else {
            return Ok(Vec::new());
        };
        let games = sync_player(
            client,
            &self.store,
            username,
            filter,
            self.config.recent_ttl(),
            |_| {},
        )
        .await?;
        let pool = Arc::clone(&self.pools[&engine.unwrap_or(self.config.analysis.engine)]);
        let engine = engine_id(&pool).await?;
        let mut estimates = Vec::with_capacity(candidates.len());
        for &settings in candidates {
            let key = analysis_key(&engine, &settings);
            let plan = self.plan(&games, &key)?;
            let lengths: Vec<usize> = plan.pending.iter().map(positions_of).collect();
            let (per_position, measured) = match self.store.speed(&key)? {
                Some(speed) => (speed, true),
                None => match self.store.speed(&benchmark_key(&key))? {
                    Some(speed) => (speed, false),
                    None => (self.benchmark(&pool, &key, settings).await?, false),
                },
            };
            estimates.push(Estimate {
                settings,
                cached_games: plan.cached,
                pending_games: lengths.len(),
                positions: lengths.iter().sum(),
                seconds_per_position: per_position,
                measured,
                seconds: seconds(review_time(&lengths, per_position, self.config.workers())),
            });
        }
        Ok(estimates)
    }

    /// Engine time per position for the configuration `key`: measured on
    /// real analyses, else on the benchmark, if either was run.
    fn known_speed(&self, key: &str) -> Result<Option<f64>, Error> {
        match self.store.speed(key)? {
            Some(speed) => Ok(Some(speed)),
            None => self.store.speed(&benchmark_key(key)),
        }
    }

    /// Seconds per position of `pool`'s engine with `settings`, measured on
    /// a worker and recorded apart from real analyses (see [`benchmark_key`]).
    async fn benchmark(
        &self,
        pool: &Arc<EnginePool>,
        key: &str,
        settings: SearchSettings,
    ) -> Result<f64, Error> {
        let _permit = Arc::clone(&self.workers)
            .acquire_owned()
            .await
            .map_err(|err| Error::Engine(err.to_string()))?;
        let pool = Arc::clone(pool);
        let per_position = tokio::task::spawn_blocking(move || {
            let mut engine = pool.take()?;
            let per_position = bench::seconds_per_position(engine.as_mut(), &settings)?;
            pool.give_back(engine);
            Ok::<_, anyhow::Error>(per_position)
        })
        .await
        .map_err(|err| Error::Engine(err.to_string()))?
        .map_err(|err| Error::Engine(format!("{err:#}")))?;
        #[allow(clippy::cast_precision_loss)] // A few positions.
        let positions = bench::POSITIONS as f64;
        self.store
            .record_speed(&benchmark_key(key), positions, per_position * positions)?;
        Ok(per_position)
    }
}

/// Where the benchmark of the configuration `key` is recorded: apart from
/// real analyses, which replace it as soon as there are some.
fn benchmark_key(key: &str) -> String {
    format!("benchmark | {key}")
}

/// Positions of a game: the start, then one per move.
fn positions_of(game: &PlayedGame) -> usize {
    game.game.moves().len() + 1
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
    fn time_left_uses_the_measured_speed_once_known() {
        // Nothing known yet.
        assert_eq!(time_left(0, 100, 0.0, None, 4), None);
        // Before 20 positions: the prior speed, shared by the workers.
        assert_eq!(time_left(10, 110, 0.5, Some(0.4), 4), Some(10.0));
        // Then the speed measured: 50 positions in 10 s, 50 left.
        assert_eq!(time_left(50, 100, 10.0, Some(9.0), 4), Some(10.0));
        assert_eq!(time_left(100, 100, 10.0, None, 4), Some(0.0));
        assert_eq!(seconds(9.2), 10);
        assert_eq!(seconds(-1.0), 0);
    }

    #[test]
    fn review_time_is_bounded_by_the_longest_game() {
        // 4 games, 4 workers: the longest one sets the pace.
        assert!((review_time(&[100, 50, 50, 50], 0.5, 4) - 50.0).abs() < 1e-9);
        // 2 workers: the work shared out.
        assert!((review_time(&[100, 100, 100, 100], 0.5, 2) - 100.0).abs() < 1e-9);
        // More workers than games.
        assert!((review_time(&[80], 0.5, 8) - 40.0).abs() < 1e-9);
        assert!(review_time(&[], 0.5, 4).abs() < 1e-9);
    }

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
