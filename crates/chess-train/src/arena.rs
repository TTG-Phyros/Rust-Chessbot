//! Engine matches: two evaluators play pairs of games from the same random
//! opening with colors swapped, to measure their Elo difference, optionally
//! with a sequential probability ratio test (SPRT) that stops as soon as
//! the result is clear.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use anyhow::{Context, Result};
use chess_core::{Color, GameStatus, Position};
use chess_eval::EngineEval;
use chess_jobs::Sprt;
pub(crate) use chess_jobs::{MatchResult, Verdict};
use chess_search::{BENCH_POSITIONS, Score, SearchLimits, Searcher, StopFlag};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::rng::Rng;
use crate::stockfish::{self, Stockfish, UciScore};
use chess_dataset::Outcome;

/// Match parameters (`[arena]` section of the configuration).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ArenaConfig {
    /// Maximum number of games (rounded up to pairs).
    pub(crate) games: u32,
    /// Nodes searched per move by our engine, unless `limit` says otherwise.
    pub(crate) nodes: u64,
    /// How our engine limits each move's search (default: `nodes`).
    pub(crate) limit: Option<MoveLimit>,
    /// Worker threads; 0 = all cores.
    pub(crate) threads: usize,
    pub(crate) random_plies_min: u32,
    pub(crate) random_plies_max: u32,
    pub(crate) max_plies: u32,
    pub(crate) seed: u64,
    /// Stop early when the SPRT concludes.
    pub(crate) sprt: bool,
    /// Stockfish binary, for the `stockfish-N` contestants.
    pub(crate) stockfish: PathBuf,
    /// SPRT hypotheses (Elo of A over B) and error rates.
    pub(crate) elo0: f64,
    pub(crate) elo1: f64,
    pub(crate) alpha: f64,
    pub(crate) beta: f64,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            games: 200,
            nodes: 3_000,
            limit: None,
            threads: 0,
            random_plies_min: 6,
            random_plies_max: 10,
            max_plies: 300,
            seed: 0xA4E7A,
            sprt: false,
            stockfish: PathBuf::from("stockfish"),
            elo0: 0.0,
            elo1: 10.0,
            alpha: 0.05,
            beta: 0.05,
        }
    }
}

/// How long our engine thinks over each move of a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub(crate) enum MoveLimit {
    /// Nodes searched: reproducible, whatever the load of the machine.
    Nodes(u64),
    /// Completed iterations of the search (plies).
    Depth(u32),
    /// Milliseconds of thinking: depends on the speed of each evaluation.
    Time(u64),
}

impl MoveLimit {
    pub(crate) fn search_limits(self) -> SearchLimits {
        match self {
            Self::Nodes(nodes) => SearchLimits::nodes(nodes.max(1)),
            Self::Depth(depth) => SearchLimits::depth(depth.max(1)),
            Self::Time(ms) => SearchLimits {
                movetime: Some(std::time::Duration::from_millis(ms.max(1))),
                ..SearchLimits::default()
            },
        }
    }

    /// Why the limit is out of the accepted range, if it is.
    pub(crate) fn validate(self) -> Result<(), String> {
        let ok = match self {
            Self::Nodes(nodes) => (100..=10_000_000).contains(&nodes),
            Self::Depth(depth) => (1..=30).contains(&depth),
            Self::Time(ms) => (10..=60_000).contains(&ms),
        };
        if ok {
            return Ok(());
        }
        Err(match self {
            Self::Nodes(_) => "nodes per move must be between 100 and 10 000 000",
            Self::Depth(_) => "the depth must be between 1 and 30",
            Self::Time(_) => "the time per move must be between 10 ms and 60 s",
        }
        .into())
    }
}

impl std::fmt::Display for MoveLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nodes(nodes) => write!(f, "{nodes} nodes per move"),
            Self::Depth(depth) => write!(f, "depth {depth}"),
            Self::Time(ms) => write!(f, "{ms} ms per move"),
        }
    }
}

impl ArenaConfig {
    pub(crate) fn move_limit(&self) -> MoveLimit {
        self.limit.unwrap_or(MoveLimit::Nodes(self.nodes))
    }

    /// The SPRT of `elo0`, `elo1`, `alpha` and `beta`.
    pub(crate) fn sprt_test(&self) -> Sprt {
        Sprt {
            elo0: self.elo0,
            elo1: self.elo1,
            alpha: self.alpha,
            beta: self.beta,
        }
    }
}

/// Counts a game of A (playing `a_color`) in `result`.
fn record(result: &mut MatchResult, outcome: Outcome, a_color: Color) {
    match (outcome, a_color) {
        (Outcome::Draw, _) => result.draws += 1,
        (Outcome::WhiteWins, Color::White) | (Outcome::BlackWins, Color::Black) => result.wins += 1,
        _ => result.losses += 1,
    }
}

/// Nodes per second of our search with `eval`: one thread, `nodes` nodes
/// on each bench position from a cleared state, best of two runs (the
/// first also warms the caches up).
pub(crate) fn measure_nps(eval: &EngineEval, nodes: u64) -> u64 {
    let mut searcher = Searcher::new(eval.clone(), StopFlag::new());
    let positions: Vec<Position> = BENCH_POSITIONS
        .iter()
        .filter_map(|fen| Position::from_fen(fen).ok())
        .collect();
    (0..2)
        .map(|_| {
            let start = Instant::now();
            let mut searched = 0;
            for pos in &positions {
                searcher.clear();
                searched += searcher
                    .search(pos, &SearchLimits::nodes(nodes), |_| {})
                    .nodes;
            }
            (searched as f64 / start.elapsed().as_secs_f64().max(1e-6)) as u64
        })
        .max()
        .unwrap_or(0)
}

/// Loads an evaluator: `classic`, or the path of a `.nnue` file.
pub(crate) fn load_evaluator(spec: &str) -> Result<EngineEval> {
    if spec.eq_ignore_ascii_case("classic") {
        return Ok(EngineEval::default());
    }
    let bytes = std::fs::read(spec).with_context(|| format!("cannot read {spec}"))?;
    EngineEval::from_nnue_bytes(&bytes).with_context(|| format!("{spec} is not a valid network"))
}

/// A player of a match.
#[derive(Debug, Clone)]
pub(crate) enum Contestant {
    /// Our engine with this evaluation, within the match's move limit.
    Engine(EngineEval),
    /// Stockfish at a level (its own depth and time per move).
    Stockfish { binary: PathBuf, level: u8 },
}

impl Contestant {
    /// `classic`, `stockfish-N` (with the Stockfish of `config`), or a `.nnue` file.
    pub(crate) fn load(spec: &str, config: &ArenaConfig) -> Result<Self> {
        match stockfish::level_of(spec) {
            Some(level) => Ok(Self::Stockfish {
                binary: config.stockfish.clone(),
                level,
            }),
            None => load_evaluator(spec).map(Self::Engine),
        }
    }

    pub(crate) fn name(&self) -> String {
        match self {
            Self::Engine(eval) => eval.name().to_owned(),
            Self::Stockfish { level, .. } => stockfish::bot_name(*level),
        }
    }

    fn seat(&self) -> Result<Seat> {
        Ok(match self {
            Self::Engine(eval) => {
                Seat::Engine(Box::new(Searcher::new(eval.clone(), StopFlag::new())))
            }
            Self::Stockfish { binary, level } => Seat::Stockfish(Stockfish::spawn(binary, *level)?),
        })
    }
}

/// A contestant ready to play, kept by a worker thread from game to game.
enum Seat {
    Engine(Box<Searcher<EngineEval>>),
    Stockfish(Stockfish),
}

/// Score of a decided game, in centipawns.
const MATE_SCORE: i32 = 30_000;

impl Seat {
    fn new_game(&mut self) -> Result<()> {
        match self {
            Self::Engine(searcher) => {
                searcher.clear();
                Ok(())
            }
            Self::Stockfish(stockfish) => stockfish.new_game(),
        }
    }

    /// Best move in `pos` (reached by the UCI `moves` from the starting
    /// position) and its score for the side to move; `None` if there is none.
    fn play(
        &mut self,
        pos: &Position,
        moves: &[String],
        limits: &SearchLimits,
    ) -> Result<Option<(chess_core::Move, i32)>> {
        match self {
            Self::Engine(searcher) => {
                let result = searcher.search(pos, limits, |_| {});
                let score = match result.score {
                    Score::Centipawns(cp) => cp,
                    Score::Mate(moves) if moves > 0 => MATE_SCORE,
                    Score::Mate(_) => -MATE_SCORE,
                };
                Ok(result.best_move.map(|best| (best, score)))
            }
            Self::Stockfish(stockfish) => {
                let (best, score) = stockfish.best_move(moves)?;
                if best == "(none)" {
                    return Ok(None);
                }
                let mv = pos
                    .parse_uci_move(&best)
                    .map_err(|err| anyhow::anyhow!("Stockfish played {best}: {err}"))?;
                let score = match score {
                    Some(UciScore::Centipawns(cp)) => cp,
                    Some(UciScore::Mate(moves)) if moves > 0 => MATE_SCORE,
                    Some(UciScore::Mate(_)) => -MATE_SCORE,
                    None => 0,
                };
                Ok(Some((mv, score)))
            }
        }
    }
}

/// Plays one game. Our engine searches within `config.move_limit()`;
/// Stockfish plays at its level.
fn play_game(
    white: &mut Seat,
    black: &mut Seat,
    config: &ArenaConfig,
    opening: &[chess_core::Move],
) -> Result<Outcome> {
    let mut pos = Position::startpos();
    let mut moves = Vec::with_capacity(160);
    for &mv in opening {
        pos.make_move(mv);
        moves.push(mv.to_string());
    }
    white.new_game()?;
    black.new_game()?;
    let limits = config.move_limit().search_limits();
    let mut decisive = 0;
    let mut quiet = 0;
    loop {
        match pos.status() {
            GameStatus::Ongoing => {}
            GameStatus::Checkmate {
                winner: Color::White,
            } => return Ok(Outcome::WhiteWins),
            GameStatus::Checkmate {
                winner: Color::Black,
            } => return Ok(Outcome::BlackWins),
            GameStatus::Stalemate | GameStatus::Draw(_) => return Ok(Outcome::Draw),
        }
        if pos.ply() >= config.max_plies as usize {
            return Ok(Outcome::Draw);
        }
        let side = pos.side_to_move();
        let seat = if side == Color::White {
            &mut *white
        } else {
            &mut *black
        };
        let Some((best, score)) = seat.play(&pos, &moves, &limits)? else {
            return Ok(Outcome::Draw);
        };
        // Adjudicate once both engines agree for a few moves.
        decisive = if score.abs() >= 1_000 {
            decisive + 1
        } else {
            0
        };
        quiet = if score.abs() <= 10 { quiet + 1 } else { 0 };
        if decisive >= 6 {
            let white_wins = score * side.sign() > 0;
            return Ok(if white_wins {
                Outcome::WhiteWins
            } else {
                Outcome::BlackWins
            });
        }
        if quiet >= 10 && pos.ply() >= 80 {
            return Ok(Outcome::Draw);
        }
        pos.make_move(best);
        moves.push(best.to_string());
    }
}

/// Random opening for pair `index` (both games of a pair share it).
fn opening(config: &ArenaConfig, index: u64) -> Vec<chess_core::Move> {
    let mut rng = Rng::new(config.seed.wrapping_add(index));
    let mut pos = Position::startpos();
    let mut moves = Vec::new();
    for _ in 0..rng.between(config.random_plies_min, config.random_plies_max) {
        let legal = pos.legal_moves();
        if legal.is_empty() || pos.status().is_over() {
            break;
        }
        let mv = legal[rng.below(legal.len())];
        pos.make_move(mv);
        moves.push(mv);
    }
    // An opening that already ended the game would be useless: retry it shorter.
    if pos.status().is_over() {
        moves.pop();
    }
    moves
}

/// Plays A against B. With `config.sprt`, stops as soon as the SPRT concludes.
/// `on_progress` receives the running result after each batch of pairs.
pub(crate) fn play_match(
    a: &Contestant,
    b: &Contestant,
    config: &ArenaConfig,
    stop: &StopFlag,
    mut on_progress: impl FnMut(MatchResult),
) -> Result<MatchResult> {
    let threads = if config.threads == 0 {
        rayon::current_num_threads()
    } else {
        config.threads
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("arena-{i}"))
        .build()
        .context("cannot start the match threads")?;
    let pairs = config.games.div_ceil(2).max(1);
    let next_pair = AtomicU32::new(0);
    let mut result = MatchResult::default();

    while !stop.is_stopped() && next_pair.load(Ordering::Relaxed) < pairs {
        let start = next_pair.fetch_add(threads as u32, Ordering::Relaxed);
        let end = (start + threads as u32).min(pairs);
        let batch: Vec<Result<MatchResult>> = pool.install(|| {
            (start..end)
                .into_par_iter()
                .map_init(
                    || Ok((a.seat()?, b.seat()?)),
                    |seats: &mut Result<(Seat, Seat)>, pair| {
                        let (seat_a, seat_b) =
                            seats.as_mut().map_err(|err| anyhow::anyhow!("{err:#}"))?;
                        let moves = opening(config, u64::from(pair));
                        let mut pair_result = MatchResult::default();
                        // Same opening, colors swapped.
                        let outcome = play_game(seat_a, seat_b, config, &moves)?;
                        record(&mut pair_result, outcome, Color::White);
                        let outcome = play_game(seat_b, seat_a, config, &moves)?;
                        record(&mut pair_result, outcome, Color::Black);
                        Ok(pair_result)
                    },
                )
                .collect()
        });
        for pair in batch {
            result += pair?;
        }
        on_progress(result);
        if config.sprt && config.sprt_test().verdict(result) != Verdict::Inconclusive {
            break;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprt_uses_the_configured_hypotheses() {
        let config = ArenaConfig {
            elo1: 5.0,
            alpha: 0.1,
            ..ArenaConfig::default()
        };
        let sprt = config.sprt_test();
        assert_eq!(
            (sprt.elo0, sprt.elo1, sprt.alpha, sprt.beta),
            (0.0, 5.0, 0.1, 0.05)
        );
        let crushing = MatchResult {
            wins: 300,
            draws: 100,
            losses: 100,
        };
        assert_eq!(sprt.verdict(crushing), Verdict::Pass);
    }

    #[test]
    fn nps_is_measured() {
        let nps = measure_nps(&EngineEval::default(), 2_000);
        assert!(nps > 1_000, "{nps}");
    }

    #[test]
    fn pairs_share_openings_and_matches_are_symmetric() {
        let config = ArenaConfig {
            games: 4,
            nodes: 200,
            threads: 2,
            max_plies: 120,
            ..ArenaConfig::default()
        };
        assert_eq!(opening(&config, 3), opening(&config, 3));
        let classic = Contestant::Engine(EngineEval::default());
        let mut progress = 0;
        let result = play_match(&classic, &classic, &config, &StopFlag::new(), |_| {
            progress += 1
        })
        .unwrap();
        assert_eq!(result.games(), 4);
        assert!(progress >= 1);
        // Identical engines: every pair is a mirror, so wins and losses balance.
        assert_eq!(result.wins, result.losses);
    }

    #[test]
    fn move_limits() {
        let config = ArenaConfig::default();
        assert_eq!(config.move_limit(), MoveLimit::Nodes(3_000));
        assert_eq!(MoveLimit::Depth(6).search_limits().depth, Some(6));
        assert_eq!(
            MoveLimit::Time(250).search_limits().movetime,
            Some(std::time::Duration::from_millis(250))
        );
        assert!(MoveLimit::Nodes(5_000).validate().is_ok());
        assert!(MoveLimit::Depth(31).validate().is_err());
        assert!(MoveLimit::Time(5).validate().is_err());
        let json = serde_json::to_string(&MoveLimit::Depth(8)).unwrap();
        assert_eq!(json, r#"{"kind":"depth","value":8}"#);
        assert_eq!(MoveLimit::Time(100).to_string(), "100 ms per move");

        // Depth-limited games are played to the end.
        let config = ArenaConfig {
            games: 2,
            threads: 1,
            max_plies: 60,
            limit: Some(MoveLimit::Depth(2)),
            ..ArenaConfig::default()
        };
        let classic = Contestant::Engine(EngineEval::default());
        let result = play_match(&classic, &classic, &config, &StopFlag::new(), |_| {}).unwrap();
        assert_eq!(result.games(), 2);
    }

    #[test]
    fn loads_evaluators() {
        assert_eq!(load_evaluator("classic").unwrap().name(), "classic");
        assert!(load_evaluator("/no/such/net.nnue").is_err());
        let config = ArenaConfig::default();
        assert_eq!(
            Contestant::load("classic", &config).unwrap().name(),
            "classic"
        );
        assert_eq!(
            Contestant::load("stockfish-4", &config).unwrap().name(),
            "stockfish-4"
        );
        assert!(Contestant::load("stockfish-11", &config).is_err());
    }

    #[test]
    fn a_missing_stockfish_fails_the_match() {
        let config = ArenaConfig {
            games: 2,
            threads: 1,
            stockfish: PathBuf::from("/no/such/stockfish"),
            ..ArenaConfig::default()
        };
        let stockfish = Contestant::load("stockfish-1", &config).unwrap();
        let classic = Contestant::Engine(EngineEval::default());
        let result = play_match(&classic, &stockfish, &config, &StopFlag::new(), |_| {});
        assert!(result.is_err());
    }

    /// Plays real games against Stockfish when one is installed (set
    /// `CHESS_TRAIN_TEST_STOCKFISH` to its path; skipped otherwise).
    #[test]
    fn plays_stockfish_when_available() {
        let Ok(binary) = std::env::var("CHESS_TRAIN_TEST_STOCKFISH") else {
            return;
        };
        let config = ArenaConfig {
            games: 2,
            nodes: 500,
            threads: 2,
            max_plies: 200,
            stockfish: PathBuf::from(binary),
            ..ArenaConfig::default()
        };
        let stockfish = Contestant::load("stockfish-1", &config).unwrap();
        let classic = Contestant::Engine(EngineEval::default());
        let result = play_match(&classic, &stockfish, &config, &StopFlag::new(), |_| {}).unwrap();
        assert_eq!(result.games(), 2);
    }
}
