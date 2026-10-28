//! Winning chances (Lichess's curve) and accuracy (fitted on Chess.com's).

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
/// `103.1668 * exp(-0.0816 * (before - after)) - 3.1669`, 100 when it lost
/// nothing. Lichess's curve, twice as steep (0.0816 instead of 0.04354):
/// together with [`game_accuracy`], fitted on the accuracies Chess.com
/// gives to games.
#[must_use]
pub fn move_accuracy(before: f64, after: f64) -> f64 {
    if after >= before {
        return 100.0;
    }
    let raw = 103.166_810_071_164_9 * (-0.0816 * (before - after)).exp() - 3.166_924_740_191_411;
    raw.clamp(0.0, 100.0)
}

/// Exponent of the power mean of [`game_accuracy`].
const POWER: f64 = 0.25;

/// Accuracy of each player over a game (White first): the power mean, of
/// exponent 1/4, of their moves' accuracies (1 at least, as in Lichess's
/// harmonic mean), the first move played by `first`; `None` for a player
/// who made no move.
///
/// Chess.com does not publish its formula. This one and the curve of
/// [`move_accuracy`] were fitted on 332 accuracies Chess.com gave (games
/// of every level, analysed by Stockfish at depth 12 to 20): a mean error
/// of 4 points, against 6 for Lichess's formula (volatility-weighted mean
/// averaged with the harmonic mean). The harmonic mean let one blunder sink
/// a short game: 36 instead of Chess.com's 62 on a 35-ply game with three.
#[must_use]
pub fn game_accuracy(accuracies: &[f64], first: Color) -> [Option<f64>; 2] {
    let mut sums = [(0.0, 0_u32); 2];
    for (ply, &accuracy) in accuracies.iter().enumerate() {
        let mover = if ply % 2 == 0 { first } else { !first };
        let sum = &mut sums[mover.index()];
        sum.0 += accuracy.clamp(1.0, 100.0).powf(POWER);
        sum.1 += 1;
    }
    sums.map(|(sum, count)| (count > 0).then(|| (sum / f64::from(count)).powf(1.0 / POWER)))
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
    fn move_accuracy_falls_with_the_chances_lost() {
        close(move_accuracy(50.0, 50.0), 100.0);
        close(move_accuracy(40.0, 60.0), 100.0);
        close(move_accuracy(60.0, 40.0), 17.0061);
        close(move_accuracy(50.0, 45.0), 65.4368);
        // Throwing away a won game.
        close(move_accuracy(100.0, 0.0), 0.0);
    }

    #[test]
    fn game_accuracy_weighs_the_worst_moves() {
        // Both players perfect.
        let [white, black] = game_accuracy(&[100.0; 6], Color::White);
        close(white.unwrap(), 100.0);
        close(black.unwrap(), 100.0);

        // Black blunders once, at move 4 (its 2nd move).
        let blunder = move_accuracy(50.0, 10.0);
        let accuracies = [100.0, 100.0, 100.0, blunder, 100.0, 100.0];
        let [white, black] = game_accuracy(&accuracies, Color::White);
        close(white.unwrap(), 100.0);
        // Below the plain mean (67): the blunder weighs more, but less than
        // in a harmonic mean (2.9).
        close(black.unwrap(), 35.5337);
    }

    #[test]
    fn game_accuracy_of_short_games() {
        assert_eq!(game_accuracy(&[], Color::White), [None, None]);
        let [white, black] = game_accuracy(&[80.0], Color::Black);
        assert_eq!(white, None);
        close(black.unwrap(), 80.0);
    }
}
