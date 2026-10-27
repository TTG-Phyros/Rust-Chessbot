//! Profile of a player over their analysed games: results and accuracy,
//! repertoire, recurring positions, errors by phase, time management,
//! conversion of won positions, tactical motifs, and their main weaknesses.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use chess_chesscom::PlayedGame;
use chess_core::pgn::TimeControl;
use chess_core::{Color, Position};
use chess_openings::Book;
use chess_review::classify::ClassCounts;
use chess_review::motifs::{Motif, Phase, PhaseRules, motif, phase};
use chess_review::{GameAnalysis, MoveClass, Thresholds, review_game};
use serde::{Deserialize, Serialize};

/// Settings of the profiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileConfig {
    /// Last move number of the opening phase.
    pub opening_moves: u16,
    /// Most knights, bishops, rooks and queens (both sides) of an endgame.
    pub endgame_pieces: u32,
    /// Winning chances (0-100) from which the player had a won game.
    pub winning: f64,
    /// Winning chances up to which the player had a lost game.
    pub losing: f64,
    /// Share of the starting time below which the player is short of time.
    pub time_trouble: f64,
    /// Positions before this ply count for the recurring positions.
    pub line_plies: usize,
    /// Least games a recurring position appears in.
    pub line_games: u32,
    /// Examples kept for each weakness.
    pub examples: usize,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        let rules = PhaseRules::default();
        Self {
            opening_moves: rules.opening_moves,
            endgame_pieces: rules.endgame_pieces,
            winning: 80.0,
            losing: 20.0,
            time_trouble: 0.1,
            line_plies: 30,
            line_games: 2,
            examples: 3,
        }
    }
}

impl ProfileConfig {
    /// Where the phases begin.
    #[must_use]
    pub const fn phase_rules(&self) -> PhaseRules {
        PhaseRules {
            opening_moves: self.opening_moves,
            endgame_pieces: self.endgame_pieces,
        }
    }
}

/// A move of one of the games, shown as an example.
#[derive(Debug, Clone, PartialEq)]
pub struct GameRef {
    /// The game.
    pub game_id: String,
    /// Index of the move.
    pub ply: usize,
    /// The move with its number: `12... Nf3`.
    pub label: String,
    /// Its class.
    pub class: MoveClass,
    /// Winning chances it lost.
    pub loss: f64,
}

/// Wins, draws and losses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Record {
    /// Games won.
    pub wins: u32,
    /// Games drawn.
    pub draws: u32,
    /// Games lost.
    pub losses: u32,
}

impl Record {
    fn add(&mut self, score: f64) {
        if score > 0.75 {
            self.wins += 1;
        } else if score > 0.25 {
            self.draws += 1;
        } else {
            self.losses += 1;
        }
    }

    /// Games counted.
    #[must_use]
    pub const fn games(&self) -> u32 {
        self.wins + self.draws + self.losses
    }
}

/// An opening family played with one colour.
#[derive(Debug, Clone, PartialEq)]
pub struct OpeningStats {
    /// Family (`Sicilian Defense`).
    pub family: String,
    /// Its most frequent ECO code in these games.
    pub eco: String,
    /// Results.
    pub record: Record,
    /// Mean accuracy of the player in these games.
    pub accuracy: Option<f64>,
    /// The most recent of these games.
    pub last_game: String,
}

/// A position the player reached in several games, with what they played.
#[derive(Debug, Clone, PartialEq)]
pub struct LineStats {
    /// Side of the player.
    pub color: Color,
    /// Moves (SAN) from the start to the position.
    pub moves: Vec<String>,
    /// The position.
    pub fen: String,
    /// Games that reached it.
    pub games: u32,
    /// Times the move played there was an inaccuracy or worse.
    pub errors: u32,
    /// The engine's move (SAN).
    pub best: Option<String>,
    /// Moves played there (SAN), most frequent first.
    pub played: Vec<(String, u32)>,
    /// Winning chances lost there in all.
    pub lost: f64,
    /// The worst of these moves.
    pub examples: Vec<GameRef>,
}

/// Moves of the player in one phase.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseStats {
    /// The phase.
    pub phase: Phase,
    /// Moves played in it.
    pub moves: u32,
    /// Mean accuracy of these moves.
    pub accuracy: Option<f64>,
    /// Average centipawn loss.
    pub acpl: Option<f64>,
    /// Moves of each class.
    pub counts: ClassCounts,
    /// Winning chances lost by its mistakes, misses and blunders.
    pub lost: f64,
    /// Mean thinking time per move, in seconds (games with clocks).
    pub seconds_per_move: Option<f64>,
}

/// Use of the clock (games with an increment or not, not daily ones).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TimeStats {
    /// Games with clocks.
    pub games: u32,
    /// Mean thinking time per move, in seconds.
    pub seconds_per_move: f64,
    /// Games where the player was short of time at some point.
    pub time_trouble_games: u32,
    /// Moves played short of time, and the mistakes among them.
    pub low_moves: u32,
    /// Mistakes, misses and blunders played short of time.
    pub low_errors: u32,
    /// Moves played with enough time.
    pub normal_moves: u32,
    /// Mistakes, misses and blunders played with enough time.
    pub normal_errors: u32,
    /// Games lost on time.
    pub timeouts: u32,
}

/// Won positions converted, lost ones saved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Conversion {
    /// Games where the player was winning at some point.
    pub winning: u32,
    /// Of those, games won.
    pub converted: u32,
    /// Games where the player was lost at some point.
    pub losing: u32,
    /// Of those, games drawn or won.
    pub saved: u32,
}

/// The mistakes of one tactical motif.
#[derive(Debug, Clone, PartialEq)]
pub struct MotifStats {
    /// The motif.
    pub motif: Motif,
    /// Mistakes, misses and blunders with it.
    pub count: u32,
    /// Winning chances they lost.
    pub lost: f64,
    /// The worst of them.
    pub examples: Vec<GameRef>,
}

/// What a weakness is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaknessKind {
    /// More errors than usual in this phase.
    Phase(Phase),
    /// Errors of this tactical motif.
    Motif(Motif),
    /// Errors when short of time.
    TimeTrouble,
    /// Won positions not won.
    Conversion,
    /// Errors in a recurring position (index in [`Profile::lines`]).
    Line(usize),
}

/// One of the main weaknesses of the player.
#[derive(Debug, Clone, PartialEq)]
pub struct Weakness {
    /// What it is about.
    pub kind: WeaknessKind,
    /// Winning chances it costs per game, on average: how weaknesses rank.
    pub lost_per_game: f64,
    /// Moves (or games, for the conversion) concerned.
    pub count: u32,
    /// Moves showing it.
    pub examples: Vec<GameRef>,
}

/// One game in the profile.
#[derive(Debug, Clone, PartialEq)]
pub struct GameStats {
    /// The game.
    pub id: String,
    /// Side of the player.
    pub color: Color,
    /// Their accuracy.
    pub accuracy: Option<f64>,
    /// Their average centipawn loss.
    pub acpl: Option<f64>,
}

/// Profile of a player.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// The player.
    pub username: String,
    /// Analysed games counted.
    pub games: u32,
    /// Games left out: unreadable, or not played by them.
    pub skipped: u32,
    /// Results.
    pub record: Record,
    /// Mean accuracy over the games.
    pub accuracy: Option<f64>,
    /// Average centipawn loss over all their moves.
    pub acpl: Option<f64>,
    /// Their moves.
    pub moves: u32,
    /// Their moves of each class.
    pub counts: ClassCounts,
    /// Their rating after the most recent game.
    pub rating: Option<i32>,
    /// Mean rating of their opponents.
    pub opponents: Option<f64>,
    /// Openings with White, then with Black, most played first.
    pub repertoire: [Vec<OpeningStats>; 2],
    /// Recurring positions where they went wrong, costliest first.
    pub lines: Vec<LineStats>,
    /// Their moves by phase.
    pub phases: Vec<PhaseStats>,
    /// Their use of the clock, if any game had clocks.
    pub time: Option<TimeStats>,
    /// Conversion of won positions, defence of lost ones.
    pub conversion: Conversion,
    /// Their mistakes by motif, most frequent first.
    pub motifs: Vec<MotifStats>,
    /// Their three main weaknesses, costliest first.
    pub weaknesses: Vec<Weakness>,
    /// Each game, as given.
    pub per_game: Vec<GameStats>,
}

const fn is_error(class: MoveClass) -> bool {
    matches!(
        class,
        MoveClass::Mistake | MoveClass::Miss | MoveClass::Blunder
    )
}

const fn is_inaccurate(class: MoveClass) -> bool {
    matches!(class, MoveClass::Inaccuracy) || is_error(class)
}

/// The `count` costliest of `refs`.
fn worst(mut refs: Vec<GameRef>, count: usize) -> Vec<GameRef> {
    refs.sort_by(|a, b| b.loss.total_cmp(&a.loss));
    refs.truncate(count);
    refs
}

#[derive(Default)]
struct OpeningAcc {
    ecos: BTreeMap<String, u32>,
    record: Record,
    accuracy: (f64, u32),
    last_game: Option<String>,
}

#[derive(Default)]
struct LineAcc {
    color: Option<Color>,
    moves: Vec<String>,
    fen: String,
    games: u32,
    errors: u32,
    best: Option<String>,
    played: BTreeMap<String, u32>,
    lost: f64,
    examples: Vec<GameRef>,
}

#[derive(Default)]
struct PhaseAcc {
    moves: u32,
    accuracy: f64,
    cp: f64,
    counts: ClassCounts,
    lost: f64,
    seconds: (f64, u32),
    errors: Vec<GameRef>,
}

#[derive(Default)]
struct MotifAcc {
    count: u32,
    lost: f64,
    examples: Vec<GameRef>,
}

/// What is known of a game being added to the profile.
struct GameContext<'a> {
    game: &'a PlayedGame,
    analysis: &'a GameAnalysis,
    side: Color,
    review: chess_review::GameReview,
}

/// Builds the profile of `username` from their analysed games (most recent
/// first).
#[must_use]
pub fn build_profile(
    username: &str,
    games: &[(PlayedGame, GameAnalysis)],
    thresholds: &Thresholds,
    config: &ProfileConfig,
) -> Profile {
    let mut builder = Builder::new(thresholds, config);
    for (game, analysis) in games {
        builder.add(username, game, analysis);
    }
    builder.finish(username)
}

struct Builder<'a> {
    thresholds: &'a Thresholds,
    config: &'a ProfileConfig,
    games: u32,
    skipped: u32,
    record: Record,
    accuracy: (f64, u32),
    cp: f64,
    moves: u32,
    counts: ClassCounts,
    rating: Option<i32>,
    opponents: (f64, u32),
    openings: HashMap<(Color, String), OpeningAcc>,
    lines: HashMap<String, LineAcc>,
    phases: BTreeMap<Phase, PhaseAcc>,
    time: TimeStats,
    time_seconds: (f64, u32),
    low_lost: f64,
    low_examples: Vec<GameRef>,
    conversion: Conversion,
    wasted: f64,
    wasted_examples: Vec<GameRef>,
    motifs: BTreeMap<Motif, MotifAcc>,
    per_game: Vec<GameStats>,
}

impl<'a> Builder<'a> {
    fn new(thresholds: &'a Thresholds, config: &'a ProfileConfig) -> Self {
        Self {
            thresholds,
            config,
            games: 0,
            skipped: 0,
            record: Record::default(),
            accuracy: (0.0, 0),
            cp: 0.0,
            moves: 0,
            counts: ClassCounts::default(),
            rating: None,
            opponents: (0.0, 0),
            openings: HashMap::new(),
            lines: HashMap::new(),
            phases: Phase::ALL
                .into_iter()
                .map(|phase| (phase, PhaseAcc::default()))
                .collect(),
            time: TimeStats::default(),
            time_seconds: (0.0, 0),
            low_lost: 0.0,
            low_examples: Vec::new(),
            conversion: Conversion::default(),
            wasted: 0.0,
            wasted_examples: Vec::new(),
            motifs: BTreeMap::new(),
            per_game: Vec::new(),
        }
    }

    fn add(&mut self, username: &str, game: &PlayedGame, analysis: &GameAnalysis) {
        let Some(side) = game.color_of(username) else {
            self.skipped += 1;
            return;
        };
        let start = game.game.start();
        let moves = game.game.moves();
        let opening = Book::lichess().classify(start, moves);
        let Ok(review) = review_game(
            start,
            moves,
            &analysis.positions,
            opening.book_plies,
            self.thresholds,
        ) else {
            self.skipped += 1;
            return;
        };
        self.games += 1;
        let score = game.score(side);
        if let Some(score) = score {
            self.record.add(score);
        }
        let player = review.player(side);
        if let Some(accuracy) = player.accuracy {
            self.accuracy.0 += accuracy;
            self.accuracy.1 += 1;
        }
        self.per_game.push(GameStats {
            id: game.id.clone(),
            color: side,
            accuracy: player.accuracy,
            acpl: player.acpl,
        });
        self.rating.get_or_insert(game.player(side).rating);
        self.opponents.0 += f64::from(game.player(!side).rating);
        self.opponents.1 += 1;

        if let Some(found) = opening.opening {
            let entry = self
                .openings
                .entry((side, found.family().to_owned()))
                .or_default();
            *entry.ecos.entry(found.eco.clone()).or_default() += 1;
            if let Some(score) = score {
                entry.record.add(score);
            }
            if let Some(accuracy) = player.accuracy {
                entry.accuracy.0 += accuracy;
                entry.accuracy.1 += 1;
            }
            entry.last_game.get_or_insert_with(|| game.id.clone());
        }

        let context = GameContext {
            game,
            analysis,
            side,
            review,
        };
        self.add_conversion(&context, score);
        self.add_moves(&context, score);
    }

    /// Whether the player had a won (or lost) game, and what came of it.
    fn add_conversion(&mut self, context: &GameContext<'_>, score: Option<f64>) {
        let Some(score) = score else {
            return;
        };
        let chances: Vec<f64> = context
            .review
            .white_wins
            .iter()
            .skip(1)
            .map(|&white| match context.side {
                Color::White => white,
                Color::Black => 100.0 - white,
            })
            .collect();
        let peak = chances.iter().copied().fold(f64::NAN, f64::max);
        let low = chances.iter().copied().fold(f64::NAN, f64::min);
        if peak >= self.config.winning {
            self.conversion.winning += 1;
            if score > 0.75 {
                self.conversion.converted += 1;
            } else {
                self.wasted += peak - score * 100.0;
                // The costliest move of the player once they were winning.
                let start = chances
                    .iter()
                    .position(|&chance| chance >= self.config.winning)
                    .unwrap_or(0);
                let first = context.game.game.start().side_to_move();
                let mut refs = self.refs(context, first);
                refs.retain(|reference| reference.ply >= start);
                self.wasted_examples.extend(worst(refs, 1));
            }
        }
        if low <= self.config.losing {
            self.conversion.losing += 1;
            if score > 0.25 {
                self.conversion.saved += 1;
            }
        }
    }

    /// The player's moves of a game, as examples.
    fn refs(&self, context: &GameContext<'_>, first: Color) -> Vec<GameRef> {
        let mut position = context.game.game.start().without_history();
        let mut refs = Vec::new();
        for (ply, &mv) in context.game.game.moves().iter().enumerate() {
            let mover = if ply % 2 == 0 { first } else { !first };
            if mover == context.side
                && let Some(verdict) = context.review.moves.get(ply)
            {
                refs.push(GameRef {
                    game_id: context.game.id.clone(),
                    ply,
                    label: label(&position, mv),
                    class: verdict.class,
                    loss: verdict.loss(),
                });
            }
            position.make_move(mv);
        }
        refs
    }

    fn add_moves(&mut self, context: &GameContext<'_>, score: Option<f64>) {
        let game = context.game;
        let rules = self.config.phase_rules();
        let (base, increment) = match game.time_control {
            Some(TimeControl::Fischer { base, increment }) => (Some(base), increment),
            _ => (None, Duration::ZERO),
        };
        let mut clock = base;
        let mut clocked = false;
        let mut trouble = false;
        let mut seen = HashSet::new();
        let mut sans = Vec::new();
        let mut position = game.game.start().without_history();
        for (ply, &mv) in game.game.moves().iter().enumerate() {
            let san = position.san(mv);
            if position.side_to_move() == context.side
                && let Some(verdict) = context.review.moves.get(ply)
            {
                let class = verdict.class;
                let loss = verdict.loss();
                let error = is_error(class);
                let reference = GameRef {
                    game_id: game.id.clone(),
                    ply,
                    label: label(&position, mv),
                    class,
                    loss,
                };
                self.moves += 1;
                self.cp += f64::from(verdict.cp_loss);
                self.counts.add(class);

                // Thinking time and time trouble.
                let mut spent = None;
                if let (Some(base), Some(available), Some(after)) =
                    (base, clock, game.clocks.get(ply).copied().flatten())
                {
                    clocked = true;
                    let seconds = (available + increment).saturating_sub(after).as_secs_f64();
                    spent = Some(seconds);
                    self.time_seconds.0 += seconds;
                    self.time_seconds.1 += 1;
                    if available.as_secs_f64() < base.as_secs_f64() * self.config.time_trouble {
                        trouble = true;
                        self.time.low_moves += 1;
                        if error {
                            self.time.low_errors += 1;
                            self.low_lost += loss;
                            self.low_examples.push(reference.clone());
                        }
                    } else {
                        self.time.normal_moves += 1;
                        self.time.normal_errors += u32::from(error);
                    }
                }
                clock = game.clocks.get(ply).copied().flatten();

                let stage = phase(&position, &rules);
                let acc = self.phases.entry(stage).or_default();
                acc.moves += 1;
                acc.accuracy += verdict.accuracy;
                acc.cp += f64::from(verdict.cp_loss);
                acc.counts.add(class);
                if let Some(seconds) = spent {
                    acc.seconds.0 += seconds;
                    acc.seconds.1 += 1;
                }
                if error {
                    acc.lost += loss;
                    acc.errors.push(reference.clone());
                    let evals = &context.analysis.positions;
                    if let (Some(eval), Some(next)) = (evals.get(ply), evals.get(ply + 1)) {
                        let found = motif(&position, mv, [eval, next], self.thresholds.sacrifice);
                        let acc = self.motifs.entry(found).or_default();
                        acc.count += 1;
                        acc.lost += loss;
                        acc.examples.push(reference.clone());
                    }
                }

                // Recurring positions of the first moves.
                if ply < self.config.line_plies {
                    let key = position.epd();
                    let first_visit = seen.insert(key.clone());
                    let acc = self.lines.entry(key).or_default();
                    if acc.color.is_none() {
                        acc.color = Some(context.side);
                        acc.moves.clone_from(&sans);
                        acc.fen = position.to_fen();
                        acc.best = context
                            .analysis
                            .positions
                            .get(ply)
                            .and_then(|eval| eval.best_move())
                            .and_then(|uci| position.parse_uci_move(uci).ok())
                            .map(|best| position.san(best));
                    }
                    if first_visit {
                        acc.games += 1;
                    }
                    *acc.played.entry(san.clone()).or_default() += 1;
                    if is_inaccurate(class) {
                        acc.errors += 1;
                        acc.lost += loss;
                        acc.examples.push(reference);
                    }
                }
            }
            sans.push(san);
            position.make_move(mv);
        }
        if clocked {
            self.time.games += 1;
            self.time.time_trouble_games += u32::from(trouble);
            if score.is_some_and(|score| score < 0.25)
                && game.player(context.side).result == "timeout"
            {
                self.time.timeouts += 1;
            }
        }
    }

    fn finish(self, username: &str) -> Profile {
        let games = f64::from(self.games.max(1));
        let examples = self.config.examples;
        let mean = |(sum, count): (f64, u32)| (count > 0).then(|| sum / f64::from(count));

        let mut repertoire: [Vec<OpeningStats>; 2] = [Vec::new(), Vec::new()];
        for ((color, family), acc) in self.openings {
            let eco = acc
                .ecos
                .iter()
                .max_by_key(|&(_, count)| *count)
                .map(|(eco, _)| eco.clone())
                .unwrap_or_default();
            repertoire[color.index()].push(OpeningStats {
                family,
                eco,
                record: acc.record,
                accuracy: mean(acc.accuracy),
                last_game: acc.last_game.unwrap_or_default(),
            });
        }
        for openings in &mut repertoire {
            openings.sort_by(|a, b| {
                b.record
                    .games()
                    .cmp(&a.record.games())
                    .then_with(|| a.family.cmp(&b.family))
            });
        }

        let mut lines: Vec<LineStats> = self
            .lines
            .into_values()
            .filter(|acc| acc.games >= self.config.line_games && acc.errors > 0)
            .map(|acc| {
                let mut played: Vec<(String, u32)> = acc.played.into_iter().collect();
                played.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                LineStats {
                    color: acc.color.unwrap_or(Color::White),
                    moves: acc.moves,
                    fen: acc.fen,
                    games: acc.games,
                    errors: acc.errors,
                    best: acc.best,
                    played,
                    lost: acc.lost,
                    examples: worst(acc.examples, examples),
                }
            })
            .collect();
        lines.sort_by(|a, b| b.lost.total_cmp(&a.lost));
        lines.truncate(10);

        let total_lost: f64 = self.phases.values().map(|acc| acc.lost).sum();
        let total_moves = f64::from(self.moves.max(1));
        let mut weaknesses = Vec::new();
        let mut phases = Vec::new();
        for (stage, acc) in self.phases {
            // Errors beyond the player's own rate in the other phases.
            let excess = acc.lost - total_lost * f64::from(acc.moves) / total_moves;
            if excess > 0.0 {
                weaknesses.push(Weakness {
                    kind: WeaknessKind::Phase(stage),
                    lost_per_game: excess / games,
                    count: u32::try_from(acc.errors.len()).unwrap_or(u32::MAX),
                    examples: worst(acc.errors, examples),
                });
            }
            phases.push(PhaseStats {
                phase: stage,
                moves: acc.moves,
                accuracy: mean((acc.accuracy, acc.moves)),
                acpl: mean((acc.cp, acc.moves)),
                counts: acc.counts,
                lost: acc.lost,
                seconds_per_move: mean(acc.seconds),
            });
        }

        let mut motifs: Vec<MotifStats> = self
            .motifs
            .into_iter()
            .map(|(found, acc)| MotifStats {
                motif: found,
                count: acc.count,
                lost: acc.lost,
                examples: worst(acc.examples, examples),
            })
            .collect();
        motifs.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| b.lost.total_cmp(&a.lost))
        });
        for stats in &motifs {
            if stats.motif != Motif::Other {
                weaknesses.push(Weakness {
                    kind: WeaknessKind::Motif(stats.motif),
                    lost_per_game: stats.lost / games,
                    count: stats.count,
                    examples: stats.examples.clone(),
                });
            }
        }

        let mut time = self.time;
        time.seconds_per_move = mean(self.time_seconds).unwrap_or(0.0);
        let rate = |errors: u32, moves: u32| f64::from(errors) / f64::from(moves.max(1));
        if time.low_moves >= 10
            && rate(time.low_errors, time.low_moves) > rate(time.normal_errors, time.normal_moves)
        {
            weaknesses.push(Weakness {
                kind: WeaknessKind::TimeTrouble,
                lost_per_game: self.low_lost / games,
                count: time.low_errors,
                examples: worst(self.low_examples, examples),
            });
        }
        if self.conversion.converted < self.conversion.winning {
            weaknesses.push(Weakness {
                kind: WeaknessKind::Conversion,
                lost_per_game: self.wasted / games,
                count: self.conversion.winning - self.conversion.converted,
                examples: worst(self.wasted_examples, examples),
            });
        }
        for (index, line) in lines.iter().enumerate() {
            weaknesses.push(Weakness {
                kind: WeaknessKind::Line(index),
                lost_per_game: line.lost / games,
                count: line.errors,
                examples: line.examples.clone(),
            });
        }
        weaknesses.retain(|weakness| weakness.lost_per_game > 0.0);
        weaknesses.sort_by(|a, b| b.lost_per_game.total_cmp(&a.lost_per_game));
        weaknesses.truncate(3);

        Profile {
            username: username.to_owned(),
            games: self.games,
            skipped: self.skipped,
            record: self.record,
            accuracy: mean(self.accuracy),
            acpl: mean((self.cp, self.moves)),
            moves: self.moves,
            counts: self.counts,
            rating: self.rating,
            opponents: mean(self.opponents),
            repertoire,
            lines,
            phases,
            time: (time.games > 0).then_some(time),
            conversion: self.conversion,
            motifs,
            weaknesses,
            per_game: self.per_game,
        }
    }
}

/// `12. Nf3` or `12... Nf3`.
fn label(position: &Position, mv: chess_core::Move) -> String {
    let dots = match position.side_to_move() {
        Color::White => ".",
        Color::Black => "...",
    };
    format!("{}{dots} {}", position.fullmove_number(), position.san(mv))
}

#[cfg(test)]
mod tests {
    use chess_chesscom::{ApiGame, ApiPlayer, TimeClass};
    use chess_core::Move;
    use chess_review::{Budget, Line, PositionEval, Score, SearchSettings};

    use super::*;

    fn api(id: &str, white: &str, black: &str, result: &str, movetext: &str) -> ApiGame {
        let pgn = format!(
            "[White \"{white}\"]\n[Black \"{black}\"]\n[Result \"{result}\"]\n\
             [TimeControl \"180\"]\n\n{movetext} {result}\n"
        );
        let end = |won: bool| if won { "win" } else { "resigned" }.to_owned();
        ApiGame {
            url: format!("https://www.chess.com/game/live/{id}"),
            uuid: id.into(),
            pgn: Some(pgn),
            time_control: "180".into(),
            end_time: 1_790_900_000,
            rated: true,
            time_class: TimeClass::Blitz,
            rules: "chess".into(),
            white: ApiPlayer {
                username: white.into(),
                rating: 1500,
                result: end(result == "1-0"),
            },
            black: ApiPlayer {
                username: black.into(),
                rating: 1400,
                result: end(result == "0-1"),
            },
            accuracies: None,
        }
    }

    /// An analysis where every move played is the best one, at `white_cp`
    /// (White's view, one per position), but at the plies of `wrong` where
    /// the best move was another one.
    fn analysis(game: &PlayedGame, white_cp: &[i32], wrong: &[(usize, &str)]) -> GameAnalysis {
        let moves: &[Move] = game.game.moves();
        let mut position = game.game.start().without_history();
        let mut positions = Vec::new();
        for ply in 0..=moves.len() {
            if ply > 0 {
                position.make_move(moves[ply - 1]);
            }
            let cp = white_cp[ply] * position.side_to_move().sign();
            let best = wrong
                .iter()
                .find(|&&(at, _)| at == ply)
                .map(|&(_, uci)| uci.to_owned())
                .or_else(|| moves.get(ply).map(ToString::to_string))
                .or_else(|| {
                    position
                        .legal_moves()
                        .iter()
                        .next()
                        .map(ToString::to_string)
                });
            positions.push(PositionEval {
                lines: best
                    .map(|uci| Line {
                        score: Score::Cp(cp),
                        depth: 10,
                        pv: vec![uci],
                    })
                    .into_iter()
                    .collect(),
                checkmated: false,
            });
        }
        GameAnalysis {
            engine: "test".into(),
            settings: SearchSettings {
                budget: Budget::Depth(10),
                multipv: 1,
            },
            positions,
        }
    }

    /// Alice falls twice for the Blackburne Shilling trap (4. Nxe5?? Qg5),
    /// short of time in the first game; draws a Queen's Gambit with Black.
    fn games() -> Vec<(PlayedGame, GameAnalysis)> {
        let trap = "1. e4 {[%clk 0:02:59]} 1... e5 {[%clk 0:02:59]} 2. Nf3 {[%clk 0:02:55]} \
            2... Nc6 {[%clk 0:02:58]} 3. Bc4 {[%clk 0:00:15]} 3... Nd4 {[%clk 0:02:57]} \
            4. Nxe5 {[%clk 0:00:12]} 4... Qg5 {[%clk 0:02:50]} 5. Nxf7 {[%clk 0:00:10]} \
            5... Qxg2 {[%clk 0:02:49]}";
        let first = PlayedGame::from_api(&api("a", "Alice", "Bob", "0-1", trap)).unwrap();
        let first_cp = [30, 30, 30, 30, 30, 30, 30, -300, -300, -500, -500];
        let again = "1. e4 e5 2. Nf3 Nc6 3. Bc4 Nd4 4. Nxe5 Qg5";
        let second = PlayedGame::from_api(&api("b", "alice", "Bob", "0-1", again)).unwrap();
        let second_cp = [30, 30, 30, 30, 30, 30, 30, -300, -300];
        let black = "1. d4 d5 2. c4 e6";
        let third = PlayedGame::from_api(&api("c", "Carol", "Alice", "1/2-1/2", black)).unwrap();
        let others = PlayedGame::from_api(&api("d", "Carol", "Dave", "1-0", black)).unwrap();
        vec![
            (first.clone(), analysis(&first, &first_cp, &[(6, "f3d4")])),
            (
                second.clone(),
                analysis(&second, &second_cp, &[(6, "f3d4")]),
            ),
            (third.clone(), analysis(&third, &[20; 5], &[])),
            (others.clone(), analysis(&others, &[20; 5], &[])),
        ]
    }

    fn profile() -> Profile {
        build_profile(
            "ALICE",
            &games(),
            &Thresholds::default(),
            &ProfileConfig::default(),
        )
    }

    #[test]
    fn results_and_repertoire() {
        let profile = profile();
        assert_eq!((profile.games, profile.skipped), (3, 1));
        assert_eq!(
            profile.record,
            Record {
                wins: 0,
                draws: 1,
                losses: 2
            }
        );
        assert_eq!(profile.rating, Some(1500));
        let white = &profile.repertoire[Color::White.index()];
        assert_eq!(white.len(), 1);
        assert_eq!(
            (white[0].family.as_str(), white[0].eco.as_str()),
            ("Italian Game", "C50")
        );
        assert_eq!(white[0].record.losses, 2);
        assert_eq!(white[0].last_game, "a");
        assert_eq!(profile.repertoire[Color::Black.index()].len(), 1);
        assert_eq!(profile.counts.get(MoveClass::Blunder), 2);
        assert_eq!(profile.per_game.len(), 3);
        assert_eq!(profile.per_game[2].color, Color::Black);
    }

    #[test]
    fn the_trap_is_a_recurring_position_and_a_fork_allowed() {
        let profile = profile();
        assert_eq!(profile.lines.len(), 1);
        let line = &profile.lines[0];
        assert_eq!(line.moves, ["e4", "e5", "Nf3", "Nc6", "Bc4", "Nd4"]);
        assert_eq!((line.games, line.errors), (2, 2));
        assert_eq!(line.best.as_deref(), Some("Nxd4"));
        assert_eq!(line.played, [("Nxe5".to_owned(), 2)]);
        assert_eq!(line.examples[0].label, "4. Nxe5");

        // 4... Qg5 attacks the knight and g2.
        assert_eq!(profile.motifs.len(), 1);
        assert_eq!(profile.motifs[0].motif, Motif::AllowedFork);
        assert_eq!(profile.motifs[0].count, 2);

        let kinds: Vec<WeaknessKind> = profile.weaknesses.iter().map(|w| w.kind).collect();
        assert!(kinds.contains(&WeaknessKind::Motif(Motif::AllowedFork)));
        assert!(kinds.contains(&WeaknessKind::Line(0)));
        let weakness = &profile.weaknesses[0];
        assert!((weakness.lost_per_game - line.lost / 3.0).abs() < 1e-9);
        assert_eq!(weakness.examples[0].ply, 6);
    }

    #[test]
    fn phases_time_and_conversion() {
        let profile = profile();
        let opening = &profile.phases[0];
        assert_eq!(opening.phase, Phase::Opening);
        // 5 + 4 moves with White, 2 with Black.
        assert_eq!(opening.moves, 11);
        assert_eq!(opening.counts.get(MoveClass::Blunder), 2);

        // Only the first game has clocks: 1, 4, 160, 3 and 2 seconds.
        let time = profile.time.unwrap();
        assert_eq!(time.games, 1);
        assert!((time.seconds_per_move - 34.0).abs() < 1e-9);
        assert_eq!((time.low_moves, time.low_errors), (2, 1));
        assert_eq!((time.normal_moves, time.normal_errors), (3, 0));
        assert_eq!(time.time_trouble_games, 1);
        assert_eq!(time.timeouts, 0);

        // Never winning; lost at -5 (14 %) in the first game, not saved;
        // -3 (25 %) in the second is not lost yet.
        assert_eq!(
            profile.conversion,
            Conversion {
                winning: 0,
                converted: 0,
                losing: 1,
                saved: 0
            }
        );
    }
}
