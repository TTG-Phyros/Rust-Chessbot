//! Engine matches: two evaluators play pairs of games from the same random
//! opening with colors swapped, to measure their Elo difference, optionally
//! with a sequential probability ratio test (SPRT) that stops as soon as
//! the result is clear.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, Result};
use chess_core::{Color, GameStatus, Position};
use chess_eval::EngineEval;
use chess_search::{Score, SearchLimits, Searcher, StopFlag};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::dataset::Outcome;
use crate::rng::Rng;
use crate::stockfish::{self, Stockfish, UciScore};

/// Match parameters (`[arena]` section of the configuration).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ArenaConfig {
    /// Maximum number of games (rounded up to pairs).
    pub(crate) games: u32,
    /// Nodes searched per move by both engines.
    pub(crate) nodes: u64,
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

/// Games won, drawn and lost by engine A.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MatchResult {
    pub(crate) wins: u32,
    pub(crate) draws: u32,
    pub(crate) losses: u32,
}

/// What the SPRT concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Verdict {
    /// A is at least `elo1` stronger (H1 accepted).
    Pass,
    /// A is not `elo1` stronger (H0 accepted).
    Fail,
    /// Not enough games yet.
    Inconclusive,
}

impl MatchResult {
    pub(crate) fn games(self) -> u32 {
        self.wins + self.draws + self.losses
    }

    /// Mean score of A, in `[0, 1]`.
    pub(crate) fn score(self) -> f64 {
        let n = f64::from(self.games().max(1));
        (f64::from(self.wins) + 0.5 * f64::from(self.draws)) / n
    }

    /// Per-game variance of A's score.
    fn variance(self) -> f64 {
        let n = f64::from(self.games().max(1));
        let s = self.score();
        (f64::from(self.wins) * (1.0 - s).powi(2)
            + f64::from(self.draws) * (0.5 - s).powi(2)
            + f64::from(self.losses) * s.powi(2))
            / n
    }

    /// Elo difference of A over B (logistic model).
    pub(crate) fn elo(self) -> f64 {
        score_to_elo(self.score())
    }

    /// Half-width of the 95 % confidence interval of [`MatchResult::elo`].
    pub(crate) fn elo_error(self) -> f64 {
        let n = f64::from(self.games().max(1));
        let stderr = (self.variance() / n).sqrt();
        let s = self.score();
        (score_to_elo(s + 1.96 * stderr) - score_to_elo(s - 1.96 * stderr)) / 2.0
    }

    /// Likelihood of superiority: probability that A is stronger.
    pub(crate) fn los(self) -> f64 {
        let (w, l) = (f64::from(self.wins), f64::from(self.losses));
        if w + l == 0.0 {
            return 0.5;
        }
        0.5 * (1.0 + erf((w - l) / (2.0 * (w + l)).sqrt()))
    }

    /// Log-likelihood ratio of H1 (`elo1`) against H0 (`elo0`), normal
    /// approximation of the trinomial model as used by fishtest.
    pub(crate) fn llr(self, elo0: f64, elo1: f64) -> f64 {
        let variance = self.variance();
        if variance <= 0.0 {
            return 0.0;
        }
        let (s0, s1) = (elo_to_score(elo0), elo_to_score(elo1));
        f64::from(self.games()) * (s1 - s0) * (2.0 * self.score() - s0 - s1) / (2.0 * variance)
    }

    pub(crate) fn verdict(self, config: &ArenaConfig) -> Verdict {
        let llr = self.llr(config.elo0, config.elo1);
        let lower = (config.beta / (1.0 - config.alpha)).ln();
        let upper = ((1.0 - config.beta) / config.alpha).ln();
        if llr >= upper {
            Verdict::Pass
        } else if llr <= lower {
            Verdict::Fail
        } else {
            Verdict::Inconclusive
        }
    }

    fn add(&mut self, outcome: Outcome, a_color: Color) {
        match (outcome, a_color) {
            (Outcome::Draw, _) => self.draws += 1,
            (Outcome::WhiteWins, Color::White) | (Outcome::BlackWins, Color::Black) => {
                self.wins += 1
            }
            _ => self.losses += 1,
        }
    }
}

fn elo_to_score(elo: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-elo / 400.0))
}

fn score_to_elo(score: f64) -> f64 {
    let s = score.clamp(1e-4, 1.0 - 1e-4);
    -400.0 * (1.0 / s - 1.0).log10()
}

/// Error function (Abramowitz & Stegun 7.1.26, |error| < 1.5e-7).
fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let y = 1.0 - poly * (-x * x).exp();
    if x >= 0.0 { y } else { -y }
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
    /// Our engine with this evaluation, at the match's nodes per move.
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

/// Plays one game. Our engine searches `config.nodes` per move; Stockfish
/// plays at its level.
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
    let limits = SearchLimits::nodes(config.nodes.max(1));
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
                        pair_result.add(play_game(seat_a, seat_b, config, &moves)?, Color::White);
                        pair_result.add(play_game(seat_b, seat_a, config, &moves)?, Color::Black);
                        Ok(pair_result)
                    },
                )
                .collect()
        });
        for pair in batch {
            let pair = pair?;
            result.wins += pair.wins;
            result.draws += pair.draws;
            result.losses += pair.losses;
        }
        on_progress(result);
        if config.sprt && result.verdict(config) != Verdict::Inconclusive {
            break;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elo_statistics() {
        let even = MatchResult {
            wins: 30,
            draws: 40,
            losses: 30,
        };
        assert!((even.score() - 0.5).abs() < 1e-12);
        assert!(even.elo().abs() < 1e-9);
        assert!((even.los() - 0.5).abs() < 1e-9);

        // 75 % is +190.85 Elo in the logistic model.
        let strong = MatchResult {
            wins: 70,
            draws: 10,
            losses: 20,
        };
        assert!((strong.elo() - 190.85).abs() < 0.1, "{}", strong.elo());
        assert!(strong.elo_error() > 0.0 && strong.elo_error() < 100.0);
        assert!(strong.los() > 0.999);

        let weak = MatchResult {
            wins: 20,
            draws: 10,
            losses: 70,
        };
        assert!((weak.elo() + strong.elo()).abs() < 1e-9);
        assert!(weak.los() < 0.001);
    }

    #[test]
    fn sprt_concludes_both_ways() {
        let config = ArenaConfig {
            sprt: true,
            ..ArenaConfig::default()
        };
        let crushing = MatchResult {
            wins: 300,
            draws: 100,
            losses: 100,
        };
        assert_eq!(crushing.verdict(&config), Verdict::Pass);
        let losing = MatchResult {
            wins: 100,
            draws: 100,
            losses: 300,
        };
        assert_eq!(losing.verdict(&config), Verdict::Fail);
        let early = MatchResult {
            wins: 3,
            draws: 2,
            losses: 2,
        };
        assert_eq!(early.verdict(&config), Verdict::Inconclusive);
        assert!((erf(0.5) - 0.520_499_877_8).abs() < 1e-6);
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
