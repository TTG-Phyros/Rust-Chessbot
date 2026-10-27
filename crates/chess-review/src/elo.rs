//! Elo estimate from how a player played, one model per time class, fitted
//! on Chess.com games (see `chess-reviewer calibrate`).
//!
//! One game says little: its accuracy, centipawn loss and error rates
//! correlate with the rating at about 0.45 only. A regression of the
//! rating on them would shrink every estimate towards the mean, and
//! averaging games would not undo it. The model is the other way round
//! (classical calibration): each feature is a linear function of the
//! rating plus a noise, correlated between features, measured on the
//! calibration games. The estimate is the rating that best explains the
//! mean features of the games (generalised least squares): unbiased, its
//! variance falling as one over the number of games, plus a player's own
//! offset that more games do not remove.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::classify::{MoveClass, PlayerReview};

/// Number of features of a game.
pub const FEATURES: usize = 5;

/// Names of the features, in the order of [`EloFeatures::vector`].
pub const FEATURE_NAMES: [&str; FEATURES] = [
    "accuracy",
    "log_acpl",
    "blunders",
    "mistakes",
    "inaccuracies",
];

/// Width of a 95 % interval, in standard deviations.
const Z95: f64 = 1.96;

/// Fewest games of a player for the cross-validation of profiles.
const PROFILE_GAMES: usize = 3;

/// How one player played one game.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EloFeatures {
    /// Accuracy, 0 to 100.
    pub accuracy: f64,
    /// Average centipawn loss.
    pub acpl: f64,
    /// Blunders and misses per 100 moves.
    pub blunders: f64,
    /// Mistakes per 100 moves.
    pub mistakes: f64,
    /// Inaccuracies per 100 moves.
    pub inaccuracies: f64,
}

impl EloFeatures {
    /// Fewest moves of a game that says something of the player.
    pub const MIN_MOVES: u32 = 10;

    /// The features of `player`'s game; `None` under [`Self::MIN_MOVES`]
    /// moves (book moves excluded).
    #[must_use]
    pub fn of(player: &PlayerReview) -> Option<Self> {
        let counts = &player.counts;
        let moves = player.moves.saturating_sub(counts.get(MoveClass::Book));
        if moves < Self::MIN_MOVES {
            return None;
        }
        let rate = |count: u32| f64::from(count) * 100.0 / f64::from(moves);
        Some(Self {
            accuracy: player.accuracy?,
            acpl: player.acpl?,
            blunders: rate(counts.get(MoveClass::Blunder) + counts.get(MoveClass::Miss)),
            mistakes: rate(counts.get(MoveClass::Mistake)),
            inaccuracies: rate(counts.get(MoveClass::Inaccuracy)),
        })
    }

    /// The model's inputs (the ACPL on a log scale: its effect fades).
    #[must_use]
    pub fn vector(&self) -> [f64; FEATURES] {
        [
            self.accuracy,
            (self.acpl.clamp(0.0, 1000.0) + 10.0).ln(),
            self.blunders,
            self.mistakes,
            self.inaccuracies,
        ]
    }
}

/// An estimated rating with its 95 % interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EloEstimate {
    /// The estimate.
    pub rating: f64,
    /// Lower bound of the 95 % interval.
    pub low: f64,
    /// Upper bound.
    pub high: f64,
    /// Games it rests on.
    pub games: usize,
}

type Matrix = [[f64; FEATURES]; FEATURES];

/// The model of one time class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassModel {
    /// Each feature at rating 0 (`feature = intercept + slope × rating`).
    pub intercepts: [f64; FEATURES],
    /// Change of each feature per rating point.
    pub slopes: [f64; FEATURES],
    /// Inverse of the covariance of the features' noise in one game.
    pub precision: Matrix,
    /// What one game tells of the rating (`slopesᵀ · precision · slopes`):
    /// the variance of a one-game estimate is its inverse.
    pub information: f64,
    /// Standard deviation of a player's own offset, which more games do
    /// not remove (style, opponents, a rating that lags).
    pub offset: f64,
    /// Calibration games.
    pub samples: usize,
    /// Lowest and highest rating among them: estimates stay within.
    pub ratings: [f64; 2],
    /// Cross-validated mean absolute error of a one-game estimate.
    pub game_mae: f64,
    /// Cross-validated mean absolute error of a player's estimate from
    /// their games (players with at least 3), and their mean number of games.
    pub player_mae: f64,
    /// Mean number of games behind `player_mae`.
    pub player_games: f64,
    /// Cross-validated share of these players whose mean rating falls in
    /// the 95 % interval of their estimate.
    pub coverage: f64,
}

impl ClassModel {
    /// Rating estimated from the games of a player, with its interval;
    /// `None` without games.
    #[must_use]
    pub fn estimate(&self, games: &[EloFeatures]) -> Option<EloEstimate> {
        let (rating, variance) = self.solve(games)?;
        let spread = (variance + self.offset.powi(2)).sqrt();
        let [lowest, highest] = self.ratings;
        let rating = rating.clamp(lowest, highest);
        Some(EloEstimate {
            rating,
            low: (rating - Z95 * spread).max(lowest),
            high: (rating + Z95 * spread).min(highest),
            games: games.len(),
        })
    }

    /// The rating that best explains the mean features of `games`, and the
    /// variance of that estimate (offset left out), unclamped.
    fn solve(&self, games: &[EloFeatures]) -> Option<(f64, f64)> {
        if games.is_empty() || self.information <= 0.0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)] // A few hundred games at most.
        let count = games.len() as f64;
        let mut mean = [0.0; FEATURES];
        for game in games {
            for (sum, value) in mean.iter_mut().zip(game.vector()) {
                *sum += value / count;
            }
        }
        let gap: Vec<f64> = (0..FEATURES)
            .map(|i| mean[i] - self.intercepts[i])
            .collect();
        let weighted: f64 = (0..FEATURES)
            .map(|i| {
                (0..FEATURES)
                    .map(|j| self.slopes[i] * self.precision[i][j] * gap[j])
                    .sum::<f64>()
            })
            .sum();
        Some((
            weighted / self.information,
            1.0 / (count * self.information),
        ))
    }
}

/// The models of every calibrated time class.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EloModel {
    /// What the models were fitted on.
    pub source: String,
    /// By time class (`blitz`).
    pub models: BTreeMap<String, ClassModel>,
}

impl EloModel {
    /// The model of `time_class`, if calibrated.
    #[must_use]
    pub fn get(&self, time_class: &str) -> Option<&ClassModel> {
        self.models.get(time_class)
    }
}

/// A calibration game: how a player played, and their rating.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Their play.
    pub features: EloFeatures,
    /// Their rating in that game.
    pub rating: f64,
    /// The player: the cross-validation keeps all their games in one fold.
    pub player: String,
}

/// Fits the model of one time class on `samples`, cross-validated over
/// `folds` folds of players (fixed by a hash of their name). `None` with
/// fewer than 2 folds, too few samples, or features that do not vary with
/// the rating.
#[must_use]
pub fn fit(samples: &[Sample], folds: usize) -> Option<ClassModel> {
    if folds < 2 || samples.len() < 3 * (FEATURES + 1) {
        return None;
    }
    let fold_of = |sample: &Sample| {
        usize::try_from(fnv(&sample.player) % u64::try_from(folds).unwrap_or(1)).unwrap_or(0)
    };
    let mut game_errors = Vec::new();
    // Each tested player: estimate error, its variance, games.
    let mut players: Vec<(f64, f64, usize)> = Vec::new();
    for fold in 0..folds {
        let training: Vec<&Sample> = samples.iter().filter(|s| fold_of(s) != fold).collect();
        let Some(model) = train(&training) else {
            continue;
        };
        let mut by_player: BTreeMap<&str, Vec<&Sample>> = BTreeMap::new();
        for sample in samples.iter().filter(|s| fold_of(s) == fold) {
            by_player.entry(&sample.player).or_default().push(sample);
            if let Some((rating, _)) = model.solve(&[sample.features]) {
                let [lowest, highest] = model.ratings;
                game_errors.push((rating.clamp(lowest, highest) - sample.rating).abs());
            }
        }
        for games in by_player
            .values()
            .filter(|games| games.len() >= PROFILE_GAMES)
        {
            let features: Vec<EloFeatures> = games.iter().map(|s| s.features).collect();
            #[allow(clippy::cast_precision_loss)] // A few games.
            let actual = games.iter().map(|s| s.rating).sum::<f64>() / games.len() as f64;
            if let Some((rating, variance)) = model.solve(&features) {
                let [lowest, highest] = model.ratings;
                players.push((
                    rating.clamp(lowest, highest) - actual,
                    variance,
                    games.len(),
                ));
            }
        }
    }
    let mut model = train(&samples.iter().collect::<Vec<_>>())?;
    model.game_mae = mean(game_errors.iter().copied());
    model.player_mae = mean(players.iter().map(|(error, _, _)| error.abs()));
    #[allow(clippy::cast_precision_loss)] // A few games.
    let player_games = mean(players.iter().map(|&(_, _, games)| games as f64));
    model.player_games = player_games;
    // What the estimates miss beyond their own variance.
    model.offset = mean(
        players
            .iter()
            .map(|(error, variance, _)| error * error - variance),
    )
    .max(0.0)
    .sqrt();
    let covered = players
        .iter()
        .filter(|(error, variance, _)| {
            error.abs() <= Z95 * (variance + model.offset.powi(2)).sqrt()
        })
        .count();
    #[allow(clippy::cast_precision_loss)] // Player counts.
    let coverage = if players.is_empty() {
        0.0
    } else {
        covered as f64 / players.len() as f64
    };
    model.coverage = coverage;
    Some(model)
}

/// Fits each feature against the rating, and the covariance of the noise.
fn train(samples: &[&Sample]) -> Option<ClassModel> {
    if samples.len() < FEATURES + 2 {
        return None;
    }
    let rows: Vec<[f64; FEATURES]> = samples.iter().map(|s| s.features.vector()).collect();
    let ratings: Vec<f64> = samples.iter().map(|s| s.rating).collect();
    let mean_rating = mean(ratings.iter().copied());
    let spread: f64 = ratings.iter().map(|r| (r - mean_rating).powi(2)).sum();
    if spread <= 0.0 {
        return None;
    }
    let mut intercepts = [0.0; FEATURES];
    let mut slopes = [0.0; FEATURES];
    for i in 0..FEATURES {
        let mean_x = mean(rows.iter().map(|row| row[i]));
        let covariance: f64 = rows
            .iter()
            .zip(&ratings)
            .map(|(row, rating)| (row[i] - mean_x) * (rating - mean_rating))
            .sum();
        slopes[i] = covariance / spread;
        intercepts[i] = mean_x - slopes[i] * mean_rating;
    }
    #[allow(clippy::cast_precision_loss)] // Sample counts.
    let degrees = (rows.len() - 2) as f64;
    let mut covariance: Matrix = [[0.0; FEATURES]; FEATURES];
    for (row, rating) in rows.iter().zip(&ratings) {
        let noise: Vec<f64> = (0..FEATURES)
            .map(|i| row[i] - intercepts[i] - slopes[i] * rating)
            .collect();
        for i in 0..FEATURES {
            for j in 0..FEATURES {
                covariance[i][j] += noise[i] * noise[j] / degrees;
            }
        }
    }
    let precision = invert(covariance)?;
    let information: f64 = (0..FEATURES)
        .map(|i| {
            (0..FEATURES)
                .map(|j| slopes[i] * precision[i][j] * slopes[j])
                .sum::<f64>()
        })
        .sum();
    if information <= 0.0 {
        return None;
    }
    let lowest = ratings.iter().copied().fold(f64::INFINITY, f64::min);
    let highest = ratings.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some(ClassModel {
        intercepts,
        slopes,
        precision,
        information,
        offset: 0.0,
        samples: samples.len(),
        ratings: [lowest, highest],
        game_mae: 0.0,
        player_mae: 0.0,
        player_games: 0.0,
        coverage: 0.0,
    })
}

/// Inverse of a covariance matrix (Gauss-Jordan, partial pivoting); a
/// tiny ridge keeps nearly dependent features solvable.
fn invert(mut a: Matrix) -> Option<Matrix> {
    let trace: f64 = (0..FEATURES).map(|i| a[i][i]).sum();
    for (i, row) in a.iter_mut().enumerate() {
        row[i] += trace * 1e-9;
    }
    let mut inverse: Matrix = [[0.0; FEATURES]; FEATURES];
    for (i, row) in inverse.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for column in 0..FEATURES {
        let pivot = (column..FEATURES)
            .max_by(|&i, &j| a[i][column].abs().total_cmp(&a[j][column].abs()))?;
        if a[pivot][column].abs() < 1e-12 {
            return None;
        }
        a.swap(column, pivot);
        inverse.swap(column, pivot);
        let scale = a[column][column];
        for k in 0..FEATURES {
            a[column][k] /= scale;
            inverse[column][k] /= scale;
        }
        let (pivot_row, pivot_inverse) = (a[column], inverse[column]);
        for row in 0..FEATURES {
            if row == column {
                continue;
            }
            let factor = a[row][column];
            for k in 0..FEATURES {
                a[row][k] -= factor * pivot_row[k];
                inverse[row][k] -= factor * pivot_inverse[k];
            }
        }
    }
    Some(inverse)
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0_u32), |(sum, count), value| (sum + value, count + 1));
    if count == 0 {
        0.0
    } else {
        sum / f64::from(count)
    }
}

/// FNV-1a: a hash that does not change between runs or platforms.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::ClassCounts;

    /// A deterministic pseudo-random number in [-1, 1] (splitmix64).
    fn noise(seed: u64) -> f64 {
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        #[allow(clippy::cast_precision_loss)]
        let unit = (z >> 11) as f64 / (1_u64 << 53) as f64;
        unit * 2.0 - 1.0
    }

    /// Features of a game by a player of `rating`, noisy as real games.
    fn game(rating: f64, seed: u64) -> EloFeatures {
        let skill = (rating - 800.0) / 2000.0;
        EloFeatures {
            accuracy: 65.0 + 15.0 * skill + 9.0 * noise(seed),
            acpl: (90.0 - 50.0 * skill + 30.0 * noise(seed + 7_000_000)).max(5.0),
            blunders: (8.0 - 5.0 * skill + 4.0 * noise(seed + 13_000_000)).max(0.0),
            mistakes: (5.0 - 2.0 * skill + 3.0 * noise(seed + 17_000_000)).max(0.0),
            inaccuracies: (8.0 - 2.0 * skill + 4.0 * noise(seed + 19_000_000)).max(0.0),
        }
    }

    /// 60 players of ratings 800 to 2750, 6 games each.
    fn samples() -> Vec<Sample> {
        let mut samples = Vec::new();
        for player in 0..60_u64 {
            #[allow(clippy::cast_precision_loss)]
            let rating = 800.0 + 33.0 * player as f64;
            for index in 0..6_u64 {
                samples.push(Sample {
                    features: game(rating, player * 100 + index),
                    rating,
                    player: format!("p{player}"),
                });
            }
        }
        samples
    }

    #[test]
    fn more_games_give_a_better_estimate() {
        let samples = samples();
        let model = fit(&samples, 5).unwrap();
        assert_eq!(model.samples, 360);
        // A game alone says little; six say much more.
        assert!(model.player_mae < model.game_mae * 0.7, "{model:?}");
        assert!((model.player_games - 6.0).abs() < 1e-9);
        assert!(model.coverage > 0.85, "{model:?}");
        // A new 2000 player, over 1 and 30 games.
        let one = model.estimate(&[game(2000.0, 99_001)]).unwrap();
        let games: Vec<EloFeatures> = (0..30).map(|i| game(2000.0, 99_000 + i)).collect();
        let many = model.estimate(&games).unwrap();
        assert!((many.rating - 2000.0).abs() < 250.0, "{many:?}");
        assert!(many.low <= 2000.0 && 2000.0 <= many.high, "{many:?}");
        assert!(many.high - many.low < one.high - one.low);
        assert!(one.low >= 800.0 && one.high <= 2747.0);
        assert_eq!(many.games, 30);
        assert_eq!(model.estimate(&[]), None);
        // Deterministic.
        assert_eq!(fit(&samples, 5), Some(model));
    }

    #[test]
    fn better_play_rates_higher() {
        let model = fit(&samples(), 5).unwrap();
        let good = EloFeatures {
            accuracy: 85.0,
            acpl: 35.0,
            blunders: 2.0,
            mistakes: 2.5,
            inaccuracies: 5.5,
        };
        let bad = EloFeatures {
            accuracy: 64.0,
            acpl: 95.0,
            blunders: 8.0,
            mistakes: 5.0,
            inaccuracies: 8.0,
        };
        let rate = |game| model.estimate(&[game; 10]).unwrap().rating;
        assert!(rate(good) > rate(bad) + 600.0);
    }

    #[test]
    fn too_few_samples_fit_nothing() {
        let samples = samples();
        assert_eq!(fit(&samples[..10], 5), None);
        assert_eq!(fit(&samples, 1), None);
    }

    #[test]
    fn features_of_a_player() {
        let mut counts = ClassCounts::default();
        for _ in 0..16 {
            counts.add(MoveClass::Best);
        }
        for _ in 0..4 {
            counts.add(MoveClass::Book);
        }
        counts.add(MoveClass::Blunder);
        counts.add(MoveClass::Miss);
        counts.add(MoveClass::Mistake);
        counts.add(MoveClass::Inaccuracy);
        let player = PlayerReview {
            moves: 24,
            accuracy: Some(81.0),
            acpl: Some(40.0),
            counts,
        };
        let features = EloFeatures::of(&player).unwrap();
        // 20 moves out of book: 2 blunders (a miss included) for 100 moves = 10.
        assert!((features.blunders - 10.0).abs() < 1e-9);
        assert!((features.mistakes - 5.0).abs() < 1e-9);
        assert!((features.inaccuracies - 5.0).abs() < 1e-9);
        let short = PlayerReview { moves: 8, ..player };
        assert_eq!(EloFeatures::of(&short), None);
    }
}
