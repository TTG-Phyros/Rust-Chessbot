//! Match statistics, shared by the workers that play matches and the API
//! that reports them: Elo difference and its error, likelihood of
//! superiority, sequential probability ratio test (SPRT), and ratings
//! fitted over several matches.

use chess_nnue_spec::ValidationError;
use serde::{Deserialize, Serialize};

/// Games won, drawn and lost by the first player of a match.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MatchResult {
    /// Games won.
    pub wins: u32,
    /// Games drawn.
    pub draws: u32,
    /// Games lost.
    pub losses: u32,
}

impl MatchResult {
    /// Games played.
    #[must_use]
    pub fn games(self) -> u32 {
        self.wins + self.draws + self.losses
    }

    /// Mean score, in `[0, 1]` (0.5 before any game).
    #[must_use]
    pub fn score(self) -> f64 {
        if self.games() == 0 {
            return 0.5;
        }
        (f64::from(self.wins) + 0.5 * f64::from(self.draws)) / f64::from(self.games())
    }

    /// Per-game variance of the score, estimated with half a game of each
    /// outcome added (a uniform prior): games that all ended the same way
    /// do not make it vanish, which would claim a certainty — an error of
    /// zero, a SPRT that never concludes. From a few dozen games on, the
    /// prior no longer matters.
    fn variance(self) -> f64 {
        let (w, d, l) = (
            f64::from(self.wins) + 0.5,
            f64::from(self.draws) + 0.5,
            f64::from(self.losses) + 0.5,
        );
        let n = w + d + l;
        let s = (w + 0.5 * d) / n;
        (w * (1.0 - s).powi(2) + d * (0.5 - s).powi(2) + l * s.powi(2)) / n
    }

    /// Elo difference (logistic model).
    #[must_use]
    pub fn elo(self) -> f64 {
        score_to_elo(self.score())
    }

    /// Half-width of the 95 % confidence interval of [`MatchResult::elo`]
    /// (0 before any game).
    #[must_use]
    pub fn elo_error(self) -> f64 {
        if self.games() == 0 {
            return 0.0;
        }
        let stderr = (self.variance() / f64::from(self.games())).sqrt();
        let s = self.score();
        (score_to_elo(s + 1.96 * stderr) - score_to_elo(s - 1.96 * stderr)) / 2.0
    }

    /// Likelihood of superiority: probability that the first player is
    /// the stronger.
    #[must_use]
    pub fn los(self) -> f64 {
        let (w, l) = (f64::from(self.wins), f64::from(self.losses));
        if w + l == 0.0 {
            return 0.5;
        }
        0.5 * (1.0 + erf((w - l) / (2.0 * (w + l)).sqrt()))
    }

    /// Log-likelihood ratio of H1 (`elo1`) against H0 (`elo0`), normal
    /// approximation of the trinomial model as used by fishtest.
    #[must_use]
    pub fn llr(self, elo0: f64, elo1: f64) -> f64 {
        let (s0, s1) = (elo_to_score(elo0), elo_to_score(elo1));
        f64::from(self.games()) * (s1 - s0) * (2.0 * self.score() - s0 - s1)
            / (2.0 * self.variance())
    }

    /// The same games, seen from the other player.
    #[must_use]
    pub fn reversed(self) -> Self {
        Self {
            wins: self.losses,
            draws: self.draws,
            losses: self.wins,
        }
    }
}

impl std::ops::AddAssign for MatchResult {
    fn add_assign(&mut self, other: Self) {
        self.wins += other.wins;
        self.draws += other.draws;
        self.losses += other.losses;
    }
}

/// A sequential probability ratio test: is the first player at least
/// `elo1` stronger (H1), or at most `elo0` (H0)?
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Sprt {
    /// Elo difference of H0.
    pub elo0: f64,
    /// Elo difference of H1.
    pub elo1: f64,
    /// Probability of accepting H1 when H0 holds.
    pub alpha: f64,
    /// Probability of accepting H0 when H1 holds.
    pub beta: f64,
}

impl Default for Sprt {
    fn default() -> Self {
        Self {
            elo0: 0.0,
            elo1: 10.0,
            alpha: 0.05,
            beta: 0.05,
        }
    }
}

/// What a SPRT concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// H1 accepted: the first player is at least `elo1` stronger.
    Pass,
    /// H0 accepted: the first player is not `elo1` stronger.
    Fail,
    /// Not enough games yet.
    Inconclusive,
}

impl Sprt {
    /// The LLR bounds: H0 is accepted below the first, H1 above the second.
    #[must_use]
    pub fn bounds(&self) -> (f64, f64) {
        (
            (self.beta / (1.0 - self.alpha)).ln(),
            ((1.0 - self.beta) / self.alpha).ln(),
        )
    }

    /// The test's conclusion after `result`.
    #[must_use]
    pub fn verdict(&self, result: MatchResult) -> Verdict {
        let llr = result.llr(self.elo0, self.elo1);
        let (lower, upper) = self.bounds();
        if llr >= upper {
            Verdict::Pass
        } else if llr <= lower {
            Verdict::Fail
        } else {
            Verdict::Inconclusive
        }
    }

    /// Range errors, with paths below `prefix` (e.g. `/sprt`).
    #[must_use]
    pub fn validate(&self, prefix: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut check = |ok: bool, field: &str, message: &str| {
            if !ok {
                errors.push(ValidationError {
                    path: format!("{prefix}/{field}"),
                    message: message.into(),
                });
            }
        };
        let elo = -1_000.0..=1_000.0;
        check(
            elo.contains(&self.elo0),
            "elo0",
            "must be between -1000 and 1000",
        );
        check(
            elo.contains(&self.elo1) && self.elo1 > self.elo0,
            "elo1",
            "must be greater than elo0, at most 1000",
        );
        check(
            self.alpha > 0.0 && self.alpha < 0.5,
            "alpha",
            "must be in (0, 0.5)",
        );
        check(
            self.beta > 0.0 && self.beta < 0.5,
            "beta",
            "must be in (0, 0.5)",
        );
        errors
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

/// Elo ratings of `players` players from the results of pairs of them
/// (`(a, b, result of a)`), relative to player 0: maximum likelihood of
/// the Bradley–Terry model, draws counting half, with one virtual draw per
/// pair that played so that a perfect score stays finite. `None` for the
/// players no chain of results links to player 0.
#[must_use]
pub fn ratings(players: usize, results: &[(usize, usize, MatchResult)]) -> Vec<Option<f64>> {
    // Points scored and games played against each opponent.
    let mut points = vec![0.0; players];
    let mut games = vec![vec![0.0; players]; players];
    for &(a, b, result) in results {
        if a == b || a >= players || b >= players || result.games() == 0 {
            continue;
        }
        let n = f64::from(result.games());
        let scored = f64::from(result.wins) + 0.5 * f64::from(result.draws);
        points[a] += scored;
        points[b] += n - scored;
        games[a][b] += n;
        games[b][a] += n;
    }
    for (row, scored) in games.iter_mut().zip(&mut points) {
        for n in row.iter_mut().filter(|n| **n > 0.0) {
            // The virtual draw, seen from this player (the opponent's row
            // counts the other side).
            *n += 1.0;
            *scored += 0.5;
        }
    }
    let linked = linked_to_first(&games);
    // Minorization-maximization (Hunter 2004): converges for connected players.
    let mut strength = vec![1.0_f64; players];
    for _ in 0..10_000 {
        let mut change = 0.0_f64;
        for i in (0..players).filter(|&i| linked[i]) {
            let denominator: f64 = (0..players)
                .filter(|&j| games[i][j] > 0.0)
                .map(|j| games[i][j] / (strength[i] + strength[j]))
                .sum();
            if denominator > 0.0 {
                let next = points[i] / denominator;
                change = change.max((next / strength[i]).ln().abs());
                strength[i] = next;
            }
        }
        if change < 1e-10 {
            break;
        }
    }
    (0..players)
        .map(|i| linked[i].then(|| 400.0 * (strength[i] / strength[0]).log10()))
        .collect()
}

/// Which players a chain of games links to player 0.
fn linked_to_first(games: &[Vec<f64>]) -> Vec<bool> {
    let mut linked = vec![false; games.len()];
    let mut pending = Vec::new();
    if !games.is_empty() {
        linked[0] = true;
        pending.push(0);
    }
    while let Some(i) = pending.pop() {
        for (j, &n) in games[i].iter().enumerate() {
            if n > 0.0 && !linked[j] {
                linked[j] = true;
                pending.push(j);
            }
        }
    }
    linked
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn wdl(wins: u32, draws: u32, losses: u32) -> MatchResult {
        MatchResult {
            wins,
            draws,
            losses,
        }
    }

    #[test]
    fn elo_statistics() {
        let even = wdl(30, 40, 30);
        assert!((even.score() - 0.5).abs() < 1e-12);
        assert!(even.elo().abs() < 1e-9);
        assert!((even.los() - 0.5).abs() < 1e-9);

        // 75 % is +190.85 Elo in the logistic model.
        let strong = wdl(70, 10, 20);
        assert!((strong.elo() - 190.85).abs() < 0.1, "{}", strong.elo());
        assert!(strong.elo_error() > 0.0 && strong.elo_error() < 100.0);
        assert!(strong.los() > 0.999);

        let weak = strong.reversed();
        assert!((weak.elo() + strong.elo()).abs() < 1e-9);
        assert!(weak.los() < 0.001);

        // Before any game: nothing is known.
        let none = MatchResult::default();
        assert_eq!((none.elo(), none.elo_error(), none.los()), (0.0, 0.0, 0.5));
        let mut sum = strong;
        sum += weak;
        assert_eq!(sum, wdl(90, 20, 90));
    }

    #[test]
    fn sprt_concludes_both_ways() {
        let sprt = Sprt::default();
        assert_eq!(sprt.verdict(wdl(300, 100, 100)), Verdict::Pass);
        assert_eq!(sprt.verdict(wdl(100, 100, 300)), Verdict::Fail);
        assert_eq!(sprt.verdict(wdl(3, 2, 2)), Verdict::Inconclusive);
        assert_eq!(sprt.verdict(MatchResult::default()), Verdict::Inconclusive);

        // Games that all end the same way still decide, and keep an error.
        let hopeless = Sprt {
            elo1: 50.0,
            ..Sprt::default()
        };
        assert_eq!(hopeless.verdict(wdl(0, 0, 4)), Verdict::Inconclusive);
        assert_eq!(hopeless.verdict(wdl(0, 0, 8)), Verdict::Fail);
        assert_eq!(hopeless.verdict(wdl(8, 0, 0)), Verdict::Pass);
        let error = wdl(0, 0, 20).elo_error();
        assert!(error > 100.0 && error.is_finite(), "{error}");
        assert!(wdl(0, 50, 0).elo_error() > 0.0);
        let (lower, upper) = sprt.bounds();
        assert!((lower + 2.944).abs() < 1e-3 && (upper - 2.944).abs() < 1e-3);
        assert!((erf(0.5) - 0.520_499_877_8).abs() < 1e-6);
    }

    #[test]
    fn sprt_bounds_are_validated() {
        assert!(Sprt::default().validate("/sprt").is_empty());
        let bad = Sprt {
            elo0: 5.0,
            elo1: 5.0,
            alpha: 0.0,
            beta: 0.5,
        };
        let paths: Vec<String> = bad.validate("/sprt").into_iter().map(|e| e.path).collect();
        assert_eq!(paths, ["/sprt/elo1", "/sprt/alpha", "/sprt/beta"]);
        let partial: Sprt = serde_json::from_str(r#"{"elo1": 5}"#).unwrap();
        assert_eq!(partial.elo0, 0.0);
        assert_eq!(partial.alpha, 0.05);
    }

    #[test]
    fn ratings_follow_the_results() {
        // B scores 75 % against A: about +190 Elo (a little less with the virtual draw).
        let ratings = ratings(3, &[(1, 0, wdl(70, 10, 20))]);
        assert_eq!(ratings[0], Some(0.0));
        let b = ratings[1].unwrap();
        assert!((185.0..191.0).contains(&b), "{b}");
        assert_eq!(ratings[2], None);

        // A chain A < B < C ranks C first, and a perfect score stays finite.
        let ratings = super::ratings(3, &[(0, 1, wdl(10, 20, 30)), (2, 1, wdl(20, 0, 0))]);
        let (b, c) = (ratings[1].unwrap(), ratings[2].unwrap());
        assert!(b > 0.0 && c > b && c.is_finite(), "{ratings:?}");

        // The order of the pair does not matter.
        let one = super::ratings(2, &[(0, 1, wdl(5, 3, 2))]);
        let other = super::ratings(2, &[(1, 0, wdl(2, 3, 5))]);
        assert!((one[1].unwrap() - other[1].unwrap()).abs() < 1e-9);
    }
}
