//! Classification of the moves of an analysed game and accuracy of the
//! players, in the manner of Chess.com's *Game Review*.
//!
//! Every move is judged by the winning chances ([`win_percent`]) it loses:
//! those of the best line before it, against those after it (the move's own
//! line when the engine listed it, so that both come from the same search;
//! otherwise the evaluation of the next position).

use chess_core::{Color, Move, Position, Square};
use serde::{Deserialize, Serialize};

use crate::accuracy::{centipawns, game_accuracy, move_accuracy, win_percent};
use crate::analysis::PositionEval;
use crate::engine::Score;
use crate::exchange;

/// Class of a move, from the best to the worst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoveClass {
    /// A sound sacrifice: the best move or nearly, giving material away,
    /// neither in a position already won nor still lost after it.
    Brilliant,
    /// The only good move: the best one, when the second best would change
    /// the outcome of the game (from won to drawn, or drawn to lost).
    Great,
    /// The engine's best move, or as good.
    Best,
    /// Nearly as good as the best.
    Excellent,
    /// A sound move.
    Good,
    /// Opening theory.
    Book,
    /// Some advantage lost.
    Inaccuracy,
    /// A clear advantage lost.
    Mistake,
    /// A chance offered by the opponent's blunder, let go.
    Miss,
    /// A decisive advantage lost.
    Blunder,
}

impl MoveClass {
    /// Every class, from the best to the worst.
    pub const ALL: [Self; 10] = [
        Self::Brilliant,
        Self::Great,
        Self::Best,
        Self::Excellent,
        Self::Good,
        Self::Book,
        Self::Inaccuracy,
        Self::Mistake,
        Self::Miss,
        Self::Blunder,
    ];

    /// Lowercase name (`brilliant`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Brilliant => "brilliant",
            Self::Great => "great",
            Self::Best => "best",
            Self::Excellent => "excellent",
            Self::Good => "good",
            Self::Book => "book",
            Self::Inaccuracy => "inaccuracy",
            Self::Mistake => "mistake",
            Self::Miss => "miss",
            Self::Blunder => "blunder",
        }
    }
}

/// Thresholds of the classification. Losses and chances are winning
/// chances of the player who moves, from 0 to 100.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    /// Most a Best move may lose (other than the engine's first choice).
    pub best: f64,
    /// Most an Excellent move may lose.
    pub excellent: f64,
    /// Most a Good move may lose.
    pub good: f64,
    /// Most an Inaccuracy may lose.
    pub inaccuracy: f64,
    /// Most a Mistake may lose; a Blunder loses more.
    pub mistake: f64,
    /// Chances from which a position is won.
    pub winning: f64,
    /// Chances up to which a position is lost.
    pub losing: f64,
    /// Least gap between the best line and the second for an only move
    /// (Great).
    pub only_move: f64,
    /// Least material a sacrifice gives away (Brilliant), in centipawns.
    pub sacrifice: i32,
    /// Most a Brilliant move may lose.
    pub brilliant_loss: f64,
    /// Chances from which the game is won without the move (with the best
    /// other one): no Brilliant then.
    pub won: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        // Chess.com's bands of expected points (0.02, 0.05, 0.10, 0.20).
        Self {
            best: 0.5,
            excellent: 2.0,
            good: 5.0,
            inaccuracy: 10.0,
            mistake: 20.0,
            winning: 70.0,
            losing: 30.0,
            only_move: 10.0,
            sacrifice: 200,
            brilliant_loss: 2.0,
            won: 90.0,
        }
    }
}

impl Thresholds {
    /// Checks that the thresholds are ordered and within 0–100.
    ///
    /// # Errors
    ///
    /// A message naming the first threshold out of place.
    pub fn check(&self) -> Result<(), String> {
        let losses = [
            ("best", self.best),
            ("excellent", self.excellent),
            ("good", self.good),
            ("inaccuracy", self.inaccuracy),
            ("mistake", self.mistake),
        ];
        let mut previous = 0.0;
        for (name, loss) in losses {
            if !(previous..=100.0).contains(&loss) {
                return Err(format!(
                    "classification.{name} = {loss}: losses must grow, between 0 and 100"
                ));
            }
            previous = loss;
        }
        if !(0.0 < self.losing && self.losing < self.winning && self.winning <= self.won)
            || self.won > 100.0
        {
            return Err("classification: 0 < losing < winning <= won <= 100".into());
        }
        if !(0.0..=100.0).contains(&self.only_move) || !(0.0..=100.0).contains(&self.brilliant_loss)
        {
            return Err("classification: only_move and brilliant_loss within 0-100".into());
        }
        if self.sacrifice <= 0 {
            return Err("classification.sacrifice must be positive".into());
        }
        Ok(())
    }
}

/// Review of one move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveReview {
    /// Its class.
    pub class: MoveClass,
    /// Winning chances of the player before the move, with the best move.
    pub win_before: f64,
    /// Their winning chances after it.
    pub win_after: f64,
    /// Accuracy of the move ([`move_accuracy`]).
    pub accuracy: f64,
    /// Centipawns lost (evaluations capped at ±10 pawns, mates included).
    pub cp_loss: i32,
}

impl MoveReview {
    /// Winning chances lost.
    #[must_use]
    pub fn loss(&self) -> f64 {
        (self.win_before - self.win_after).max(0.0)
    }
}

/// Number of moves of each class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClassCounts([u32; MoveClass::ALL.len()]);

impl ClassCounts {
    /// Moves of `class`.
    #[must_use]
    pub const fn get(&self, class: MoveClass) -> u32 {
        self.0[class as usize]
    }

    /// Counts one more move of `class`.
    pub const fn add(&mut self, class: MoveClass) {
        self.0[class as usize] += 1;
    }
}

/// Review of the moves of one player.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerReview {
    /// Moves played.
    pub moves: u32,
    /// Accuracy over the game ([`game_accuracy`]); `None` without moves.
    pub accuracy: Option<f64>,
    /// Average centipawn loss; `None` without moves.
    pub acpl: Option<f64>,
    /// Moves of each class.
    pub counts: ClassCounts,
}

/// Review of a game.
#[derive(Debug, Clone, PartialEq)]
pub struct GameReview {
    /// One per move.
    pub moves: Vec<MoveReview>,
    /// White's winning chances in each position (one more than the moves).
    pub white_wins: Vec<f64>,
    /// The players, White first.
    pub players: [PlayerReview; 2],
}

impl GameReview {
    /// Review of the moves of `color`.
    #[must_use]
    pub const fn player(&self, color: Color) -> &PlayerReview {
        &self.players[color.index()]
    }
}

/// The opponent's move before the one being judged.
#[derive(Debug, Clone, Copy)]
struct Previous {
    /// Winning chances it lost.
    loss: f64,
    /// Square of the piece it captured.
    captured_on: Option<Square>,
}

/// Reviews the `moves` played from `start`, given the evaluation of every
/// position (one more than the moves, as [`crate::analyse_game`] makes
/// them); the first `book_plies` moves are opening theory.
///
/// # Errors
///
/// When the evaluations do not match the moves, or a move is illegal.
pub fn review_game(
    start: &Position,
    moves: &[Move],
    positions: &[PositionEval],
    book_plies: usize,
    thresholds: &Thresholds,
) -> anyhow::Result<GameReview> {
    anyhow::ensure!(
        positions.len() == moves.len() + 1,
        "{} evaluations for {} moves",
        positions.len(),
        moves.len()
    );
    let mut position = start.without_history();
    let first = position.side_to_move();
    let mut white_wins = Vec::with_capacity(positions.len());
    let mut reviews: Vec<MoveReview> = Vec::with_capacity(moves.len());
    let mut previous = None;
    for (ply, (&mv, pair)) in moves.iter().zip(positions.windows(2)).enumerate() {
        anyhow::ensure!(position.is_legal(mv), "illegal move {mv} at ply {ply}");
        white_wins.push(white(&position, &pair[0]));
        let review = review_move(
            &position,
            mv,
            [&pair[0], &pair[1]],
            ply < book_plies,
            previous,
            thresholds,
        );
        previous = Some(Previous {
            loss: review.loss(),
            captured_on: mv.is_capture().then(|| mv.to()),
        });
        reviews.push(review);
        position.make_move(mv);
    }
    if let Some(last) = positions.last() {
        white_wins.push(white(&position, last));
    }

    let accuracies: Vec<f64> = reviews.iter().map(|review| review.accuracy).collect();
    let accuracy = game_accuracy(&accuracies, first);
    let mut players: [PlayerReview; 2] = Default::default();
    let mut cp_losses = [0.0; 2];
    for (ply, review) in reviews.iter().enumerate() {
        let mover = if ply % 2 == 0 { first } else { !first };
        let player = &mut players[mover.index()];
        player.moves += 1;
        player.counts.add(review.class);
        cp_losses[mover.index()] += f64::from(review.cp_loss);
    }
    for (index, player) in players.iter_mut().enumerate() {
        player.accuracy = accuracy[index];
        player.acpl = (player.moves > 0).then(|| cp_losses[index] / f64::from(player.moves));
    }
    Ok(GameReview {
        moves: reviews,
        white_wins,
        players,
    })
}

/// White's winning chances in `position`, evaluated as `eval`.
fn white(position: &Position, eval: &PositionEval) -> f64 {
    let win = win_percent(eval.score());
    match position.side_to_move() {
        Color::White => win,
        Color::Black => 100.0 - win,
    }
}

/// Reviews `mv`, played in `position`; `evals` are the evaluations of the
/// position and of the next one.
fn review_move(
    position: &Position,
    mv: Move,
    [eval, next]: [&PositionEval; 2],
    book: bool,
    previous: Option<Previous>,
    thresholds: &Thresholds,
) -> MoveReview {
    let uci = mv.to_string();
    let best = eval.score();
    let played = if next.checkmated {
        Score::Mate(1)
    } else {
        eval.lines
            .iter()
            .find(|line| line.pv.first() == Some(&uci))
            .map_or_else(|| next.score().negate(), |line| line.score)
    };
    let (before, after) = (win_percent(best), win_percent(played));
    let judged = Judged {
        position,
        mv,
        eval,
        is_best: eval.best_move() == Some(uci.as_str()),
        before,
        after,
        loss: (before - after).max(0.0),
        previous,
    };
    MoveReview {
        class: if book {
            MoveClass::Book
        } else {
            judged.class(thresholds)
        },
        win_before: before,
        win_after: after,
        accuracy: move_accuracy(before, after),
        cp_loss: (centipawns(best) - centipawns(played)).max(0),
    }
}

/// What a move is judged on.
struct Judged<'a> {
    position: &'a Position,
    mv: Move,
    eval: &'a PositionEval,
    /// The engine's first choice.
    is_best: bool,
    /// Winning chances of the player with the best move, and after this one.
    before: f64,
    after: f64,
    loss: f64,
    previous: Option<Previous>,
}

impl Judged<'_> {
    fn class(&self, t: &Thresholds) -> MoveClass {
        if self.is_brilliant(t) {
            MoveClass::Brilliant
        } else if self.is_best && self.is_only_move(t) && !self.is_obvious() {
            MoveClass::Great
        } else if self.is_miss(t) {
            MoveClass::Miss
        } else if self.is_best || self.loss <= t.best {
            MoveClass::Best
        } else if self.loss <= t.excellent {
            MoveClass::Excellent
        } else if self.loss <= t.good {
            MoveClass::Good
        } else if self.loss <= t.inaccuracy {
            MoveClass::Inaccuracy
        } else if self.loss <= t.mistake {
            MoveClass::Mistake
        } else {
            MoveClass::Blunder
        }
    }

    /// A sacrifice that keeps (nearly) the best chances, unless the game
    /// was won even without it (the best other line wins) or is still lost
    /// after it. Never out of check: giving material there is forced.
    fn is_brilliant(&self, t: &Thresholds) -> bool {
        let uci = self.mv.to_string();
        let without = self
            .eval
            .lines
            .iter()
            .find(|line| line.pv.first() != Some(&uci))
            .map_or(self.before, |line| win_percent(line.score));
        self.loss <= t.brilliant_loss
            && without < t.won
            && self.after > t.losing
            && !self.position.in_check()
            && self.position.legal_moves().len() > 1
            && exchange::sacrifice(self.position, self.mv) >= t.sacrifice
    }

    /// The best line is far ahead of the second, and the second leads to
    /// another outcome (won, drawn or lost).
    fn is_only_move(&self, t: &Thresholds) -> bool {
        let [first, second, ..] = self.eval.lines.as_slice() else {
            return false;
        };
        let (best, other) = (win_percent(first.score), win_percent(second.score));
        let outcome = |win: f64| {
            if win >= t.winning {
                1
            } else if win <= t.losing {
                -1
            } else {
                0
            }
        };
        best - other >= t.only_move && outcome(best) != outcome(other)
    }

    /// A move anyone finds, however forced: taking back a piece just
    /// captured, or moving away a piece under attack. Not taking a piece
    /// left hanging nor answering a check: Chess.com counts them as Great.
    fn is_obvious(&self) -> bool {
        let recapture = self.mv.is_capture()
            && self
                .previous
                .is_some_and(|previous| previous.captured_on == Some(self.mv.to()));
        recapture || exchange::threat(self.position, self.mv.from()) > 0
    }

    /// After the opponent's blunder, more than an inaccuracy lost: the
    /// chance it offered let go, whatever the position (Chess.com calls a
    /// Miss what would otherwise be a Mistake or a Blunder there, even when
    /// the game is lost after it).
    fn is_miss(&self, t: &Thresholds) -> bool {
        self.loss > t.inaccuracy
            && self
                .previous
                .is_some_and(|previous| previous.loss > t.mistake)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Line;

    fn line(uci: &str, score: Score) -> Line {
        Line {
            score,
            depth: 20,
            pv: vec![uci.to_owned()],
        }
    }

    fn eval(lines: Vec<Line>) -> PositionEval {
        PositionEval {
            lines,
            checkmated: false,
        }
    }

    /// The position after `moves` (SAN) from the start.
    fn after(moves: &str) -> Position {
        let mut position = Position::startpos();
        for san in moves.split_whitespace() {
            let mv = position.parse_san(san).unwrap();
            position.make_move(mv);
        }
        position.without_history()
    }

    fn review(
        start: &Position,
        moves: &[&str],
        positions: &[PositionEval],
        book: usize,
    ) -> GameReview {
        let mut position = start.clone();
        let moves: Vec<Move> = moves
            .iter()
            .map(|uci| {
                let mv = position.parse_uci_move(uci).unwrap();
                position.make_move(mv);
                mv
            })
            .collect();
        review_game(start, &moves, positions, book, &Thresholds::default()).unwrap()
    }

    fn classes(review: &GameReview) -> Vec<MoveClass> {
        review.moves.iter().map(|review| review.class).collect()
    }

    /// Legal's mate: 5. Nxe5!! leaves the queen to the pinning bishop
    /// (5... Bxd1 6. Bxf7+ Ke7 7. Nd5#).
    fn legal(best: i32, second: i32) -> GameReview {
        let start = after("e4 e5 Nf3 d6 Bc4 Bg4 Nc3 g6");
        let positions = [
            eval(vec![
                line("f3e5", Score::Cp(best)),
                line("h2h3", Score::Cp(second)),
            ]),
            eval(vec![line("d6e5", Score::Cp(-best))]),
        ];
        review(&start, &["f3e5"], &positions, 0)
    }

    #[test]
    fn a_queen_sacrifice_is_brilliant() {
        let review = legal(250, 60);
        assert_eq!(classes(&review), [MoveClass::Brilliant]);
        assert_eq!(review.moves[0].cp_loss, 0);
        assert!((review.moves[0].accuracy - 100.0).abs() < 1e-9);
    }

    #[test]
    fn no_brilliant_when_won_already_or_still_lost() {
        // +8: the game was won anyway.
        assert_eq!(classes(&legal(800, 700)), [MoveClass::Best]);
        // -4: still lost after the sacrifice.
        assert_eq!(classes(&legal(-400, -450)), [MoveClass::Best]);
    }

    /// The Saavedra position: only 1. c7! wins, the other moves draw.
    fn saavedra(lines: Vec<Line>) -> GameReview {
        let start = Position::from_fen("8/8/1KP5/3r4/8/8/8/k7 w - - 0 1").unwrap();
        let positions = [eval(lines), eval(vec![line("d5d6", Score::Cp(-600))])];
        review(&start, &["c6c7"], &positions, 0)
    }

    #[test]
    fn the_only_winning_move_is_great() {
        let review = saavedra(vec![
            line("c6c7", Score::Cp(600)),
            line("b6c7", Score::Cp(0)),
        ]);
        assert_eq!(classes(&review), [MoveClass::Great]);
        // Without a second line, no way to tell.
        let review = saavedra(vec![line("c6c7", Score::Cp(600))]);
        assert_eq!(classes(&review), [MoveClass::Best]);
        // Two winning moves: neither is the only one.
        let review = saavedra(vec![
            line("c6c7", Score::Cp(600)),
            line("b6c7", Score::Cp(550)),
        ]);
        assert_eq!(classes(&review), [MoveClass::Best]);
    }

    #[test]
    fn obvious_moves_are_not_great() {
        // Taking a hanging knight, the only winning move: Great (as on
        // Chess.com).
        let start = Position::from_fen("4k3/8/8/3n4/8/8/8/3RK3 w - - 0 1").unwrap();
        let positions = [
            eval(vec![
                line("d1d5", Score::Cp(500)),
                line("e1e2", Score::Cp(0)),
            ]),
            eval(vec![line("e8e7", Score::Cp(-500))]),
        ];
        let taken = review(&start, &["d1d5"], &positions, 0);
        assert_eq!(classes(&taken), [MoveClass::Great]);

        // Saving the knight attacked by a pawn: forced, but anyone sees it.
        let start = Position::from_fen("4k3/8/4p3/3N4/8/8/8/4K3 w - - 0 1").unwrap();
        let positions = [
            eval(vec![
                line("d5c3", Score::Cp(250)),
                line("e1e2", Score::Cp(-50)),
            ]),
            eval(vec![line("e8e7", Score::Cp(-250))]),
        ];
        let saved = review(&start, &["d5c3"], &positions, 0);
        assert_eq!(classes(&saved), [MoveClass::Best]);

        // 2. exd5 Qxd5: taking back, forced, but anyone sees it.
        let start = after("e4 d5");
        let positions = [
            eval(vec![line("e4d5", Score::Cp(30))]),
            eval(vec![
                line("d8d5", Score::Cp(-30)),
                line("e7e6", Score::Cp(-350)),
            ]),
            eval(vec![line("b1c3", Score::Cp(30))]),
        ];
        let review = review(&start, &["e4d5", "d8d5"], &positions, 0);
        assert_eq!(classes(&review), [MoveClass::Best, MoveClass::Best]);
    }

    /// 1. e4 e5 2. Bc4 Nc6 3. Qh5, then 3... Nf6?? (instead of g6) and
    /// 4. Qe2? (instead of Qxf7#).
    fn scholar(book: usize) -> GameReview {
        let start = after("e4 e5 Bc4 Nc6 Qh5");
        let positions = [
            eval(vec![
                line("g7g6", Score::Cp(-20)),
                line("d8e7", Score::Cp(-40)),
            ]),
            eval(vec![
                line("h5f7", Score::Mate(1)),
                line("c4f7", Score::Cp(150)),
            ]),
            eval(vec![line("g7g6", Score::Cp(30))]),
        ];
        review(&start, &["g8f6", "h5e2"], &positions, book)
    }

    #[test]
    fn allowing_mate_is_a_blunder_and_missing_it_a_miss() {
        let review = scholar(0);
        assert_eq!(classes(&review), [MoveClass::Blunder, MoveClass::Miss]);
        let blunder = &review.moves[0];
        assert!((blunder.win_before - 48.1598).abs() < 1e-3);
        assert!(blunder.win_after.abs() < 1e-9);
        // From -0.2 to mated: 980 centipawns (mates count as 10 pawns).
        assert_eq!(blunder.cp_loss, 980);
        let miss = &review.moves[1];
        assert!((miss.win_before - 100.0).abs() < 1e-9);
        assert!((miss.win_after - 47.2412).abs() < 1e-3);

        let black = review.player(Color::Black);
        assert_eq!(black.moves, 1);
        assert_eq!(black.counts.get(MoveClass::Blunder), 1);
        assert_eq!(black.acpl, Some(980.0));
        // One move: the game accuracy is the move's (1 at least).
        assert!(blunder.accuracy < 1.0);
        assert!((black.accuracy.unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(review.player(Color::White).counts.get(MoveClass::Miss), 1);
        // Black to move first: White's chances are the complement.
        assert!((review.white_wins[0] - (100.0 - 48.1598)).abs() < 1e-3);
        assert!((review.white_wins[1] - 100.0).abs() < 1e-9);
        assert_eq!(review.white_wins.len(), 3);
    }

    #[test]
    fn a_miss_needs_a_blunder_but_may_lose_the_game() {
        let start = after("e4 e5 Bc4 Nc6 Qh5");
        let game = |chances: Score, after: i32| {
            let positions = [
                eval(vec![line("g7g6", Score::Cp(-20))]),
                eval(vec![line("h5f7", chances)]),
                eval(vec![line("g7g6", Score::Cp(after))]),
            ];
            classes(&review(&start, &["g8f6", "h5e2"], &positions, 0))
        };
        // Lost after letting the mate go: a Miss all the same.
        assert_eq!(
            game(Score::Mate(1), 400),
            [MoveClass::Blunder, MoveClass::Miss]
        );
        // After a mere Mistake (+1.5), letting it go is a Mistake.
        assert_eq!(
            game(Score::Cp(150), 30),
            [MoveClass::Mistake, MoveClass::Mistake]
        );
    }

    #[test]
    fn book_moves_are_book() {
        let review = scholar(1);
        assert_eq!(classes(&review), [MoveClass::Book, MoveClass::Miss]);
        // Still counted in the accuracy.
        assert!(review.moves[0].accuracy < 20.0);
    }

    #[test]
    fn mating_is_best() {
        let start = after("f3 e5 g4");
        let positions = [
            eval(vec![line("d8h4", Score::Mate(1))]),
            PositionEval {
                lines: Vec::new(),
                checkmated: true,
            },
        ];
        let review = review(&start, &["d8h4"], &positions, 0);
        assert_eq!(classes(&review), [MoveClass::Best]);
        assert_eq!(review.white_wins, [0.0, 0.0]);
    }

    #[test]
    fn losses_set_the_class() {
        // 1. e4 against the best line at +0.30: Excellent, Good, Inaccuracy,
        // Mistake and Blunder as the evaluation after it falls.
        let start = Position::startpos();
        let class = |after: i32| {
            let positions = [
                eval(vec![line("d2d4", Score::Cp(30))]),
                eval(vec![line("e7e5", Score::Cp(-after))]),
            ];
            classes(&review(&start, &["e2e4"], &positions, 0))[0]
        };
        assert_eq!(class(28), MoveClass::Best);
        assert_eq!(class(15), MoveClass::Excellent);
        assert_eq!(class(-10), MoveClass::Good);
        assert_eq!(class(-60), MoveClass::Inaccuracy);
        assert_eq!(class(-150), MoveClass::Mistake);
        assert_eq!(class(-400), MoveClass::Blunder);
    }

    #[test]
    fn evaluations_must_match_the_moves() {
        let start = Position::startpos();
        let e4 = start.parse_uci_move("e2e4").unwrap();
        let positions = [eval(Vec::new())];
        assert!(review_game(&start, &[e4], &positions, 0, &Thresholds::default()).is_err());
    }

    #[test]
    fn thresholds_are_checked() {
        assert_eq!(Thresholds::default().check(), Ok(()));
        let unordered = Thresholds {
            good: 30.0,
            ..Thresholds::default()
        };
        assert!(unordered.check().unwrap_err().contains("inaccuracy"));
        let bands = Thresholds {
            losing: 80.0,
            ..Thresholds::default()
        };
        assert!(bands.check().is_err());
    }
}
