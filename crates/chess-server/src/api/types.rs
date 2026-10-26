//! Types exchanged with the web UI.
//!
//! Each type derives [`TS`]: `cargo test` (or `just gen-types`) writes its
//! TypeScript definition to `web/src/api/generated/`, so front and back can't
//! drift. REST types also derive [`ToSchema`] for the OpenAPI document.
//!
//! The API is stateless: a game is always sent as a starting position plus
//! the moves played, and the server replays and validates them. The UI never
//! decides on its own whether a move is legal.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

/// Response of `GET /api/health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct HealthResponse {
    /// Always `"ok"` when the server answers.
    pub status: String,
    /// Version of the server binary.
    pub version: String,
}

/// Error body of every failed request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ErrorResponse {
    /// Human-readable reason.
    pub error: String,
}

/// A game: starting position and moves played since.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct GameRequest {
    /// Starting position in FEN; the standard starting position if absent.
    pub fen: Option<String>,
    /// Moves played from the starting position, in UCI notation (`e2e4`, `e7e8q`).
    #[serde(default)]
    #[ts(optional = false)]
    pub moves: Vec<String>,
}

/// A color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Side {
    /// White.
    White,
    /// Black.
    Black,
}

/// A legal move, in every notation the UI needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct MoveView {
    /// UCI notation (`e7e8q`); what the API expects back.
    pub uci: String,
    /// Standard Algebraic Notation (`e8=Q+`).
    pub san: String,
    /// Origin square (`e7`).
    pub from: String,
    /// Destination square (`e8`).
    pub to: String,
    /// Promotion piece (`q`, `r`, `b`, `n`).
    pub promotion: Option<String>,
}

/// Why a game is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DrawReason {
    /// Fifty moves without capture or pawn move.
    FiftyMoveRule,
    /// Same position three times.
    ThreefoldRepetition,
    /// Neither side can mate.
    InsufficientMaterial,
}

/// State of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum GameStatus {
    /// The game goes on.
    Ongoing,
    /// The side to move is mated.
    Checkmate {
        /// Side that delivered mate.
        winner: Side,
    },
    /// The side to move has no legal move and is not in check.
    Stalemate,
    /// Drawn by rule.
    Draw {
        /// Which rule.
        reason: DrawReason,
    },
}

/// Everything the UI shows about a position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct PositionView {
    /// FEN of the current position.
    pub fen: String,
    /// Side to move.
    pub side_to_move: Side,
    /// Legal moves of the side to move.
    pub legal_moves: Vec<MoveView>,
    /// State of the game.
    pub status: GameStatus,
    /// Square of the king in check, if any.
    pub check_square: Option<String>,
    /// Last move played, if any.
    pub last_move: Option<MoveView>,
    /// Moves played from the starting position, in SAN.
    pub history: Vec<String>,
}

/// Score from White's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ScoreView {
    /// Advantage in centipawns (positive: White is better).
    Cp {
        /// Centipawns.
        value: i32,
    },
    /// Forced mate (positive: White mates).
    Mate {
        /// Moves until mate.
        moves: i32,
    },
}

/// How long the engine may think. With nothing set, it thinks one second.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct EngineLimits {
    /// Maximum depth in plies (at most 64).
    pub depth: Option<u32>,
    /// Maximum time in milliseconds (at most 60 000).
    #[ts(type = "number", optional = nullable)]
    pub movetime_ms: Option<u64>,
    /// Maximum number of nodes.
    #[ts(type = "number", optional = nullable)]
    pub nodes: Option<u64>,
}

/// Request of `POST /api/engine/move`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EngineMoveRequest {
    /// The game whose next move is wanted.
    pub game: GameRequest,
    /// Thinking limits.
    #[serde(default)]
    pub limits: EngineLimits,
}

/// The engine's choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct EngineMoveResponse {
    /// Best move; absent when the game is over.
    pub best_move: Option<MoveView>,
    /// Evaluation of the position.
    pub score: ScoreView,
    /// Depth reached.
    pub depth: u32,
    /// Nodes searched.
    #[ts(type = "number")]
    pub nodes: u64,
    /// Thinking time in milliseconds.
    #[ts(type = "number")]
    pub time_ms: u64,
    /// Expected continuation, in SAN.
    pub pv: Vec<String>,
}

/// A PGN tag pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct PgnTag {
    /// Tag name (`White`, `Event`, ...).
    pub name: String,
    /// Tag value.
    pub value: String,
}

/// Request of `POST /api/pgn/import`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct PgnImportRequest {
    /// PGN text; only its first game is imported.
    pub pgn: String,
}

/// The first game of the imported PGN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PgnImportResponse {
    /// The game, ready to be sent back to the other endpoints.
    pub game: GameRequest,
    /// Tags of the game (the result excluded).
    pub tags: Vec<PgnTag>,
    /// Recorded result: `1-0`, `0-1`, `1/2-1/2` or `*`.
    pub result: String,
}

/// Request of `POST /api/pgn/export`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct PgnExportRequest {
    /// The game to export.
    pub game: GameRequest,
    /// Tags to write (the Seven Tag Roster is always present).
    #[serde(default)]
    #[ts(optional = false)]
    pub tags: Vec<PgnTag>,
    /// Result to record; derived from the final position if absent.
    pub result: Option<String>,
}

/// Exported PGN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct PgnExportResponse {
    /// PGN text.
    pub pgn: String,
}

/// Message sent by the UI over `/ws`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ClientMessage {
    /// Round-trip check; answered with [`ServerMessage::Pong`].
    Ping {
        /// Echoed back in the reply.
        id: u32,
    },
    /// Starts analysing a position, replacing any analysis in progress.
    /// Without limits it runs until [`ClientMessage::Stop`] (10 minutes at most).
    Analyze {
        /// Chosen by the client and echoed in every reply, so that late
        /// messages of a replaced analysis can be ignored.
        id: u32,
        /// Position to analyse.
        game: GameRequest,
        /// Optional limits.
        #[serde(default)]
        limits: EngineLimits,
    },
    /// Stops the analysis in progress (its final result is still sent).
    Stop,
}

/// Progress of an analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnalysisInfo {
    /// Identifier of the analysis.
    pub id: u32,
    /// Completed depth.
    pub depth: u32,
    /// Deepest ply reached.
    pub seldepth: u32,
    /// Evaluation, from White's point of view.
    pub score: ScoreView,
    /// Nodes searched.
    #[ts(type = "number")]
    pub nodes: u64,
    /// Nodes per second.
    #[ts(type = "number")]
    pub nps: u64,
    /// Elapsed time in milliseconds.
    #[ts(type = "number")]
    pub time_ms: u64,
    /// Transposition table usage, in permille.
    pub hashfull: u32,
    /// Principal variation, in UCI.
    pub pv: Vec<String>,
    /// Principal variation, in SAN.
    pub pv_san: Vec<String>,
}

/// Message sent by the server over `/ws`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ServerMessage {
    /// First message of every connection.
    Hello {
        /// Version of the server binary.
        version: String,
    },
    /// Reply to [`ClientMessage::Ping`].
    Pong {
        /// Identifier of the ping.
        id: u32,
    },
    /// The client sent something the server could not handle.
    Error {
        /// Human-readable reason.
        message: String,
    },
    /// An analysis completed one more depth.
    Info {
        /// Search progress.
        info: AnalysisInfo,
    },
    /// An analysis ended (limit reached or stopped).
    Done {
        /// Identifier of the analysis.
        id: u32,
        /// Best move in UCI; absent when the game is over.
        #[ts(optional = nullable)]
        best_move: Option<String>,
    },
}

/// Which evaluation the engine uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct EngineInfo {
    /// `nnue` or `classic`.
    pub evaluator: String,
    /// Network file in use, if any.
    pub weights_file: Option<String>,
    /// Modification time of that file (Unix seconds).
    #[ts(type = "number | null", optional = nullable)]
    pub weights_modified: Option<u64>,
}

/// State of the training control daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ControllerState {
    /// The training service is not reachable (not started, or not configured).
    Unavailable,
    /// Ready to start a run.
    Idle,
    /// A run is in progress.
    Running,
    /// A run is finishing its current step.
    Stopping,
}

/// The training control daemon, as seen by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct ControllerView {
    /// State of the daemon.
    pub state: ControllerState,
    /// Name of the run it controls.
    pub run: Option<String>,
    /// Why the last run failed, or why the daemon is unreachable.
    pub error: Option<String>,
}

/// Latest state of a training run (from its `status.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct TrainingStatus {
    /// Name of the run.
    pub run: String,
    /// `selfplay`, `training`, `pipeline` or `finished`.
    pub phase: String,
    /// Device used for training (GPU name or CPU fallback).
    pub device: String,
    /// Unix seconds.
    #[ts(type = "number")]
    pub started_at: u64,
    /// Unix seconds.
    #[ts(type = "number")]
    pub updated_at: u64,
    /// Self-play games played by the current run.
    #[ts(type = "number")]
    pub games: u64,
    /// Positions generated by the current run.
    #[ts(type = "number")]
    pub positions: u64,
    #[ts(type = "number")]
    /// Superbatches trained (one checkpoint each).
    pub superbatch: u64,
    /// Training loss of the last superbatch.
    pub loss: Option<f64>,
    /// Current learning rate.
    pub learning_rate: Option<f64>,
    /// Training speed.
    pub positions_per_second: Option<f64>,
    #[ts(type = "number | null", optional = nullable)]
    /// GPU memory in use, in MiB (absent on CPU).
    pub vram_used_mb: Option<u64>,
    /// Latest exported network file.
    pub latest_weights: Option<String>,
    /// Elo of the latest network against the classical evaluation.
    pub elo: Option<f64>,
    /// Half-width of the 95 % interval of `elo`.
    pub elo_error: Option<f64>,
}

/// Loss after a superbatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LossPoint {
    #[ts(type = "number")]
    /// Superbatch number.
    pub superbatch: u64,
    /// Mean training loss over the superbatch.
    pub loss: f64,
    /// Learning rate used.
    pub learning_rate: f64,
    /// Training speed.
    pub positions_per_second: f64,
}

/// Elo of an exported network against the classical evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EloPoint {
    #[ts(type = "number")]
    /// Superbatch of the measured export.
    pub superbatch: u64,
    /// Elo difference (negative: weaker than the classical evaluation).
    pub elo: f64,
    /// Half-width of the 95 % confidence interval.
    pub error: f64,
    /// Games played for the measurement.
    pub games: u32,
}

/// Response of `GET /api/training`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct TrainingView {
    /// Training control daemon.
    pub controller: ControllerView,
    /// Absent until a run has written its status.
    pub status: Option<TrainingStatus>,
    /// Loss curve of the current run.
    pub loss: Vec<LossPoint>,
    /// Elo curve of the current run.
    pub elo: Vec<EloPoint>,
    /// Evaluation used by the engine right now.
    pub engine: EngineInfo,
}

/// Request of `POST /api/training/start`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct StartTrainingRequest {
    /// Start a new network instead of resuming the checkpoint.
    #[serde(default)]
    #[ts(optional = false)]
    pub fresh: bool,
    /// Stop after this many superbatches.
    #[ts(type = "number | null", optional = nullable)]
    pub superbatches: Option<u64>,
}
