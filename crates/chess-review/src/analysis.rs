//! Evaluation of every position of a game.

use chess_core::{Move, Position};
use serde::{Deserialize, Serialize};

use crate::engine::{AnalysisEngine, Line, Score, SearchSettings};

/// Analysis of one position of a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionEval {
    /// The best lines, best first; empty when the game is over (checkmate
    /// or stalemate on the board).
    pub lines: Vec<Line>,
    /// Whether the side to move is checkmated (only when `lines` is empty).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub checkmated: bool,
}

impl PositionEval {
    /// Score for the side to move: the best line's, mate 0 when
    /// checkmated, a draw when stalemated.
    #[must_use]
    pub fn score(&self) -> Score {
        match self.lines.first() {
            Some(line) => line.score,
            None if self.checkmated => Score::Mate(0),
            None => Score::Cp(0),
        }
    }

    /// Best move, in UCI notation.
    #[must_use]
    pub fn best_move(&self) -> Option<&str> {
        self.lines.first()?.pv.first().map(String::as_str)
    }
}

/// Analysis of a whole game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameAnalysis {
    /// Engine that analysed it ([`AnalysisEngine::id`]).
    pub engine: String,
    /// Settings of each search.
    pub settings: SearchSettings,
    /// One entry per position: the start, then after each move (one more
    /// than the moves).
    pub positions: Vec<PositionEval>,
}

/// Analyses the start position and the position after each of `moves`
/// with `engine`. `progress` is called with the number of positions done
/// and their total after each one; returning `false` cancels the analysis
/// (then `Ok(None)`).
///
/// # Errors
///
/// When the engine fails, or a move is illegal.
pub fn analyse_game(
    engine: &mut dyn AnalysisEngine,
    start: &Position,
    moves: &[Move],
    settings: &SearchSettings,
    mut progress: impl FnMut(usize, usize) -> bool,
) -> anyhow::Result<Option<GameAnalysis>> {
    let start = start.without_history();
    let total = moves.len() + 1;
    let mut position = start.clone();
    let mut positions = Vec::with_capacity(total);
    engine.new_game()?;
    for ply in 0..total {
        if ply > 0 {
            position.play(moves[ply - 1])?;
        }
        let eval = if position.legal_moves().is_empty() {
            PositionEval {
                lines: Vec::new(),
                checkmated: position.in_check(),
            }
        } else {
            PositionEval {
                lines: engine.analyse(&start, &moves[..ply], settings)?,
                checkmated: false,
            }
        };
        positions.push(eval);
        if !progress(ply + 1, total) {
            return Ok(None);
        }
    }
    Ok(Some(GameAnalysis {
        engine: engine.id().to_owned(),
        settings: *settings,
        positions,
    }))
}

#[cfg(test)]
mod tests {
    use chess_core::pgn::parse_pgn;

    use super::*;

    /// Scores every position with the number of legal moves, so that tests
    /// can check what was asked without a real engine.
    struct CountingEngine {
        calls: usize,
    }

    impl AnalysisEngine for CountingEngine {
        fn id(&self) -> &str {
            "counting"
        }

        fn new_game(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        fn analyse(
            &mut self,
            start: &Position,
            moves: &[Move],
            settings: &SearchSettings,
        ) -> anyhow::Result<Vec<Line>> {
            self.calls += 1;
            let mut position = start.clone();
            for &mv in moves {
                position.play(mv)?;
            }
            let legal = position.legal_moves();
            Ok(legal
                .iter()
                .take(settings.multipv)
                .map(|mv| Line {
                    score: Score::Cp(i32::try_from(legal.len()).unwrap_or(0)),
                    depth: 1,
                    pv: vec![mv.to_string()],
                })
                .collect())
        }
    }

    const FOOLS_MATE: &str = "1. f3 e5 2. g4 Qh4# 0-1";

    #[test]
    fn every_position_is_evaluated() {
        let game = parse_pgn(FOOLS_MATE).unwrap().remove(0);
        let mut engine = CountingEngine { calls: 0 };
        let settings = SearchSettings::default();
        let mut reports = Vec::new();
        let analysis = analyse_game(
            &mut engine,
            game.start(),
            game.moves(),
            &settings,
            |d, t| {
                reports.push((d, t));
                true
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(analysis.positions.len(), 5);
        assert_eq!(analysis.engine, "counting");
        // The final checkmate is not sent to the engine.
        assert_eq!(engine.calls, 4);
        assert_eq!(analysis.positions[0].score(), Score::Cp(20));
        assert_eq!(analysis.positions[0].lines.len(), 3);
        let mated = &analysis.positions[4];
        assert!(mated.checkmated);
        assert_eq!(mated.score(), Score::Mate(0));
        assert_eq!(mated.best_move(), None);
        assert_eq!(reports.last(), Some(&(5, 5)));
    }

    #[test]
    fn analyses_can_be_cancelled() {
        let game = parse_pgn(FOOLS_MATE).unwrap().remove(0);
        let mut engine = CountingEngine { calls: 0 };
        let result = analyse_game(
            &mut engine,
            game.start(),
            game.moves(),
            &SearchSettings::default(),
            |done, _| done < 2,
        )
        .unwrap();
        assert_eq!(result, None);
        assert_eq!(engine.calls, 2);
    }

    #[test]
    fn stalemate_is_a_draw() {
        let start = Position::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        let mut engine = CountingEngine { calls: 0 };
        let analysis = analyse_game(
            &mut engine,
            &start,
            &[],
            &SearchSettings::default(),
            |_, _| true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(analysis.positions[0].score(), Score::Cp(0));
        assert!(!analysis.positions[0].checkmated);
        assert_eq!(engine.calls, 0);
    }

    /// Our engine (always available) finds a mate in one.
    #[cfg(feature = "native")]
    #[test]
    fn native_engine_finds_the_mate() {
        let mut engine = crate::engine::NativeEngine::new(None, 4).unwrap();
        let start =
            Position::from_fen("r1bqkbnr/pppp1ppp/2n5/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4")
                .unwrap();
        let settings = SearchSettings {
            budget: crate::Budget::Depth(4),
            multipv: 2,
        };
        let lines = engine.analyse(&start, &[], &settings).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].pv[0], "h5f7");
        assert_eq!(lines[0].score, Score::Mate(1));
        assert_eq!(engine.id(), "chess-bot classic");
    }

    /// Needs a Stockfish on the `PATH` (in the Docker images); skipped otherwise.
    #[cfg(feature = "stockfish")]
    #[test]
    fn stockfish_reviews_a_game() {
        let binary = std::path::Path::new("stockfish");
        if chess_stockfish::probe(binary).is_none() {
            return;
        }
        let spec = crate::EngineSpec::Stockfish {
            binary: binary.into(),
            threads: 1,
            hash_mb: 16,
        };
        let mut engine = spec.spawn().unwrap();
        let game = parse_pgn(FOOLS_MATE).unwrap().remove(0);
        let settings = SearchSettings {
            budget: crate::Budget::Depth(10),
            multipv: 2,
        };
        let analysis = analyse_game(
            engine.as_mut(),
            game.start(),
            game.moves(),
            &settings,
            |_, _| true,
        )
        .unwrap()
        .unwrap();
        assert!(analysis.engine.starts_with("Stockfish"));
        // Before 2. g4, Black is fine; after it, Black mates in one.
        assert_eq!(analysis.positions[3].score(), Score::Mate(1));
        assert_eq!(analysis.positions[3].best_move(), Some("d8h4"));
        assert_eq!(analysis.positions[3].lines.len(), 2);
        assert_eq!(analysis.positions[4].score(), Score::Mate(0));
    }
}
