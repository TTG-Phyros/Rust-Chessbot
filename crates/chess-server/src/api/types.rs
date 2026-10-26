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
    /// Network being trained, or the last one trained.
    pub run: Option<String>,
    /// Why the last run failed, or why the daemon is unreachable.
    pub error: Option<String>,
    /// Id of the tournament being played.
    pub tournament: Option<String>,
    /// Stockfish the daemon found for the `stockfish-1` to `stockfish-10`
    /// bots (`Stockfish 19`), absent if none.
    pub stockfish: Option<String>,
    /// When the running training stops on its own (Unix seconds), if it has a time limit.
    #[ts(type = "number | null", optional = nullable)]
    pub deadline: Option<u64>,
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
    /// Network shown: the requested one, else the one training (or last trained).
    pub network: Option<String>,
    /// Status of the network shown, while it trains or after its last run.
    pub status: Option<TrainingStatus>,
    /// Loss curve of the network shown.
    pub loss: Vec<LossPoint>,
    /// Elo curve of the network shown.
    pub elo: Vec<EloPoint>,
    /// Every network.
    pub networks: Vec<NetworkSummary>,
    /// Evaluation used by the engine right now.
    pub engine: EngineInfo,
}

/// Request of `POST /api/training/start`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct StartTrainingRequest {
    /// Network to train (default: the last one trained).
    pub network: Option<String>,
    /// Start a new network instead of resuming the checkpoint.
    #[serde(default)]
    #[ts(optional = false)]
    pub fresh: bool,
    /// Stop after this many superbatches.
    #[ts(type = "number | null", optional = nullable)]
    pub superbatches: Option<u64>,
    /// Stop cleanly after this many minutes (1 to 10 080); absent: no time limit.
    pub max_minutes: Option<u32>,
}

/// Request of `POST /api/training/timer`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct TrainingTimerRequest {
    /// Stop the running training this many minutes from now (1 to 10 080);
    /// absent: no time limit.
    pub minutes: Option<u32>,
}

/// Hyperparameters of a network (`data/runs/<name>.json`, written by
/// `chess-train`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkParams {
    /// Nodes per move in self-play: stronger games, generated more slowly.
    #[ts(type = "number")]
    pub selfplay_nodes: u64,
    /// AdamW learning rate.
    pub learning_rate: f64,
    /// The learning rate is multiplied by `lr_drop_factor` every this many superbatches.
    #[ts(type = "number")]
    pub lr_drop_every: u64,
    /// Learning rate multiplier.
    pub lr_drop_factor: f64,
    /// Weight of the game result in the target (the rest is the search score).
    pub wdl: f64,
    /// Positions per optimizer step.
    #[ts(type = "number")]
    pub batch_size: u64,
    /// Steps per superbatch.
    #[ts(type = "number")]
    pub batches_per_superbatch: u64,
    /// Superbatches to train; 0 = until stopped.
    #[ts(type = "number")]
    pub superbatches: u64,
    /// Export (and measure) the network every this many superbatches.
    #[ts(type = "number")]
    pub export_every: u64,
    /// Positions to generate before training starts.
    #[ts(type = "number")]
    pub min_positions: u64,
    /// Most times each position is trained on, on average (0 = no limit).
    #[ts(type = "number")]
    pub max_reuse: u64,
    /// Games of each measurement against the classical evaluation.
    pub eval_games: u32,
    /// Other networks whose self-play games this one also trains on.
    pub datasets_from: Vec<String>,
}

/// A measured snapshot, among the best of its network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RankedSnapshot {
    /// Superbatch of the snapshot.
    #[ts(type = "number")]
    pub superbatch: u64,
    /// Elo against the classical evaluation.
    pub elo: f64,
    /// Half-width of the 95 % interval of `elo`.
    pub elo_error: f64,
    /// Games of the measurement.
    pub games: u32,
    /// Unix seconds.
    #[ts(type = "number")]
    pub measured_at: u64,
}

/// A network and where its training stands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct NetworkSummary {
    /// Name of the network.
    pub name: String,
    /// Its hyperparameters (absent for a network never started from the daemon).
    pub params: Option<NetworkParams>,
    /// Superbatches of its checkpoint.
    #[ts(type = "number | null", optional = nullable)]
    pub superbatch: Option<u64>,
    /// Its best snapshots, best first.
    pub best: Vec<RankedSnapshot>,
    /// Whether it has exported a network yet.
    pub has_weights: bool,
}

/// Request of `POST /api/networks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateNetworkRequest {
    /// 1 to 32 letters, digits, `-` or `_`.
    pub name: String,
    /// Hyperparameters.
    pub params: NetworkParams,
}

/// A saved network, ready to play tournaments (`data/bots/<name>.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct BotInfo {
    /// Name of the bot.
    pub name: String,
    /// Network it was saved from.
    pub network: String,
    /// Snapshot saved; absent: the latest export at the time.
    #[ts(type = "number | null", optional = nullable)]
    pub superbatch: Option<u64>,
    /// Elo of the snapshot against the classical evaluation, if measured.
    pub elo: Option<f64>,
    /// Half-width of the 95 % interval of `elo`.
    pub elo_error: Option<f64>,
    /// Hyperparameters of the network it comes from.
    pub params: Option<NetworkParams>,
    /// Unix seconds.
    #[ts(type = "number")]
    pub created_at: u64,
}

/// Request of `POST /api/bots`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct SaveBotRequest {
    /// Name of the new bot.
    pub name: String,
    /// Network to save.
    pub network: String,
    /// Snapshot to save (one of the best); absent: the latest export.
    #[ts(type = "number | null", optional = nullable)]
    pub superbatch: Option<u64>,
}

/// How long the bots (not Stockfish) think over each move of a tournament.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
#[ts(export)]
pub enum MoveLimit {
    /// Nodes searched (100 to 10 000 000): reproducible, whatever the load.
    Nodes(#[ts(type = "number")] u64),
    /// Search depth in plies (1 to 30).
    Depth(u32),
    /// Milliseconds per move (10 to 60 000).
    Time(#[ts(type = "number")] u64),
}

/// Request of `POST /api/tournaments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TournamentRequest {
    /// Bots taking part; `classic` is the classical evaluation (PeSTO).
    pub bots: Vec<String>,
    /// Games of each pairing (colors alternate).
    pub games_per_pair: u32,
    /// How long the bots think per move, the same for all (Stockfish plays at its level).
    pub limit: MoveLimit,
    /// Worker threads; 0 = all cores.
    pub threads: u32,
    /// The pairings to play (either way round); absent: every pair of bots.
    #[ts(optional = nullable)]
    pub pairings: Option<Vec<PlannedPairing>>,
}

/// Two bots of a tournament that play each other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct PlannedPairing {
    /// A bot of the tournament.
    pub a: String,
    /// Another bot of the tournament.
    pub b: String,
}

/// Answer of `POST /api/tournaments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct TournamentStarted {
    /// Id of the new tournament.
    pub id: String,
}

/// State of a tournament.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TournamentState {
    /// Being played.
    Running,
    /// Every pairing played.
    Finished,
    /// Stopped before the end (on request, or the training service stopped).
    Stopped,
    /// Ended by an error.
    Failed,
}

/// Result of bot `a` against bot `b`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Pairing {
    /// First bot.
    pub a: String,
    /// Second bot.
    pub b: String,
    /// Games won by `a`.
    pub wins: u32,
    /// Draws.
    pub draws: u32,
    /// Games lost by `a`.
    pub losses: u32,
    /// Elo of `a` over `b`.
    pub elo: f64,
    /// Half-width of the 95 % interval of `elo`.
    pub elo_error: f64,
}

/// A line of the tournament table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Standing {
    /// Bot.
    pub bot: String,
    /// Games played.
    pub games: u32,
    /// 1 per win, ½ per draw.
    pub points: f64,
    /// Elo fitted on every game: relative to `classic` when it takes part,
    /// to the field's average otherwise.
    pub elo: f64,
}

/// A round-robin tournament between bots (`data/tournaments/<id>.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields = nullable)]
pub struct Tournament {
    /// Id of the tournament.
    pub id: String,
    /// Where it stands.
    pub state: TournamentState,
    /// Why it failed.
    pub error: Option<String>,
    /// Unix seconds.
    #[ts(type = "number")]
    pub created_at: u64,
    /// Unix seconds.
    #[ts(type = "number")]
    pub updated_at: u64,
    /// Bots taking part.
    pub bots: Vec<String>,
    /// Games of each pairing.
    pub games_per_pair: u32,
    /// How long the bots think per move.
    pub limit: MoveLimit,
    /// Every pairing, in playing order.
    pub pairings: Vec<Pairing>,
    /// Best first.
    pub standings: Vec<Standing>,
}

/// Response of `GET /api/arena`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, ToSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArenaView {
    /// Training control daemon (which plays the tournaments).
    pub controller: ControllerView,
    /// Saved bots, newest first.
    pub bots: Vec<BotInfo>,
    /// Latest tournaments, newest first.
    pub tournaments: Vec<Tournament>,
}
