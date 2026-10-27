//! Stateless game endpoints: describe a position, ask the engine for a
//! move, import and export PGN.

use axum::Json;
use axum::extract::State;
use chess_core::Position;
use chess_core::pgn::{Game, GameResult, parse_pgn};
use chess_eval::{EngineEval, Evaluator};
use chess_search::StopFlag;

use super::convert::{
    move_view, position_view, pv_lines, pv_san, replay, wdl_estimate, white_score,
};
use super::error::ApiError;
use super::types::{
    EngineMoveRequest, EngineMoveResponse, ErrorResponse, EvalExplanation, ExplainRequest,
    GameRequest, PgnExportRequest, PgnExportResponse, PgnImportRequest, PgnImportResponse, PgnTag,
    PieceContribution, PositionView,
};
use crate::AppState;
use crate::engine::move_limits;

/// Largest PGN accepted by the import endpoint.
const MAX_PGN_BYTES: usize = 1024 * 1024;

/// Replays a game and describes the resulting position: legal moves (with
/// SAN), check, status, move history.
#[utoipa::path(
    post,
    path = "/api/position",
    tag = "game",
    request_body = GameRequest,
    responses(
        (status = 200, description = "The position after the moves", body = PositionView),
        (status = 422, description = "Invalid FEN or illegal move", body = ErrorResponse),
    )
)]
pub(super) async fn position(
    Json(game): Json<GameRequest>,
) -> Result<Json<PositionView>, ApiError> {
    Ok(Json(position_view(&replay(&game)?)))
}

/// Asks the engine for its move in a game.
#[utoipa::path(
    post,
    path = "/api/engine/move",
    tag = "engine",
    request_body = EngineMoveRequest,
    responses(
        (status = 200, description = "The engine's move", body = EngineMoveResponse),
        (status = 404, description = "No such bot", body = ErrorResponse),
        (status = 422, description = "Invalid FEN or illegal move", body = ErrorResponse),
    )
)]
pub(super) async fn engine_move(
    State(state): State<AppState>,
    Json(request): Json<EngineMoveRequest>,
) -> Result<Json<EngineMoveResponse>, ApiError> {
    let position = replay(&request.game)?.position;
    let side_to_move = position.side_to_move();
    let limits = move_limits(request.limits);
    let start = std::time::Instant::now();
    let result = state
        .engine
        .search(
            request.bot.as_deref(),
            position.clone(),
            limits,
            StopFlag::new(),
            |_| {},
        )
        .await?;
    Ok(Json(EngineMoveResponse {
        best_move: result.best_move.map(|mv| move_view(&position, mv)),
        score: white_score(result.score, side_to_move),
        depth: result.depth,
        nodes: result.nodes,
        time_ms: start.elapsed().as_millis() as u64,
        pv: pv_san(&position, &result.pv),
        lines: pv_lines(&position, &result.lines),
    }))
}

/// Takes the static evaluation of a position apart: raw output, win/draw/loss
/// estimate and, with a network, what each piece brings.
#[utoipa::path(
    post,
    path = "/api/engine/explain",
    tag = "engine",
    request_body = ExplainRequest,
    responses(
        (status = 200, description = "The evaluation, taken apart", body = EvalExplanation),
        (status = 404, description = "No such bot", body = ErrorResponse),
        (status = 422, description = "Invalid FEN or illegal move", body = ErrorResponse),
    )
)]
pub(super) async fn explain(
    State(state): State<AppState>,
    Json(request): Json<ExplainRequest>,
) -> Result<Json<EvalExplanation>, ApiError> {
    let position = replay(&request.game)?.position;
    let eval = state.engine.evaluator_for(request.bot.as_deref())?;
    // White's point of view: the evaluation is for the side to move.
    let sign = position.side_to_move().sign();
    let static_cp = eval.evaluate(&position) * sign;
    let pieces = match &eval {
        EngineEval::Nnue(network) => network
            .weights()
            .piece_contributions(&position)
            .into_iter()
            .map(|(square, piece, cp)| PieceContribution {
                square: square.to_string(),
                piece: piece.to_fen_char().to_string(),
                cp: cp * sign,
            })
            .collect(),
        EngineEval::Classic(_) => Vec::new(),
    };
    Ok(Json(EvalExplanation {
        evaluator: eval.name().to_owned(),
        static_cp,
        wdl: wdl_estimate(static_cp),
        pieces,
    }))
}

/// Reads the first game of a PGN text.
#[utoipa::path(
    post,
    path = "/api/pgn/import",
    tag = "game",
    request_body = PgnImportRequest,
    responses(
        (status = 200, description = "The first game", body = PgnImportResponse),
        (status = 422, description = "Invalid PGN", body = ErrorResponse),
    )
)]
pub(super) async fn pgn_import(
    Json(request): Json<PgnImportRequest>,
) -> Result<Json<PgnImportResponse>, ApiError> {
    if request.pgn.len() > MAX_PGN_BYTES {
        return Err(ApiError::unprocessable("PGN larger than 1 MiB"));
    }
    let games = parse_pgn(&request.pgn).map_err(|err| ApiError::unprocessable(err.to_string()))?;
    let game = games
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::unprocessable("no game found"))?;
    let start = game.start();
    let fen = (*start != Position::startpos()).then(|| start.to_fen());
    Ok(Json(PgnImportResponse {
        game: GameRequest {
            fen,
            moves: game.moves().iter().map(ToString::to_string).collect(),
        },
        tags: game
            .tags()
            .iter()
            .map(|(name, value)| PgnTag {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        result: game.result().as_str().to_owned(),
    }))
}

/// Writes a game as PGN.
#[utoipa::path(
    post,
    path = "/api/pgn/export",
    tag = "game",
    request_body = PgnExportRequest,
    responses(
        (status = 200, description = "PGN text", body = PgnExportResponse),
        (status = 422, description = "Invalid game or result", body = ErrorResponse),
    )
)]
pub(super) async fn pgn_export(
    Json(request): Json<PgnExportRequest>,
) -> Result<Json<PgnExportResponse>, ApiError> {
    let replayed = replay(&request.game)?;
    let mut start = replayed.position.clone();
    while start.ply() > 0 {
        start.unmake_move();
    }
    let mut game = Game::from_position(&start);
    for mv in replayed.position.moves_played() {
        game.push(mv)
            .map_err(|err| ApiError::internal(err.to_string()))?;
    }
    for tag in &request.tags {
        game.set_tag(&tag.name, &tag.value);
    }
    let result = match request.result.as_deref() {
        Some(text) => GameResult::from_pgn(text)
            .ok_or_else(|| ApiError::unprocessable(format!("invalid result {text:?}")))?,
        None => GameResult::from_status(replayed.position.status()),
    };
    game.set_result(result);
    Ok(Json(PgnExportResponse { pgn: game.to_pgn() }))
}
