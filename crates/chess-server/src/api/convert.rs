//! Conversions between `chess-core`/`chess-search` types and API types.

use chess_core::{Color, Move, Position};
use chess_search::{Score, SearchInfo};

use super::error::ApiError;
use super::types::{
    AnalysisInfo, DrawReason, GameRequest, GameStatus, MoveView, PositionView, ScoreView, Side,
};

/// Longest game accepted (in plies), to bound the work of one request.
pub(crate) const MAX_PLIES: usize = 2_000;

/// A game replayed by the engine.
pub(crate) struct Replayed {
    /// Position after the last move, with the moves as history (repetitions).
    pub(crate) position: Position,
    /// Moves in SAN.
    pub(crate) san: Vec<String>,
}

/// Replays `game`, rejecting an invalid FEN or the first illegal move.
pub(crate) fn replay(game: &GameRequest) -> Result<Replayed, ApiError> {
    let mut position = match game.fen.as_deref().map(str::trim) {
        Some(fen) if !fen.is_empty() => Position::from_fen(fen)
            .map_err(|err| ApiError::unprocessable(format!("invalid FEN: {err}")))?,
        _ => Position::startpos(),
    };
    if game.moves.len() > MAX_PLIES {
        return Err(ApiError::unprocessable(format!(
            "games are limited to {MAX_PLIES} moves"
        )));
    }
    let mut san = Vec::with_capacity(game.moves.len());
    for (index, text) in game.moves.iter().enumerate() {
        let mv = position.parse_uci_move(text).map_err(|err| {
            ApiError::unprocessable(format!("move {} ({text}): {err}", index + 1))
        })?;
        san.push(position.san(mv));
        position.make_move(mv);
    }
    Ok(Replayed { position, san })
}

pub(crate) const fn side(color: Color) -> Side {
    match color {
        Color::White => Side::White,
        Color::Black => Side::Black,
    }
}

/// `mv` (legal in `pos`) in every notation.
pub(crate) fn move_view(pos: &Position, mv: Move) -> MoveView {
    MoveView {
        uci: mv.to_string(),
        san: pos.san(mv),
        from: mv.from().to_string(),
        to: mv.to().to_string(),
        promotion: mv.promotion().map(|kind| kind.to_char().to_string()),
    }
}

pub(crate) fn status(pos: &Position) -> GameStatus {
    match pos.status() {
        chess_core::GameStatus::Ongoing => GameStatus::Ongoing,
        chess_core::GameStatus::Checkmate { winner } => GameStatus::Checkmate {
            winner: side(winner),
        },
        chess_core::GameStatus::Stalemate => GameStatus::Stalemate,
        chess_core::GameStatus::Draw(reason) => GameStatus::Draw {
            reason: match reason {
                chess_core::DrawReason::FiftyMoveRule => DrawReason::FiftyMoveRule,
                chess_core::DrawReason::ThreefoldRepetition => DrawReason::ThreefoldRepetition,
                chess_core::DrawReason::InsufficientMaterial => DrawReason::InsufficientMaterial,
            },
        },
    }
}

pub(crate) fn position_view(replayed: &Replayed) -> PositionView {
    let pos = &replayed.position;
    // The last move is described from the position before it.
    let last_move = pos.last_move().map(|mv| {
        let mut before = pos.clone();
        before.unmake_move();
        move_view(&before, mv)
    });
    PositionView {
        fen: pos.to_fen(),
        side_to_move: side(pos.side_to_move()),
        legal_moves: pos
            .legal_moves()
            .iter()
            .map(|&mv| move_view(pos, mv))
            .collect(),
        status: status(pos),
        check_square: pos
            .in_check()
            .then(|| pos.king_square(pos.side_to_move()).to_string()),
        last_move,
        history: replayed.san.clone(),
    }
}

/// Converts a score for `side_to_move` into White's point of view.
pub(crate) const fn white_score(score: Score, side_to_move: Color) -> ScoreView {
    let sign = side_to_move.sign();
    match score {
        Score::Centipawns(value) => ScoreView::Cp {
            value: value * sign,
        },
        Score::Mate(moves) => ScoreView::Mate {
            moves: moves * sign,
        },
    }
}

/// SAN of a principal variation starting from `pos`.
pub(crate) fn pv_san(pos: &Position, pv: &[Move]) -> Vec<String> {
    let mut pos = pos.without_history();
    pv.iter()
        .map_while(|&mv| {
            pos.is_legal(mv).then(|| {
                let san = pos.san(mv);
                pos.make_move(mv);
                san
            })
        })
        .collect()
}

pub(crate) fn analysis_info(id: u32, pos: &Position, info: &SearchInfo) -> AnalysisInfo {
    AnalysisInfo {
        id,
        depth: info.depth,
        seldepth: info.seldepth,
        score: white_score(info.score, pos.side_to_move()),
        nodes: info.nodes,
        nps: info.nps,
        time_ms: info.time.as_millis() as u64,
        hashfull: info.hashfull,
        pv: info.pv.iter().map(ToString::to_string).collect(),
        pv_san: pv_san(pos, &info.pv),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(fen: Option<&str>, moves: &[&str]) -> GameRequest {
        GameRequest {
            fen: fen.map(str::to_owned),
            moves: moves.iter().map(|m| (*m).to_owned()).collect(),
        }
    }

    #[test]
    fn replays_and_describes_a_game() {
        let replayed = replay(&game(
            None,
            &["e2e4", "e7e5", "d1h5", "b8c6", "f1c4", "g8f6", "h5f7"],
        ))
        .unwrap();
        let view = position_view(&replayed);
        assert_eq!(
            view.history,
            ["e4", "e5", "Qh5", "Nc6", "Bc4", "Nf6", "Qxf7#"]
        );
        assert_eq!(
            view.status,
            GameStatus::Checkmate {
                winner: Side::White
            }
        );
        assert_eq!(view.check_square.as_deref(), Some("e8"));
        assert!(view.legal_moves.is_empty());
        assert_eq!(view.last_move.map(|m| m.san).as_deref(), Some("Qxf7#"));
    }

    #[test]
    fn rejects_bad_games() {
        let err = replay(&game(None, &["e2e4", "e2e4"])).err().unwrap();
        assert!(
            err.message().starts_with("move 2 (e2e4)"),
            "{}",
            err.message()
        );
        assert!(replay(&game(Some("nonsense"), &[])).is_err());
        assert!(replay(&game(Some("  "), &[])).is_ok());
    }

    #[test]
    fn promotion_moves_carry_the_piece() {
        let replayed = replay(&game(Some("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1"), &[])).unwrap();
        let view = position_view(&replayed);
        let promo = view.legal_moves.iter().find(|m| m.uci == "b7b8n").unwrap();
        assert_eq!(promo.promotion.as_deref(), Some("n"));
        assert_eq!(promo.san, "b8=N");
    }

    #[test]
    fn scores_are_white_relative() {
        assert_eq!(
            white_score(Score::Centipawns(50), Color::Black),
            ScoreView::Cp { value: -50 }
        );
        assert_eq!(
            white_score(Score::Mate(2), Color::Black),
            ScoreView::Mate { moves: -2 }
        );
        assert_eq!(
            white_score(Score::Mate(-1), Color::White),
            ScoreView::Mate { moves: -1 }
        );
    }
}
