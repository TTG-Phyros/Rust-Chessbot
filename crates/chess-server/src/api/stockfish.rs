//! `POST /api/stockfish/compare`: what Stockfish plays in a position, at
//! several levels, to compare with our engine. Each level runs in its own
//! process, all of them in parallel.

use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::State;
use chess_stockfish::{Budget, LEVELS, Stockfish};

use super::convert::{move_view, replay, white_uci_score};
use super::error::ApiError;
use super::types::{
    EngineLimits, ErrorResponse, StockfishCompareRequest, StockfishCompareResponse, StockfishResult,
};
use crate::AppState;

/// Most levels one request may ask for.
const MAX_LEVELS: usize = 4;
/// Longest search accepted for a shared budget.
const MAX_MOVETIME: Duration = Duration::from_secs(10);
/// Deepest search accepted for a shared budget.
const MAX_DEPTH: u32 = 30;
/// Most nodes accepted for a shared budget.
const MAX_NODES: u64 = 50_000_000;

/// Stockfish's budget for limits shared with our engine (the first one set
/// among depth, nodes and time; the level's own budget if none).
fn budget(limits: Option<EngineLimits>) -> Budget {
    let Some(limits) = limits else {
        return Budget::Level;
    };
    if let Some(depth) = limits.depth {
        Budget::Depth(depth.clamp(1, MAX_DEPTH))
    } else if let Some(nodes) = limits.nodes {
        Budget::Nodes(nodes.clamp(1, MAX_NODES))
    } else if let Some(ms) = limits.movetime_ms {
        Budget::MoveTime(Duration::from_millis(ms.max(1)).min(MAX_MOVETIME))
    } else {
        Budget::Level
    }
}

/// Asks Stockfish levels for their move and evaluation of a position.
#[utoipa::path(
    post,
    path = "/api/stockfish/compare",
    tag = "engine",
    request_body = StockfishCompareRequest,
    responses(
        (status = 200, description = "One result per level", body = StockfishCompareResponse),
        (status = 422, description = "Invalid game or levels", body = ErrorResponse),
        (status = 503, description = "Stockfish is not installed", body = ErrorResponse),
    )
)]
pub(super) async fn compare(
    State(state): State<AppState>,
    Json(request): Json<StockfishCompareRequest>,
) -> Result<Json<StockfishCompareResponse>, ApiError> {
    let (binary, version) = state.engine.stockfish().cloned().ok_or_else(|| {
        ApiError::unavailable("Stockfish is not installed: set CHESS_SERVER_STOCKFISH")
    })?;
    let mut levels: Vec<u8> = Vec::with_capacity(request.levels.len());
    for &level in &request.levels {
        if !levels.contains(&level) {
            levels.push(level);
        }
    }
    if levels.is_empty() || levels.len() > MAX_LEVELS {
        return Err(ApiError::unprocessable(format!(
            "ask for 1 to {MAX_LEVELS} levels"
        )));
    }
    if let Some(level) = levels.iter().find(|level| !LEVELS.contains(level)) {
        return Err(ApiError::unprocessable(format!(
            "level {level} does not exist (1 to 10)"
        )));
    }
    let position = replay(&request.game)?.position;
    let budget = budget(request.budget);
    let fen = request
        .game
        .fen
        .clone()
        .filter(|fen| !fen.trim().is_empty());

    let searches = levels.iter().map(|&level| {
        let (binary, fen, moves) = (binary.clone(), fen.clone(), request.game.moves.clone());
        tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            let analysis =
                Stockfish::spawn(&binary, level)?.analyse(fen.as_deref(), &moves, budget)?;
            Ok::<_, anyhow::Error>((level, analysis, start.elapsed()))
        })
    });
    let mut results = Vec::with_capacity(levels.len());
    for search in searches.collect::<Vec<_>>() {
        let (level, analysis, elapsed) = search
            .await
            .map_err(|err| ApiError::internal(format!("Stockfish failed: {err}")))?
            .map_err(|err| ApiError::internal(format!("Stockfish failed: {err:#}")))?;
        results.push(StockfishResult {
            level,
            best_move: position
                .parse_uci_move(&analysis.best_move)
                .ok()
                .map(|mv| move_view(&position, mv)),
            score: analysis
                .score
                .map(|score| white_uci_score(score, position.side_to_move())),
            depth: analysis.depth,
            time_ms: elapsed.as_millis() as u64,
        });
    }
    Ok(Json(StockfishCompareResponse { version, results }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_budgets_follow_our_limits() {
        assert_eq!(budget(None), Budget::Level);
        assert_eq!(budget(Some(EngineLimits::default())), Budget::Level);
        let depth = EngineLimits {
            depth: Some(99),
            ..EngineLimits::default()
        };
        assert_eq!(budget(Some(depth)), Budget::Depth(MAX_DEPTH));
        let time = EngineLimits {
            movetime_ms: Some(250),
            ..EngineLimits::default()
        };
        assert_eq!(
            budget(Some(time)),
            Budget::MoveTime(Duration::from_millis(250))
        );
    }
}
