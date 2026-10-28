//! Statistics of a dataset, computed in one pass over its records.

use chess_core::Color;
use serde::{Deserialize, Serialize};

use crate::{Outcome, Record};

/// Counts of values in bins of equal width, with the values below the
/// first bin in `below` and those past the last in `above`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Histogram {
    /// Lower bound of the first bin (inclusive).
    pub min: i64,
    /// Width of each bin.
    pub width: i64,
    /// Values in `[min + i·width, min + (i+1)·width)`.
    pub counts: Vec<u64>,
    /// Values below `min`.
    pub below: u64,
    /// Values at or past `min + counts.len()·width`.
    pub above: u64,
}

impl Histogram {
    fn new(min: i64, width: i64, bins: usize) -> Self {
        Self {
            min,
            width,
            counts: vec![0; bins],
            below: 0,
            above: 0,
        }
    }

    fn add(&mut self, value: i64) {
        if value < self.min {
            self.below += 1;
            return;
        }
        let bin = ((value - self.min) / self.width.max(1)) as usize;
        match self.counts.get_mut(bin) {
            Some(count) => *count += 1,
            None => self.above += 1,
        }
    }
}

/// Game results of the positions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcomes {
    /// Positions from games White won.
    pub white_wins: u64,
    /// Positions from drawn games.
    pub draws: u64,
    /// Positions from games Black won.
    pub black_wins: u64,
}

/// What a dataset contains.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetStats {
    /// Positions.
    pub records: u64,
    /// Positions with White to move.
    pub white_to_move: u64,
    /// Results of the games the positions come from.
    pub outcomes: Outcomes,
    /// Mean absolute search score, in centipawns.
    pub mean_abs_score: f64,
    /// Search scores (White's point of view), bins of 100 centipawns from
    /// −1500 to +1500.
    pub scores: Histogram,
    /// `pieces[n]`: positions with `n` pieces, kings included (0 to 32).
    pub pieces: Vec<u64>,
    /// Plies of the positions in their games, bins of 20 up to 400.
    pub plies: Histogram,
}

impl Default for DatasetStats {
    fn default() -> Self {
        Self {
            records: 0,
            white_to_move: 0,
            outcomes: Outcomes::default(),
            mean_abs_score: 0.0,
            scores: Histogram::new(-1500, 100, 30),
            pieces: vec![0; 33],
            plies: Histogram::new(0, 20, 20),
        }
    }
}

impl DatasetStats {
    /// Counts `record`.
    pub fn add(&mut self, record: &Record) {
        let n = self.records as f64;
        self.records += 1;
        self.mean_abs_score =
            (self.mean_abs_score * n + f64::from(record.score.unsigned_abs())) / (n + 1.0);
        if record.side_to_move == Color::White {
            self.white_to_move += 1;
        }
        match record.outcome {
            Outcome::WhiteWins => self.outcomes.white_wins += 1,
            Outcome::Draw => self.outcomes.draws += 1,
            Outcome::BlackWins => self.outcomes.black_wins += 1,
        }
        self.scores.add(i64::from(record.score));
        if let Some(count) = self.pieces.get_mut(record.piece_count() as usize) {
            *count += 1;
        }
        self.plies.add(i64::from(record.ply));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use chess_core::Position;

    use super::*;

    #[test]
    fn counts_every_dimension() {
        let mut stats = DatasetStats::default();
        let start = Position::startpos();
        let endgame = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 b - - 0 1").unwrap();
        stats.add(&Record::new(&start, 20, Outcome::Draw));
        stats.add(&Record::new(&endgame, 900, Outcome::WhiteWins));
        stats.add(&Record::new(&endgame, -2000, Outcome::BlackWins));

        assert_eq!(stats.records, 3);
        assert_eq!(stats.white_to_move, 1);
        assert_eq!(
            stats.outcomes,
            Outcomes {
                white_wins: 1,
                draws: 1,
                black_wins: 1
            }
        );
        assert!((stats.mean_abs_score - 2920.0 / 3.0).abs() < 1e-9);
        // 20 is in [0, 100), the 16th bin; 900 in [900, 1000); −2000 below.
        assert_eq!(stats.scores.counts[15], 1);
        assert_eq!(stats.scores.counts[24], 1);
        assert_eq!(stats.scores.below, 1);
        assert_eq!(stats.pieces[32], 1);
        assert_eq!(stats.pieces[3], 2);
        assert_eq!(stats.plies.counts[0], 3);

        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["outcomes"]["whiteWins"], 1);
        assert_eq!(json["scores"]["width"], 100);
    }
}
