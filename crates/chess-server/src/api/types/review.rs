//! Types of the Chess.com reviews (`/api/review/*`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

use super::{ScoreView, Side};

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

/// Wins, draws and losses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewRecord {
    /// Games won.
    pub wins: u32,
    /// Games drawn.
    pub draws: u32,
    /// Games lost.
    pub losses: u32,
}

/// A move of one of the games, shown as an example.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewGameRef {
    /// The game.
    pub game_id: String,
    /// Index of the move: the game page shows the position after it with
    /// `?ply=` one more.
    pub ply: u32,
    /// The move with its number: `12... Nf3`.
    pub label: String,
    /// Its class.
    pub class: ReviewClass,
    /// Winning chances it lost.
    pub loss: f64,
}

/// An opening family played with one colour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewOpeningStats {
    /// Family (`Sicilian Defense`).
    pub family: String,
    /// Its most frequent ECO code.
    pub eco: String,
    /// Results.
    pub record: ReviewRecord,
    /// Mean accuracy of the player in these games.
    pub accuracy: Option<f64>,
    /// The most recent of these games.
    pub last_game: String,
}

/// A move played in a recurring position, and how often.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewPlayedMove {
    /// The move (SAN).
    pub san: String,
    /// Times played.
    pub count: u32,
}

/// A position the player reached in several games and went wrong in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export, optional_fields = nullable)]
pub struct ReviewLineStats {
    /// Side of the player.
    pub color: Side,
    /// Moves (SAN) from the start to the position.
    pub moves: Vec<String>,
    /// The position.
    pub fen: String,
    /// Games that reached it.
    pub games: u32,
    /// Times the move played there was an inaccuracy or worse.
    pub errors: u32,
    /// The engine's move (SAN): the one to learn.
    pub best: Option<String>,
    /// Moves played there, most frequent first.
    pub played: Vec<ReviewPlayedMove>,
    /// Winning chances lost there in all.
    pub lost: f64,
    /// The worst of these moves.
    pub examples: Vec<ReviewGameRef>,
}

/// Phase of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ReviewPhase {
    /// The first moves.
    Opening,
    /// Between the two.
    Middlegame,
    /// Few pieces left.
    Endgame,
}

/// The player's moves in one phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewPhaseStats {
    /// The phase.
    pub phase: ReviewPhase,
    /// Moves played in it.
    pub moves: u32,
    /// Mean accuracy of these moves.
    pub accuracy: Option<f64>,
    /// Average centipawn loss.
    pub acpl: Option<f64>,
    /// Moves of each class.
    pub counts: ReviewClassCounts,
    /// Winning chances lost by its mistakes, misses and blunders.
    pub lost: f64,
    /// Mean thinking time per move, in seconds.
    pub seconds_per_move: Option<f64>,
}

/// Use of the clock.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewTimeStats {
    /// Games with clocks.
    pub games: u32,
    /// Mean thinking time per move, in seconds.
    pub seconds_per_move: f64,
    /// Games where the player was short of time at some point.
    pub time_trouble_games: u32,
    /// Moves played short of time.
    pub low_moves: u32,
    /// Mistakes, misses and blunders among them.
    pub low_errors: u32,
    /// Moves played with enough time.
    pub normal_moves: u32,
    /// Mistakes, misses and blunders among them.
    pub normal_errors: u32,
    /// Games lost on time.
    pub timeouts: u32,
}

/// Won positions converted, lost ones saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewConversion {
    /// Games where the player was winning at some point.
    pub winning: u32,
    /// Of those, games won.
    pub converted: u32,
    /// Games where the player was lost at some point.
    pub losing: u32,
    /// Of those, games drawn or won.
    pub saved: u32,
}

/// Tactical motif of a mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReviewMotif {
    /// A mate on the board, let go.
    MissedMate,
    /// A move that lets the opponent mate.
    AllowedMate,
    /// Material left to be taken.
    HangingPiece,
    /// A capture winning material, not played.
    MissedMaterial,
    /// A fork, not played.
    MissedFork,
    /// A move that lets the opponent fork.
    AllowedFork,
    /// None of these.
    Other,
}

/// The mistakes of one motif.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewMotifStats {
    /// The motif.
    pub motif: ReviewMotif,
    /// Mistakes, misses and blunders with it.
    pub count: u32,
    /// Winning chances they lost.
    pub lost: f64,
    /// The worst of them.
    pub examples: Vec<ReviewGameRef>,
}

/// What a weakness is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
#[ts(export)]
pub enum ReviewWeaknessKind {
    /// More errors than usual in this phase.
    Phase(ReviewPhase),
    /// Errors of this motif.
    Motif(ReviewMotif),
    /// Errors when short of time.
    TimeTrouble,
    /// Won positions not won.
    Conversion,
    /// Errors in a recurring position (index in `lines`).
    Line(u32),
}

/// One of the main weaknesses of the player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewWeakness {
    /// What it is about.
    pub kind: ReviewWeaknessKind,
    /// Winning chances it costs per game, on average.
    pub lost_per_game: f64,
    /// Moves (games, for the conversion) concerned.
    pub count: u32,
    /// Moves showing it.
    pub examples: Vec<ReviewGameRef>,
}

/// One game of the profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export, optional_fields = nullable)]
pub struct ReviewProfileGame {
    /// The game.
    pub id: String,
    /// Side of the player.
    pub color: Side,
    /// Their accuracy.
    pub accuracy: Option<f64>,
    /// Their average centipawn loss.
    pub acpl: Option<f64>,
}

/// Openings of the player, with each colour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ReviewRepertoire {
    /// With White, most played first.
    pub white: Vec<ReviewOpeningStats>,
    /// With Black.
    pub black: Vec<ReviewOpeningStats>,
}

/// `GET /api/review/players/{username}/profile`: a player over their
/// analysed games.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ReviewProfile {
    /// The player.
    pub username: String,
    /// Analysed games counted.
    pub games: u32,
    /// Games kept by the filter but not analysed yet.
    pub unanalysed: u32,
    /// Games left out: unreadable, or not played by them.
    pub skipped: u32,
    /// Results.
    pub record: ReviewRecord,
    /// Mean accuracy over the games.
    pub accuracy: Option<f64>,
    /// Average centipawn loss over all their moves.
    pub acpl: Option<f64>,
    /// Their moves.
    pub moves: u32,
    /// Their moves of each class.
    pub counts: ReviewClassCounts,
    /// Their rating after the most recent game.
    pub rating: Option<i32>,
    /// Mean rating of their opponents.
    pub opponents: Option<f64>,
    /// Their openings.
    pub repertoire: ReviewRepertoire,
    /// Recurring positions where they went wrong, costliest first.
    pub lines: Vec<ReviewLineStats>,
    /// Their moves by phase.
    pub phases: Vec<ReviewPhaseStats>,
    /// Their use of the clock.
    pub time: Option<ReviewTimeStats>,
    /// Conversion of won positions, defence of lost ones.
    pub conversion: ReviewConversion,
    /// Their mistakes by motif, most frequent first.
    pub motifs: Vec<ReviewMotifStats>,
    /// Their three main weaknesses, costliest first.
    pub weaknesses: Vec<ReviewWeakness>,
    /// Each game, most recent first.
    pub per_game: Vec<ReviewProfileGame>,
}
