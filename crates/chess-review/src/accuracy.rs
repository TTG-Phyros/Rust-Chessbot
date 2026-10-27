//! Winning chances and accuracy, with the formulas of Lichess.

use chess_core::Color;

use crate::engine::Score;

/// Advantage, in centipawns, beyond which more counts the same (as on
/// Lichess); a mate counts as this much.
pub const CP_CAP: i32 = 1000;

/// Centipawns of `score`, capped at ±[`CP_CAP`] (a mate counts as the cap).
#[must_use]
pub fn centipawns(score: Score) -> i32 {
    match score {
        Score::Cp(cp) => cp.clamp(-CP_CAP, CP_CAP),
        Score::Mate(moves) if moves > 0 => CP_CAP,
        Score::Mate(_) => -CP_CAP,
    }
}

/// Winning chances, from 0 to 100, of the side to move with `score`:
/// `50 + 50 * (2 / (1 + exp(-0.00368208 * cp)) - 1)`, the centipawns being
/// capped at ±[`CP_CAP`]; 100 when it mates, 0 when it gets mated.
#[must_use]
pub fn win_percent(score: Score) -> f64 {
    match score {
        Score::Cp(cp) => {
            let cp = f64::from(cp.clamp(-CP_CAP, CP_CAP));
            50.0 + 50.0 * (2.0 / (1.0 + (-0.003_682_08 * cp).exp()) - 1.0)
        }
        Score::Mate(moves) if moves > 0 => 100.0,
        Score::Mate(_) => 0.0,
    }
}

/// Accuracy, from 0 to 100, of a move that took the winning chances of the
/// player from `before` to `after`:
/// `103.1668 * exp(-0.04354 * (before - after)) - 3.1669`, 100 when it lost
/// nothing.
#[must_use]
pub fn move_accuracy(before: f64, after: f64) -> f64 {
    if after >= before {
        return 100.0;
    }
    let raw = 103.166_810_071_164_9 * (-0.043_544_153_867_539_51 * (before - after)).exp()
        - 3.166_924_740_191_411;
    raw.clamp(0.0, 100.0)
}

/// Accuracy of each player over a game (White first), as Lichess computes
/// it: the mean of their moves' accuracies weighted by the volatility of
/// the game around each move (standard deviation of the winning chances
/// over a few moves), averaged with their harmonic mean, which weighs the
/// worst moves most. `None` for a player who made no move.
///
/// `white_wins` are White's winning chances in each position (one more
/// than the moves), `accuracies` the accuracy of each move, the first one
/// played by `first`.
#[must_use]
pub fn game_accuracy(white_wins: &[f64], accuracies: &[f64], first: Color) -> [Option<f64>; 2] {
    let moves = accuracies.len();
    if moves == 0 || white_wins.len() != moves + 1 {
        return [None, None];
    }
    // One window per move: copies of the first window for the first moves,
    // then a window sliding over the positions.
    let size = (moves / 10).clamp(2, 8).min(white_wins.len());
    let first_window = deviation(&white_wins[..size]);
    let weights = std::iter::repeat_n(first_window, size - 2)
        .chain(white_wins.windows(size).map(deviation))
        .map(|weight| weight.clamp(0.5, 12.0));
    let mut sums = [(0.0, 0.0, 0.0, 0_u32); 2];
    for (ply, (&accuracy, weight)) in accuracies.iter().zip(weights).enumerate() {
        let mover = if ply % 2 == 0 { first } else { !first };
        let sum = &mut sums[mover.index()];
        sum.0 += accuracy * weight;
        sum.1 += weight;
        sum.2 += 1.0 / accuracy.max(1.0);
        sum.3 += 1;
    }
    sums.map(|(weighted, weights, inverses, count)| {
        (count > 0).then(|| (weighted / weights + f64::from(count) / inverses) / 2.0)
    })
}

/// Population standard deviation.
fn deviation(values: &[f64]) -> f64 {
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    (values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / count)
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-3,
            "{actual} instead of {expected}"
        );
    }

    #[test]
    fn winning_chances_follow_the_lichess_curve() {
        close(win_percent(Score::Cp(0)), 50.0);
        close(win_percent(Score::Cp(100)), 59.1026);
        close(win_percent(Score::Cp(300)), 75.1126);
        close(win_percent(Score::Cp(-300)), 24.8874);
        // Symmetric, and capped at 10 pawns.
        for cp in [17, 250, 999] {
            close(
                win_percent(Score::Cp(cp)) + win_percent(Score::Cp(-cp)),
                100.0,
            );
        }
        close(win_percent(Score::Cp(5000)), win_percent(Score::Cp(CP_CAP)));
        // Mates are certain.
        close(win_percent(Score::Mate(3)), 100.0);
        close(win_percent(Score::Mate(-1)), 0.0);
        close(win_percent(Score::Mate(0)), 0.0);
        assert_eq!(centipawns(Score::Mate(2)), 1000);
        assert_eq!(centipawns(Score::Mate(-2)), -1000);
        assert_eq!(centipawns(Score::Cp(-1500)), -1000);
    }

    #[test]
    fn move_accuracy_follows_the_lichess_curve() {
        close(move_accuracy(50.0, 50.0), 100.0);
        close(move_accuracy(40.0, 60.0), 100.0);
        close(move_accuracy(60.0, 40.0), 40.0168);
        close(move_accuracy(50.0, 45.0), 79.8153);
        // Throwing away a won game.
        close(move_accuracy(100.0, 0.0), 0.0);
    }

    #[test]
    fn game_accuracy_weighs_the_worst_moves() {
        // A quiet game: both players perfect.
        let wins = [50.0; 7];
        let perfect = [100.0; 6];
        let [white, black] = game_accuracy(&wins, &perfect, Color::White);
        close(white.unwrap(), 100.0);
        close(black.unwrap(), 100.0);

        // Black blunders once, at move 4 (its 2nd move).
        let wins = [50.0, 50.0, 50.0, 50.0, 90.0, 90.0, 90.0];
        let accuracies = [100.0, 100.0, 100.0, move_accuracy(50.0, 10.0), 100.0, 100.0];
        let [white, black] = game_accuracy(&wins, &accuracies, Color::White);
        close(white.unwrap(), 100.0);
        let black = black.unwrap();
        let mean = (100.0 + accuracies[3] + 100.0) / 3.0;
        // Below the plain mean: the blunder weighs more.
        assert!(black < mean, "{black} >= {mean}");
        assert!(black > accuracies[3]);
    }

    #[test]
    fn game_accuracy_of_short_games() {
        assert_eq!(game_accuracy(&[50.0], &[], Color::White), [None, None]);
        let [white, black] = game_accuracy(&[50.0, 45.0], &[80.0], Color::Black);
        assert_eq!(white, None);
        close(black.unwrap(), 80.0);
        // Inconsistent lengths.
        assert_eq!(game_accuracy(&[50.0], &[80.0], Color::White), [None, None]);
    }
}
