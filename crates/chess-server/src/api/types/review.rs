//! Types of the Chess.com reviews (`/api/review/*`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

use super::ScoreView;

/// Speed category of a Chess.com game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TimeClassView {
    /// Under 3 minutes.
    Bullet,
    /// 3 to 10 minutes.
    Blitz,
    /// 10 minutes or more.
    Rapid,
    /// Days per move.
    Daily,
}

/// Engine of a review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ReviewEngine {
    /// Stockfish, the reference.
    Stockfish,
    /// Our engine (NNUE, or the classical evaluation without a network).
    Native,
}

/// Body of `POST /api/review/jobs`: whose games, which ones, how deep.
/// Every field but the username is optional (configured defaults).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewRequest {
    /// Chess.com username.
    #[ts(optional = false)]
    pub username: String,
    /// Time classes to keep (all if empty).
    #[serde(default)]
    #[ts(optional = false)]
    pub time_classes: Vec<TimeClassView>,
    /// Number of games, most recent first (20 by default).
    pub max_games: Option<u32>,
    /// Games that ended on or after this day (`YYYY-MM-DD`, UTC).
    pub since: Option<String>,
    /// Games that ended on or before this day (`YYYY-MM-DD`, UTC).
    pub until: Option<String>,
    /// Rated games only.
    pub rated_only: Option<bool>,
    /// Engine.
    pub engine: Option<ReviewEngine>,
    /// Preset (`name` of a [`ReviewPreset`]); the default one if absent.
    /// `depth`, `movetimeMs` and `multipv` override its values.
    pub preset: Option<String>,
    /// Depth of each search.
    pub depth: Option<u32>,
    /// Time of each search, in milliseconds (instead of a depth).
    #[ts(type = "number", optional = nullable)]
    pub movetime_ms: Option<u64>,
    /// Best lines per position (1 to 5).
    pub multipv: Option<u32>,
}

/// Named search settings offered to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewPreset {
    /// Identifier, for requests (`fast`).
    pub name: String,
    /// Label (`Rapide`).
    pub label: String,
    /// Depth of each search.
    pub depth: Option<u32>,
    /// Time of each search, in milliseconds.
    #[ts(type = "number", optional = nullable)]
    pub movetime_ms: Option<u64>,
    /// Best lines per position.
    pub multipv: u32,
}

/// `GET /api/review/options`: what a review request may choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewOptions {
    /// Whether Chess.com can be reached (a User-Agent is configured).
    pub chesscom_enabled: bool,
    /// Engine used by default.
    pub default_engine: ReviewEngine,
    /// The presets, fastest first.
    pub presets: Vec<ReviewPreset>,
    /// Preset used by default.
    pub default_preset: String,
    /// Games analysed at once.
    pub workers: u32,
    /// Most games per review.
    pub max_games: u32,
    /// Most lines per position.
    pub max_multipv: u32,
    /// Deepest search.
    pub max_depth: u32,
}

/// Estimated duration of a review with some settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewEstimate {
    /// The preset, or `null` for the request's own settings.
    pub preset: Option<String>,
    /// Label of the preset (`Personnalisé` for the request's own settings).
    pub label: String,
    /// Depth of each search.
    pub depth: Option<u32>,
    /// Time of each search, in milliseconds.
    #[ts(type = "number", optional = nullable)]
    pub movetime_ms: Option<u64>,
    /// Best lines per position.
    pub multipv: u32,
    /// Games already analysed with these settings.
    pub cached_games: u32,
    /// Games to analyse.
    pub pending_games: u32,
    /// Positions of the games to analyse.
    pub positions: u32,
    /// Engine time per position (one worker), in seconds.
    pub seconds_per_position: f64,
    /// Whether that time comes from real analyses (else a short benchmark).
    pub measured: bool,
    /// Estimated duration, in seconds.
    #[ts(type = "number")]
    pub seconds: u64,
}

/// Reply of `POST /api/review/estimate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewEstimates {
    /// Games of the request (cached now: the review starts at once).
    pub games: u32,
    /// Workers sharing the analyses.
    pub workers: u32,
    /// One estimate per preset, then the request's own settings if they
    /// differ from every preset.
    pub options: Vec<ReviewEstimate>,
}

/// Reply of `POST /api/review/jobs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewStarted {
    /// Identifier of the job, for its progress (`/api/review/jobs/{id}`).
    #[ts(type = "number")]
    pub job_id: u64,
}

/// Stage of a review job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ReviewJobState {
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

/// Progress of a review job (also each event of its SSE stream).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewJobView {
    /// Identifier.
    #[ts(type = "number")]
    pub id: u64,
    /// Player reviewed.
    pub username: String,
    /// Stage.
    pub state: ReviewJobState,
    /// Months of the archive read so far.
    pub months_done: u32,
    /// Months that may be read.
    pub months_total: u32,
    /// Games of the review, most recent first (once fetched).
    pub games: Vec<String>,
    /// Games already analysed with these settings.
    pub games_cached: u32,
    /// Games analysed by this job.
    pub games_done: u32,
    /// Games that could not be analysed.
    pub games_failed: u32,
    /// Positions analysed so far.
    pub positions_done: u32,
    /// Positions of the games to analyse.
    pub positions_total: u32,
    /// Engine analysing the games.
    pub engine: Option<String>,
    /// Estimated time left while analysing, in seconds.
    #[ts(type = "number", optional = nullable)]
    pub eta_seconds: Option<u64>,
    /// Why the job failed.
    pub error: Option<String>,
}

/// A player of a reviewed game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewPlayer {
    /// Chess.com username.
    pub username: String,
    /// Rating after the game.
    pub rating: i32,
}

/// Opening of a game, recognised by position (transpositions included).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewOpening {
    /// ECO code (`D02`).
    pub eco: String,
    /// Full name (`Queen's Pawn Game: London System`).
    pub name: String,
    /// The name before its first `:` (`Queen's Pawn Game`).
    pub family: String,
    /// The name after it (`London System`).
    pub variation: Option<String>,
    /// Move (ply) at which the game reached the named position.
    pub ply: u32,
    /// Number of first moves that are book (theory).
    pub book_plies: u32,
}

/// A cached Chess.com game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewGameSummary {
    /// Identifier (Chess.com UUID).
    pub id: String,
    /// Page of the game on chess.com.
    pub url: String,
    /// End of the game, Unix seconds.
    #[ts(type = "number")]
    pub end_time: i64,
    /// Speed category.
    pub time_class: TimeClassView,
    /// Time control as Chess.com writes it (`180+2`, `1/86400`).
    pub time_control: String,
    /// Rated game.
    pub rated: bool,
    /// White player.
    pub white: ReviewPlayer,
    /// Black player.
    pub black: ReviewPlayer,
    /// `1-0`, `0-1`, `1/2-1/2` or `*`.
    pub result: String,
    /// Number of moves (plies).
    pub plies: u32,
    /// Opening, if the game reached a named position.
    pub opening: Option<ReviewOpening>,
    /// Whether an analysis is cached.
    pub analysed: bool,
}

/// A move of a reviewed game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewMove {
    /// UCI notation.
    pub uci: String,
    /// SAN.
    pub san: String,
    /// Clock of the player after the move, in milliseconds (`[%clk]`).
    #[ts(type = "number", optional = nullable)]
    pub clock_ms: Option<u64>,
    /// The move is book: still in the opening theory.
    pub book: bool,
}

/// One of the best lines of a position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewLine {
    /// Score, from White's point of view.
    pub score: ScoreView,
    /// Depth of the search.
    pub depth: u32,
    /// The line in UCI notation.
    pub uci: Vec<String>,
    /// The line in SAN.
    pub san: Vec<String>,
}

/// Analysis of one position (the start, then after each move).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewPosition {
    /// FEN of the position.
    pub fen: String,
    /// Evaluation, from White's point of view (the best line's).
    pub score: ScoreView,
    /// White's winning chances, from 0 to 100 (the evaluation graph).
    pub win: f64,
    /// The side to move is checkmated (then `score` is a mate in 0).
    pub checkmated: bool,
    /// The best lines, best first (none when the game is over).
    pub lines: Vec<ReviewLine>,
}

/// Class of a move, from the best to the worst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ReviewClass {
    /// A sound sacrifice.
    Brilliant,
    /// The only good move.
    Great,
    /// The engine's best move, or as good.
    Best,
    /// Nearly as good as the best.
    Excellent,
    /// A sound move.
    Good,
    /// Opening theory.
    Book,
    /// Some advantage lost.
    Inaccuracy,
    /// A clear advantage lost.
    Mistake,
    /// A winning chance offered by the opponent, let go.
    Miss,
    /// A decisive advantage lost.
    Blunder,
}

/// How good a move was.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewVerdict {
    /// Its class.
    pub class: ReviewClass,
    /// Winning chances (0-100) of the player before the move, with the best move.
    pub win_before: f64,
    /// Their winning chances after it.
    pub win_after: f64,
    /// Accuracy of the move, from 0 to 100.
    pub accuracy: f64,
    /// Centipawns lost.
    pub cp_loss: i32,
}

/// Number of moves of each class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewClassCounts {
    /// Brilliant moves.
    pub brilliant: u32,
    /// Great moves.
    pub great: u32,
    /// Best moves.
    pub best: u32,
    /// Excellent moves.
    pub excellent: u32,
    /// Good moves.
    pub good: u32,
    /// Book moves.
    pub book: u32,
    /// Inaccuracies.
    pub inaccuracy: u32,
    /// Mistakes.
    pub mistake: u32,
    /// Misses.
    pub miss: u32,
    /// Blunders.
    pub blunder: u32,
}

/// How one player played a game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export, optional_fields = nullable)]
pub struct ReviewPlayerStats {
    /// Moves played.
    pub moves: u32,
    /// Accuracy over the game, from 0 to 100 (as on Lichess).
    pub accuracy: Option<f64>,
    /// Average centipawn loss.
    pub acpl: Option<f64>,
    /// Moves of each class.
    pub counts: ReviewClassCounts,
}

/// The engine analysis of a game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewAnalysis {
    /// Engine that analysed it.
    pub engine: String,
    /// Search budget of each position (`depth 16`, `200 ms`…).
    pub budget: String,
    /// Lines asked per position.
    pub multipv: u32,
    /// One entry per position: one more than the moves.
    pub positions: Vec<ReviewPosition>,
    /// One verdict per move.
    pub moves: Vec<ReviewVerdict>,
    /// How White played.
    pub white: ReviewPlayerStats,
    /// How Black played.
    pub black: ReviewPlayerStats,
}

/// `GET /api/review/games/{id}`: a game with its analysis, if any.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewGameView {
    /// The game.
    pub game: ReviewGameSummary,
    /// Starting position (FEN).
    pub start_fen: String,
    /// Moves played.
    pub moves: Vec<ReviewMove>,
    /// The latest analysis.
    pub analysis: Option<ReviewAnalysis>,
}
