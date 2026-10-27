//! `/api/review/*`: reviews of a Chess.com player's games. A job fetches
//! the games and analyses them in the background; its progress is polled
//! or streamed (Server-Sent Events); games and analyses are then read from
//! the cache.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, HeaderName};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use chess_chesscom::{ApiGame, GameFilter, PlayedGame, TimeClass, parse_date};
use chess_core::{Color, Position};
use chess_openings::Book;
use chess_review::{
    Budget, GameAnalysis, GameReview, MoveClass, PlayerReview, Score, SearchSettings, review_game,
};
use chess_reviewer::jobs::{MAX_DEPTH, MAX_MULTIPV};
use chess_reviewer::{EngineKind, Job, JobProgress, JobRequest, JobState, ReviewService};
use futures_util::Stream;
use serde::Deserialize;
use utoipa::IntoParams;

use super::error::ApiError;
use super::types::review::{
    ReviewAnalysis, ReviewClass, ReviewClassCounts, ReviewEngine, ReviewEstimate, ReviewEstimates,
    ReviewGameSummary, ReviewGameView, ReviewJobState, ReviewJobView, ReviewLine, ReviewMove,
    ReviewOpening, ReviewOptions, ReviewPlayer, ReviewPlayerStats, ReviewPosition, ReviewPreset,
    ReviewRequest, ReviewStarted, ReviewVerdict, TimeClassView,
};
use super::types::{ErrorResponse, ScoreView};
use crate::AppState;

/// Games of a review when the request does not say.
const DEFAULT_GAMES: usize = 20;
/// Label of the request's own settings among the presets.
const CUSTOM_LABEL: &str = "Personnalisé";
/// Least time between two progress events of a stream.
const EVENT_INTERVAL: Duration = Duration::from_millis(150);

fn service(state: &AppState) -> Result<&Arc<ReviewService>, ApiError> {
    state
        .review
        .as_ref()
        .ok_or_else(|| ApiError::unavailable("Chess.com reviews are not available (no cache)"))
}

fn job(state: &AppState, id: u64) -> Result<Arc<Job>, ApiError> {
    service(state)?
        .job(id)
        .ok_or_else(|| ApiError::not_found(format!("no review job {id}")))
}

fn api_error(err: chess_reviewer::Error) -> ApiError {
    match err {
        chess_reviewer::Error::Invalid(message) => ApiError::unprocessable(message),
        chess_reviewer::Error::Unavailable(message) => ApiError::unavailable(message),
        err => ApiError::internal(err.to_string()),
    }
}

fn day(text: Option<&str>, field: &str) -> Result<Option<i64>, ApiError> {
    text.filter(|text| !text.trim().is_empty())
        .map(|text| {
            parse_date(text).ok_or_else(|| {
                ApiError::unprocessable(format!("{field}: expected YYYY-MM-DD, got {text:?}"))
            })
        })
        .transpose()
}

const fn time_class(class: TimeClassView) -> TimeClass {
    match class {
        TimeClassView::Bullet => TimeClass::Bullet,
        TimeClassView::Blitz => TimeClass::Blitz,
        TimeClassView::Rapid => TimeClass::Rapid,
        TimeClassView::Daily => TimeClass::Daily,
    }
}

const fn time_class_view(class: TimeClass) -> TimeClassView {
    match class {
        TimeClass::Bullet => TimeClassView::Bullet,
        TimeClass::Blitz => TimeClassView::Blitz,
        TimeClass::Rapid => TimeClassView::Rapid,
        TimeClass::Daily => TimeClassView::Daily,
    }
}

const fn engine_kind(engine: ReviewEngine) -> EngineKind {
    match engine {
        ReviewEngine::Stockfish => EngineKind::Stockfish,
        ReviewEngine::Native => EngineKind::Native,
    }
}

/// The games a request designates.
fn filter(request: &ReviewRequest) -> Result<GameFilter, ApiError> {
    Ok(GameFilter {
        time_classes: request
            .time_classes
            .iter()
            .copied()
            .map(time_class)
            .collect(),
        max_games: request
            .max_games
            .map_or(DEFAULT_GAMES, |n| usize::try_from(n).unwrap_or(usize::MAX)),
        since: day(request.since.as_deref(), "since")?,
        // Inclusive: up to the end of that day.
        until: day(request.until.as_deref(), "until")?.map(|start| start + 86_400),
        rated_only: request.rated_only.unwrap_or(false),
    })
}

/// The search settings of a request: its preset (the default one if it
/// names none), overridden by its own depth, time and lines.
fn settings(service: &ReviewService, request: &ReviewRequest) -> Result<SearchSettings, ApiError> {
    let config = service.config();
    let base = match request.preset.as_deref() {
        Some(name) => config
            .preset(name)
            .ok_or_else(|| ApiError::unprocessable(format!("unknown preset {name:?}")))?
            .settings(),
        None => config.search_settings(),
    };
    Ok(SearchSettings {
        budget: match (request.depth, request.movetime_ms) {
            (Some(depth), _) => Budget::Depth(depth),
            (None, Some(ms)) => Budget::MoveTimeMs(ms),
            (None, None) => base.budget,
        },
        multipv: request
            .multipv
            .map_or(base.multipv, |n| usize::try_from(n).unwrap_or(0)),
    })
}

/// Starts reviewing a player's games.
#[utoipa::path(
    post,
    path = "/api/review/jobs",
    tag = "review",
    request_body = ReviewRequest,
    responses(
        (status = 202, description = "Job started", body = ReviewStarted),
        (status = 422, description = "Invalid username, dates, preset or settings", body = ErrorResponse),
        (status = 503, description = "No User-Agent configured for Chess.com", body = ErrorResponse),
    )
)]
pub(super) async fn start(
    State(state): State<AppState>,
    Json(request): Json<ReviewRequest>,
) -> Result<(StatusCode, Json<ReviewStarted>), ApiError> {
    let service = service(&state)?;
    let job = service
        .start(JobRequest {
            username: request.username.trim().to_owned(),
            filter: filter(&request)?,
            engine: request.engine.map(engine_kind),
            settings: Some(settings(service, &request)?),
        })
        .map_err(api_error)?;
    Ok((StatusCode::ACCEPTED, Json(ReviewStarted { job_id: job.id })))
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Depth and time of a budget, as the API spells them.
const fn budget_fields(budget: Budget) -> (Option<u32>, Option<u64>) {
    match budget {
        Budget::Depth(depth) => (Some(depth), None),
        Budget::MoveTimeMs(ms) => (None, Some(ms)),
        // Not offered by the API: shown as neither.
        Budget::Nodes(_) => (None, None),
    }
}

/// What a review request may choose: presets, engine, limits.
#[utoipa::path(
    get,
    path = "/api/review/options",
    tag = "review",
    responses(
        (status = 200, description = "Presets and limits", body = ReviewOptions),
        (status = 503, description = "Reviews unavailable", body = ErrorResponse),
    )
)]
pub(super) async fn options(
    State(state): State<AppState>,
) -> Result<Json<ReviewOptions>, ApiError> {
    let service = service(&state)?;
    let config = service.config();
    let presets = config
        .analysis
        .presets
        .iter()
        .map(|preset| {
            let settings = preset.settings();
            let (depth, movetime_ms) = budget_fields(settings.budget);
            ReviewPreset {
                name: preset.name.clone(),
                label: preset.label.clone(),
                depth,
                movetime_ms,
                multipv: count(settings.multipv),
            }
        })
        .collect();
    Ok(Json(ReviewOptions {
        chesscom_enabled: service.chesscom_enabled(),
        default_engine: match config.analysis.engine {
            EngineKind::Stockfish => ReviewEngine::Stockfish,
            EngineKind::Native => ReviewEngine::Native,
        },
        presets,
        default_preset: config
            .default_preset()
            .map(|preset| preset.name.clone())
            .unwrap_or_default(),
        workers: count(config.workers()),
        max_games: count(config.max_games),
        max_multipv: count(MAX_MULTIPV),
        max_depth: MAX_DEPTH,
    }))
}

/// How long reviewing the requested games would take with each preset
/// (and with the request's own settings). Brings the games into the cache;
/// settings never used before are timed on a short benchmark first (a few
/// seconds).
#[utoipa::path(
    post,
    path = "/api/review/estimate",
    tag = "review",
    request_body = ReviewRequest,
    responses(
        (status = 200, description = "One estimate per preset", body = ReviewEstimates),
        (status = 422, description = "Invalid username, dates, preset or settings", body = ErrorResponse),
        (status = 503, description = "No User-Agent configured for Chess.com", body = ErrorResponse),
    )
)]
pub(super) async fn estimate(
    State(state): State<AppState>,
    Json(request): Json<ReviewRequest>,
) -> Result<Json<ReviewEstimates>, ApiError> {
    let service = service(&state)?;
    let config = service.config();
    let mut candidates: Vec<(Option<String>, String, SearchSettings)> = config
        .analysis
        .presets
        .iter()
        .map(|preset| {
            (
                Some(preset.name.clone()),
                preset.label.clone(),
                preset.settings(),
            )
        })
        .collect();
    let own = settings(service, &request)?;
    if candidates.iter().all(|(_, _, settings)| *settings != own) {
        candidates.push((None, CUSTOM_LABEL.to_owned(), own));
    }
    let settings: Vec<SearchSettings> = candidates.iter().map(|(_, _, s)| *s).collect();
    let estimates = service
        .estimate(
            request.username.trim(),
            &filter(&request)?,
            request.engine.map(engine_kind),
            &settings,
        )
        .await
        .map_err(api_error)?;
    let games = estimates
        .first()
        .map_or(0, |e| e.cached_games + e.pending_games);
    let options = candidates
        .into_iter()
        .zip(estimates)
        .map(|((preset, label, settings), estimate)| {
            let (depth, movetime_ms) = budget_fields(settings.budget);
            ReviewEstimate {
                preset,
                label,
                depth,
                movetime_ms,
                multipv: count(settings.multipv),
                cached_games: count(estimate.cached_games),
                pending_games: count(estimate.pending_games),
                positions: count(estimate.positions),
                seconds_per_position: estimate.seconds_per_position,
                measured: estimate.measured,
                seconds: estimate.seconds,
            }
        })
        .collect();
    Ok(Json(ReviewEstimates {
        games: count(games),
        workers: count(config.workers()),
        options,
    }))
}

fn job_view(job: &Job, progress: JobProgress) -> ReviewJobView {
    ReviewJobView {
        id: job.id,
        username: job.username.clone(),
        state: match progress.state {
            JobState::Fetching => ReviewJobState::Fetching,
            JobState::Analysing => ReviewJobState::Analysing,
            JobState::Done => ReviewJobState::Done,
            JobState::Failed => ReviewJobState::Failed,
            JobState::Cancelled => ReviewJobState::Cancelled,
        },
        months_done: count(progress.months_done),
        months_total: count(progress.months_total),
        games: progress.games,
        games_cached: count(progress.games_cached),
        games_done: count(progress.games_done),
        games_failed: count(progress.games_failed),
        positions_done: count(progress.positions_done),
        positions_total: count(progress.positions_total),
        engine: progress.engine,
        eta_seconds: progress.eta_secs,
        error: progress.error,
    }
}

/// Progress of a review job.
#[utoipa::path(
    get,
    path = "/api/review/jobs/{id}",
    tag = "review",
    params(("id" = u64, Path, description = "Job identifier")),
    responses(
        (status = 200, description = "Progress", body = ReviewJobView),
        (status = 404, description = "Unknown or expired job", body = ErrorResponse),
    )
)]
pub(super) async fn progress(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<ReviewJobView>, ApiError> {
    let job = job(&state, id)?;
    Ok(Json(job_view(&job, job.progress())))
}

/// Progress of a review job as Server-Sent Events: a `progress` event
/// (a [`ReviewJobView`]) at once, then at each change (at most every
/// 150 ms); the stream ends after the final state.
#[utoipa::path(
    get,
    path = "/api/review/jobs/{id}/events",
    tag = "review",
    params(("id" = u64, Path, description = "Job identifier")),
    responses(
        (status = 200, description = "`progress` events carrying a ReviewJobView", content_type = "text/event-stream"),
        (status = 404, description = "Unknown or expired job", body = ErrorResponse),
    )
)]
pub(super) async fn events(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, ApiError> {
    let job = job(&state, id)?;
    let stream = progress_events(job);
    Ok((
        // Nginx would otherwise buffer the stream.
        [
            (HeaderName::from_static("x-accel-buffering"), "no"),
            (CACHE_CONTROL, "no-cache"),
        ],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    ))
}

fn progress_events(job: Arc<Job>) -> impl Stream<Item = Result<Event, Infallible>> {
    let updates = job.subscribe();
    futures_util::stream::unfold(
        (job, updates, true, false),
        |(job, mut updates, first, done)| async move {
            if done {
                return None;
            }
            if !first {
                // Several changes in the interval become one event.
                tokio::time::sleep(EVENT_INTERVAL).await;
                // The job ended and was dropped: nothing more will come.
                updates.changed().await.ok()?;
            }
            let progress = updates.borrow_and_update().clone();
            let finished = progress.state.is_final();
            let view = job_view(&job, progress);
            let event = Event::default()
                .event("progress")
                .json_data(view)
                .unwrap_or_else(|_| Event::default().event("progress"));
            Some((Ok(event), (job, updates, false, finished)))
        },
    )
}

/// Asks a review job to stop.
#[utoipa::path(
    delete,
    path = "/api/review/jobs/{id}",
    tag = "review",
    params(("id" = u64, Path, description = "Job identifier")),
    responses(
        (status = 204, description = "Stopping (the stream reports `cancelled`)"),
        (status = 404, description = "Unknown or expired job", body = ErrorResponse),
    )
)]
pub(super) async fn cancel(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    job(&state, id)?.cancel();
    Ok(StatusCode::NO_CONTENT)
}

/// Filters of `GET /api/review/players/{username}/games`.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub(super) struct GamesQuery {
    /// Time classes, comma-separated (`blitz,rapid`); all by default.
    time_classes: Option<String>,
    /// Number of games, most recent first (20 by default).
    max_games: Option<usize>,
    /// Games that ended on or after this day (`YYYY-MM-DD`).
    since: Option<String>,
    /// Games that ended on or before this day (`YYYY-MM-DD`).
    until: Option<String>,
    /// Rated games only.
    rated_only: Option<bool>,
}

/// Cached games of a player (nothing is downloaded: start a job for that).
#[utoipa::path(
    get,
    path = "/api/review/players/{username}/games",
    tag = "review",
    params(("username" = String, Path, description = "Chess.com username"), GamesQuery),
    responses(
        (status = 200, description = "Games, most recent first", body = [ReviewGameSummary]),
        (status = 422, description = "Invalid filter", body = ErrorResponse),
    )
)]
pub(super) async fn player_games(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(query): Query<GamesQuery>,
) -> Result<Json<Vec<ReviewGameSummary>>, ApiError> {
    let service = service(&state)?;
    let time_classes = query
        .time_classes
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .filter(|text| !text.trim().is_empty())
        .map(str::parse)
        .collect::<Result<Vec<TimeClass>, _>>()
        .map_err(ApiError::unprocessable)?;
    let filter = GameFilter {
        time_classes,
        max_games: query
            .max_games
            .unwrap_or(DEFAULT_GAMES)
            .min(service.config().max_games),
        since: day(query.since.as_deref(), "since")?,
        until: day(query.until.as_deref(), "until")?.map(|start| start + 86_400),
        rated_only: query.rated_only.unwrap_or(false),
    };
    let store = service.store();
    let games = store.games(&username, &filter).map_err(api_error)?;
    games
        .iter()
        .map(|api| {
            let analysed = store
                .latest_analysis(api.id())
                .map_err(api_error)?
                .is_some();
            Ok(summary(api, analysed))
        })
        .collect::<Result<_, ApiError>>()
        .map(Json)
}

/// The opening of `game` in the lichess base.
fn opening(game: &PlayedGame) -> Option<ReviewOpening> {
    let found = Book::lichess().classify(game.game.start(), game.game.moves());
    found.opening.map(|opening| ReviewOpening {
        eco: opening.eco.clone(),
        name: opening.name.clone(),
        family: opening.family().to_owned(),
        variation: opening.variation().map(str::to_owned),
        ply: count(found.opening_ply),
        book_plies: count(found.book_plies),
    })
}

fn summary(api: &ApiGame, analysed: bool) -> ReviewGameSummary {
    let parsed = PlayedGame::from_api(api).ok();
    let player = |player: &chess_chesscom::ApiPlayer| ReviewPlayer {
        username: player.username.clone(),
        rating: player.rating,
    };
    ReviewGameSummary {
        id: api.id().to_owned(),
        url: api.url.clone(),
        end_time: api.end_time,
        time_class: time_class_view(api.time_class),
        time_control: api.time_control.clone(),
        rated: api.rated,
        white: player(&api.white),
        black: player(&api.black),
        result: parsed
            .as_ref()
            .map_or("*", |game| game.result.as_str())
            .to_owned(),
        plies: parsed
            .as_ref()
            .map_or(0, |game| count(game.game.moves().len())),
        opening: parsed.as_ref().and_then(opening),
        analysed,
    }
}

/// A cached game with its latest analysis.
#[utoipa::path(
    get,
    path = "/api/review/games/{id}",
    tag = "review",
    params(("id" = String, Path, description = "Game identifier (Chess.com UUID)")),
    responses(
        (status = 200, description = "The game and its analysis", body = ReviewGameView),
        (status = 404, description = "Game not in the cache", body = ErrorResponse),
    )
)]
pub(super) async fn game(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ReviewGameView>, ApiError> {
    let service = service(&state)?;
    let store = service.store();
    let api = store
        .game(&id)
        .map_err(api_error)?
        .ok_or_else(|| ApiError::not_found(format!("no cached game {id}")))?;
    let played = PlayedGame::from_api(&api)
        .map_err(|err| ApiError::internal(format!("unreadable game {id}: {err}")))?;
    let analysis = service.analysis_to_show(&id).map_err(api_error)?;

    let start = played.game.start().clone();
    let book = Book::lichess()
        .classify(&start, played.game.moves())
        .book_plies;
    let mut position = start.clone();
    let mut moves = Vec::with_capacity(played.game.moves().len());
    for (ply, (&mv, clock)) in played.game.moves().iter().zip(&played.clocks).enumerate() {
        moves.push(ReviewMove {
            uci: mv.to_string(),
            san: position.san(mv),
            clock_ms: clock.map(|clock| u64::try_from(clock.as_millis()).unwrap_or(u64::MAX)),
            book: ply < book,
        });
        position.make_move(mv);
    }
    let analysis = match analysis.filter(|analysis| analysis.positions.len() == moves.len() + 1) {
        Some(analysis) => {
            let review = review_game(
                &start,
                played.game.moves(),
                &analysis.positions,
                book,
                &service.config().classification,
            )
            .map_err(|err| ApiError::internal(format!("cannot review game {id}: {err}")))?;
            Some(analysis_view(&played, &analysis, &review))
        }
        None => None,
    };
    Ok(Json(ReviewGameView {
        game: summary(&api, analysis.is_some()),
        start_fen: start.to_fen(),
        moves,
        analysis,
    }))
}

/// `score` (for `to_move`) from White's point of view.
const fn white_view(score: Score, to_move: Color) -> ScoreView {
    let sign = to_move.sign();
    match score {
        Score::Cp(value) => ScoreView::Cp {
            value: value * sign,
        },
        Score::Mate(moves) => ScoreView::Mate {
            moves: moves * sign,
        },
    }
}

fn analysis_view(
    played: &PlayedGame,
    analysis: &GameAnalysis,
    review: &GameReview,
) -> ReviewAnalysis {
    let mut position = played.game.start().clone();
    let mut positions = Vec::with_capacity(analysis.positions.len());
    for (ply, eval) in analysis.positions.iter().enumerate() {
        if ply > 0 {
            position.make_move(played.game.moves()[ply - 1]);
        }
        let to_move = position.side_to_move();
        positions.push(ReviewPosition {
            fen: position.to_fen(),
            score: white_view(eval.score(), to_move),
            win: review.white_wins.get(ply).copied().unwrap_or(50.0),
            checkmated: eval.checkmated,
            lines: eval
                .lines
                .iter()
                .map(|line| ReviewLine {
                    score: white_view(line.score, to_move),
                    depth: line.depth,
                    san: line_san(&position, &line.pv),
                    uci: line.pv.clone(),
                })
                .collect(),
        });
    }
    ReviewAnalysis {
        engine: analysis.engine.clone(),
        budget: analysis.settings.budget.to_string(),
        multipv: count(analysis.settings.multipv),
        positions,
        moves: review
            .moves
            .iter()
            .map(|verdict| ReviewVerdict {
                class: class_view(verdict.class),
                win_before: verdict.win_before,
                win_after: verdict.win_after,
                accuracy: verdict.accuracy,
                cp_loss: verdict.cp_loss,
            })
            .collect(),
        white: player_stats(review.player(Color::White)),
        black: player_stats(review.player(Color::Black)),
    }
}

const fn class_view(class: MoveClass) -> ReviewClass {
    match class {
        MoveClass::Brilliant => ReviewClass::Brilliant,
        MoveClass::Great => ReviewClass::Great,
        MoveClass::Best => ReviewClass::Best,
        MoveClass::Excellent => ReviewClass::Excellent,
        MoveClass::Good => ReviewClass::Good,
        MoveClass::Book => ReviewClass::Book,
        MoveClass::Inaccuracy => ReviewClass::Inaccuracy,
        MoveClass::Mistake => ReviewClass::Mistake,
        MoveClass::Miss => ReviewClass::Miss,
        MoveClass::Blunder => ReviewClass::Blunder,
    }
}

fn player_stats(player: &PlayerReview) -> ReviewPlayerStats {
    let counts = &player.counts;
    ReviewPlayerStats {
        moves: player.moves,
        accuracy: player.accuracy,
        acpl: player.acpl,
        counts: ReviewClassCounts {
            brilliant: counts.get(MoveClass::Brilliant),
            great: counts.get(MoveClass::Great),
            best: counts.get(MoveClass::Best),
            excellent: counts.get(MoveClass::Excellent),
            good: counts.get(MoveClass::Good),
            book: counts.get(MoveClass::Book),
            inaccuracy: counts.get(MoveClass::Inaccuracy),
            mistake: counts.get(MoveClass::Mistake),
            miss: counts.get(MoveClass::Miss),
            blunder: counts.get(MoveClass::Blunder),
        },
    }
}

/// SAN of a line (UCI) from `position`, up to its first unreadable move.
fn line_san(position: &Position, pv: &[String]) -> Vec<String> {
    let mut position = position.without_history();
    let mut san = Vec::with_capacity(pv.len());
    for text in pv {
        let Ok(mv) = position.parse_uci_move(text) else {
            break;
        };
        san.push(position.san(mv));
        position.make_move(mv);
    }
    san
}
