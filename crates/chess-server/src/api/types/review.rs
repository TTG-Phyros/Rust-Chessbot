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
    /// Depth of each search.
    pub depth: Option<u32>,
    /// Time of each search, in milliseconds (instead of a depth).
    #[ts(type = "number", optional = nullable)]
    pub movetime_ms: Option<u64>,
    /// Best lines per position (1 to 5).
    pub multipv: Option<u32>,
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

/// A cached Chess.com game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewPosition {
    /// FEN of the position.
    pub fen: String,
    /// Evaluation, from White's point of view (the best line's).
    pub score: ScoreView,
    /// The side to move is checkmated (then `score` is a mate in 0).
    pub checkmated: bool,
    /// The best lines, best first (none when the game is over).
    pub lines: Vec<ReviewLine>,
}

/// The engine analysis of a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
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
}

/// `GET /api/review/games/{id}`: a game with its analysis, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
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
