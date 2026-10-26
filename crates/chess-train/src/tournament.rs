//! Round-robin tournaments between bots (saved networks and the classical
//! evaluation): every pairing is a match of the arena, from the same random
//! openings with colors swapped, at a fixed number of nodes per move.
//!
//! Progress is rewritten to `<data_dir>/tournaments/<id>.json` after every
//! batch of games, in camelCase for `chess-server`.

use std::path::PathBuf;

use chess_search::StopFlag;
use serde::{Deserialize, Serialize};

use crate::arena::{self, ArenaConfig, Contestant, MatchResult, MoveLimit};
use crate::config::Config;
use crate::metrics::now;
use crate::network::{self, CLASSIC};
use crate::stockfish;
use crate::trainer::write_atomically;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TournamentRequest {
    /// Bot names; [`CLASSIC`] is the classical evaluation, `stockfish-1` to
    /// `stockfish-10` the Stockfish levels.
    pub(crate) bots: Vec<String>,
    /// Games of each pairing (rounded up to an even number).
    pub(crate) games_per_pair: u32,
    /// How long our bots think per move (Stockfish plays at its level).
    pub(crate) limit: MoveLimit,
    /// Worker threads; 0 = all cores.
    #[serde(default)]
    pub(crate) threads: usize,
    /// The pairings to play (in any order, either way round); absent: every
    /// pair of bots.
    #[serde(default)]
    pub(crate) pairings: Option<Vec<PlannedPairing>>,
}

/// Two bots of the request that play each other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PlannedPairing {
    pub(crate) a: String,
    pub(crate) b: String,
}

impl TournamentRequest {
    /// Indices `(a, b)`, `a < b`, of the pairings to play, in playing order
    /// (that of the bots). Expects a validated request.
    pub(crate) fn schedule(&self) -> Vec<(usize, usize)> {
        let n = self.bots.len();
        let wanted = |a: usize, b: usize| {
            self.pairings.as_ref().is_none_or(|pairings| {
                pairings.iter().any(|p| {
                    let (x, y) = (&self.bots[a], &self.bots[b]);
                    (p.a == *x && p.b == *y) || (p.a == *y && p.b == *x)
                })
            })
        };
        (0..n)
            .flat_map(|a| (a + 1..n).map(move |b| (a, b)))
            .filter(|&(a, b)| wanted(a, b))
            .collect()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if !(2..=16).contains(&self.bots.len()) {
            return Err("a tournament needs 2 to 16 bots".into());
        }
        for (i, bot) in self.bots.iter().enumerate() {
            if !bot.eq_ignore_ascii_case(CLASSIC) && stockfish::level_of(bot).is_none() {
                network::validate_name(bot)?;
            }
            if self.bots[..i]
                .iter()
                .any(|other| other.eq_ignore_ascii_case(bot))
            {
                return Err(format!("{bot} is listed twice"));
            }
        }
        if let Some(pairings) = &self.pairings {
            if pairings.is_empty() {
                return Err("no pairing to play".into());
            }
            let mut seen = Vec::new();
            for pairing in pairings {
                let index = |name: &str| self.bots.iter().position(|bot| bot == name);
                let (Some(a), Some(b)) = (index(&pairing.a), index(&pairing.b)) else {
                    return Err(format!(
                        "{} vs {}: both must take part in the tournament",
                        pairing.a, pairing.b
                    ));
                };
                if a == b {
                    return Err(format!("{} cannot play itself", pairing.a));
                }
                let key = (a.min(b), a.max(b));
                if seen.contains(&key) {
                    return Err(format!("{} vs {} is listed twice", pairing.a, pairing.b));
                }
                seen.push(key);
            }
        }
        if !(2..=2_000).contains(&self.games_per_pair) {
            return Err("games per pairing must be between 2 and 2 000".into());
        }
        self.limit.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TournamentState {
    Running,
    Finished,
    Stopped,
    Failed,
}

/// Result of `a` against `b`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Pairing {
    pub(crate) a: String,
    pub(crate) b: String,
    pub(crate) wins: u32,
    pub(crate) draws: u32,
    pub(crate) losses: u32,
    /// Elo of `a` over `b`, with the half-width of its 95 % interval.
    pub(crate) elo: f64,
    pub(crate) elo_error: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Standing {
    pub(crate) bot: String,
    pub(crate) games: u32,
    /// 1 per win, ½ per draw.
    pub(crate) points: f64,
    /// Elo fitted on every game of the tournament: relative to the classical
    /// evaluation when it takes part, to the field's average otherwise.
    pub(crate) elo: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tournament {
    pub(crate) id: String,
    pub(crate) state: TournamentState,
    pub(crate) error: Option<String>,
    /// Unix seconds.
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
    pub(crate) bots: Vec<String>,
    pub(crate) games_per_pair: u32,
    /// How long our bots think per move.
    pub(crate) limit: MoveLimit,
    /// Every pairing, in playing order; the ones not reached yet have no games.
    pub(crate) pairings: Vec<Pairing>,
    /// Best first.
    pub(crate) standings: Vec<Standing>,
}

impl Tournament {
    fn new(id: String, request: &TournamentRequest) -> Self {
        let bots = &request.bots;
        let pairings = request
            .schedule()
            .into_iter()
            .map(|(a, b)| Pairing {
                a: bots[a].clone(),
                b: bots[b].clone(),
                wins: 0,
                draws: 0,
                losses: 0,
                elo: 0.0,
                elo_error: 0.0,
            })
            .collect();
        let mut tournament = Self {
            id,
            state: TournamentState::Running,
            error: None,
            created_at: now(),
            updated_at: now(),
            bots: bots.clone(),
            games_per_pair: request.games_per_pair.div_ceil(2) * 2,
            limit: request.limit,
            pairings,
            standings: Vec::new(),
        };
        tournament.standings = standings(&tournament.bots, &tournament.pairings);
        tournament
    }

    fn set_result(&mut self, pairing: usize, result: MatchResult) {
        if let Some(pairing) = self.pairings.get_mut(pairing) {
            pairing.wins = result.wins;
            pairing.draws = result.draws;
            pairing.losses = result.losses;
            if result.games() > 0 {
                pairing.elo = result.elo();
                pairing.elo_error = result.elo_error();
            }
        }
        self.standings = standings(&self.bots, &self.pairings);
        self.updated_at = now();
    }
}

/// Points, games and fitted Elo of every bot, best first.
///
/// The Elo comes from a Bradley-Terry fit (minorization-maximization) on
/// every game, which accounts for the strength of the opponents. One virtual
/// draw per pairing played keeps perfect and null scores finite.
fn standings(bots: &[String], pairings: &[Pairing]) -> Vec<Standing> {
    let n = bots.len();
    let index = |name: &str| bots.iter().position(|bot| bot == name);
    let mut games = vec![vec![0.0_f64; n]; n];
    let mut points = vec![0.0_f64; n];
    let mut played = vec![0_u32; n];
    for pairing in pairings {
        let (Some(i), Some(j)) = (index(&pairing.a), index(&pairing.b)) else {
            continue;
        };
        let count = pairing.wins + pairing.draws + pairing.losses;
        if count == 0 {
            continue;
        }
        let draws = 0.5 * f64::from(pairing.draws);
        games[i][j] += f64::from(count);
        games[j][i] += f64::from(count);
        points[i] += f64::from(pairing.wins) + draws;
        points[j] += f64::from(pairing.losses) + draws;
        played[i] += count;
        played[j] += count;
    }

    let mut gamma = vec![1.0_f64; n];
    for _ in 0..10_000 {
        let mut next = gamma.clone();
        for i in 0..n {
            let (mut score, mut weight) = (points[i], 0.0);
            for j in (0..n).filter(|&j| j != i && games[i][j] > 0.0) {
                score += 0.5;
                weight += (games[i][j] + 1.0) / (gamma[i] + gamma[j]);
            }
            if weight > 0.0 {
                next[i] = score / weight;
            }
        }
        // Ratings are relative: fix the geometric mean at 1.
        let mean = next.iter().map(|g| g.ln()).sum::<f64>() / n.max(1) as f64;
        next.iter_mut().for_each(|g| *g /= mean.exp());
        let change = next
            .iter()
            .zip(&gamma)
            .map(|(a, b)| (a / b).ln().abs())
            .fold(0.0, f64::max);
        gamma = next;
        if change < 1e-10 {
            break;
        }
    }
    let elo: Vec<f64> = gamma.iter().map(|g| 400.0 * g.log10()).collect();
    let anchor = bots
        .iter()
        .position(|bot| bot.eq_ignore_ascii_case(CLASSIC))
        .map_or(0.0, |i| elo[i]);

    let mut standings: Vec<Standing> = bots
        .iter()
        .enumerate()
        .map(|(i, bot)| Standing {
            bot: bot.clone(),
            games: played[i],
            points: points[i],
            elo: elo[i] - anchor,
        })
        .collect();
    standings.sort_by(|a, b| b.elo.total_cmp(&a.elo).then(b.points.total_cmp(&a.points)));
    standings
}

/// A new tournament id (`t<unix seconds>`), unused in `dir`.
pub(crate) fn new_id(config: &Config) -> String {
    let base = format!("t{}", now());
    let dir = config.tournaments_dir();
    (0..)
        .map(|n| {
            if n == 0 {
                base.clone()
            } else {
                format!("{base}-{n}")
            }
        })
        .find(|id| !dir.join(format!("{id}.json")).exists())
        .unwrap_or(base)
}

fn path(config: &Config, id: &str) -> PathBuf {
    config.tournaments_dir().join(format!("{id}.json"))
}

fn save(config: &Config, tournament: &Tournament) {
    let written = serde_json::to_vec_pretty(tournament)
        .map_err(anyhow::Error::from)
        .and_then(|bytes| write_atomically(&path(config, &tournament.id), &bytes));
    if let Err(err) = written {
        tracing::warn!(error = %format!("{err:#}"), "cannot write the tournament");
    }
}

/// Plays the tournament `id` between `engines` (in the order of
/// `request.bots`) until it ends or `stop` is raised.
pub(crate) fn play(
    config: &Config,
    id: String,
    request: &TournamentRequest,
    engines: &[Contestant],
    stop: &StopFlag,
) -> Tournament {
    let mut tournament = Tournament::new(id, request);
    save(config, &tournament);
    tracing::info!(id = %tournament.id, bots = ?request.bots, limit = %request.limit, "tournament started");
    let arena = ArenaConfig {
        games: tournament.games_per_pair,
        limit: Some(request.limit),
        threads: request.threads,
        sprt: false,
        ..config.arena.clone()
    };
    for (pairing, (a, b)) in request.schedule().into_iter().enumerate() {
        if stop.is_stopped() {
            break;
        }
        let result = arena::play_match(&engines[a], &engines[b], &arena, stop, |result| {
            tournament.set_result(pairing, result);
            save(config, &tournament);
        });
        match result {
            Ok(result) => tournament.set_result(pairing, result),
            Err(err) => {
                tournament.state = TournamentState::Failed;
                tournament.error = Some(format!("{err:#}"));
                save(config, &tournament);
                return tournament;
            }
        }
    }
    tournament.state = if stop.is_stopped() {
        TournamentState::Stopped
    } else {
        TournamentState::Finished
    };
    tournament.updated_at = now();
    save(config, &tournament);
    if let Some(winner) = tournament.standings.first() {
        tracing::info!(id = %tournament.id, state = ?tournament.state, winner = %winner.bot, "tournament over");
    }
    tournament
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir;

    fn pairing(a: &str, b: &str, wins: u32, draws: u32, losses: u32) -> Pairing {
        Pairing {
            a: a.into(),
            b: b.into(),
            wins,
            draws,
            losses,
            elo: 0.0,
            elo_error: 0.0,
        }
    }

    #[test]
    fn requests_are_validated() {
        let ok = TournamentRequest {
            bots: vec!["classic".into(), "best".into(), "stockfish-10".into()],
            games_per_pair: 20,
            limit: MoveLimit::Nodes(2_000),
            threads: 0,
            pairings: None,
        };
        assert_eq!(ok.validate(), Ok(()));
        let invalid = [
            TournamentRequest {
                bots: vec!["best".into()],
                ..ok.clone()
            },
            TournamentRequest {
                bots: vec!["best".into(), "best".into()],
                ..ok.clone()
            },
            TournamentRequest {
                bots: vec!["best".into(), "../etc".into()],
                ..ok.clone()
            },
            TournamentRequest {
                games_per_pair: 0,
                ..ok.clone()
            },
            TournamentRequest {
                bots: vec!["best".into(), "stockfish-11".into()],
                ..ok.clone()
            },
            TournamentRequest {
                limit: MoveLimit::Nodes(1),
                ..ok.clone()
            },
            TournamentRequest {
                limit: MoveLimit::Time(0),
                ..ok
            },
        ];
        for request in invalid {
            assert!(request.validate().is_err(), "{request:?}");
        }
    }

    #[test]
    fn chosen_pairings_only() {
        let planned = |a: &str, b: &str| PlannedPairing {
            a: a.into(),
            b: b.into(),
        };
        let request = TournamentRequest {
            bots: ["a", "b", "c", "stockfish-1"].map(String::from).into(),
            games_per_pair: 2,
            limit: MoveLimit::Depth(1),
            threads: 1,
            pairings: None,
        };
        assert_eq!(request.schedule().len(), 6, "every pair by default");
        // Our bots against Stockfish only, listed either way round.
        let against_stockfish = TournamentRequest {
            pairings: Some(vec![
                planned("stockfish-1", "a"),
                planned("b", "stockfish-1"),
                planned("c", "stockfish-1"),
            ]),
            ..request.clone()
        };
        assert_eq!(against_stockfish.validate(), Ok(()));
        assert_eq!(against_stockfish.schedule(), [(0, 3), (1, 3), (2, 3)]);
        let tournament = Tournament::new("t".into(), &against_stockfish);
        assert_eq!(tournament.pairings.len(), 3);
        assert_eq!(
            (
                tournament.pairings[0].a.as_str(),
                tournament.pairings[0].b.as_str()
            ),
            ("a", "stockfish-1")
        );
        assert_eq!(tournament.standings.len(), 4);

        for pairings in [
            vec![],
            vec![planned("a", "zz")],
            vec![planned("a", "a")],
            vec![planned("a", "b"), planned("b", "a")],
        ] {
            let invalid = TournamentRequest {
                pairings: Some(pairings.clone()),
                ..request.clone()
            };
            assert!(invalid.validate().is_err(), "{pairings:?}");
        }
    }

    #[test]
    fn standings_fit_elo_on_every_game() {
        let bots: Vec<String> = ["classic", "strong", "weak"].map(String::from).into();
        // strong beats classic 75 %, classic beats weak 75 %, strong beats weak.
        let pairings = [
            pairing("classic", "strong", 20, 20, 60),
            pairing("classic", "weak", 60, 20, 20),
            pairing("strong", "weak", 80, 10, 10),
        ];
        let table = standings(&bots, &pairings);
        assert_eq!(
            table.iter().map(|s| s.bot.as_str()).collect::<Vec<_>>(),
            ["strong", "classic", "weak"]
        );
        let elo = |bot: &str| table.iter().find(|s| s.bot == bot).unwrap().elo;
        assert!(
            elo("classic").abs() < 1e-9,
            "anchored on the classical evaluation"
        );
        // 70 % is +147 Elo.
        assert!(
            elo("strong") > 120.0 && elo("strong") < 200.0,
            "{}",
            elo("strong")
        );
        assert!(
            (elo("strong") + elo("weak")).abs() < 1.0,
            "symmetric results"
        );
        assert_eq!(table[0].games, 200);
        assert!((table[0].points - 155.0).abs() < 1e-9);

        // A perfect score stays finite; bots without games sit at 0.
        let pairings = [
            pairing("classic", "strong", 0, 0, 10),
            pairing("classic", "weak", 0, 0, 0),
            pairing("strong", "weak", 0, 0, 0),
        ];
        let table = standings(&bots, &pairings);
        assert!(table.iter().all(|s| s.elo.is_finite()));
        assert_eq!(table[0].bot, "strong");
        let weak = table.iter().find(|s| s.bot == "weak").unwrap();
        assert_eq!(weak.games, 0);
    }

    #[test]
    fn plays_a_small_tournament() {
        let dir = test_dir("tournament");
        let mut config = Config {
            data_dir: dir,
            ..Config::default()
        };
        config.arena.max_plies = 60;
        let request = TournamentRequest {
            bots: vec!["classic".into(), "a".into(), "b".into()],
            games_per_pair: 2,
            limit: MoveLimit::Depth(2),
            threads: 2,
            pairings: None,
        };
        let engines = vec![Contestant::Engine(chess_eval::EngineEval::default()); 3];
        let id = new_id(&config);
        let tournament = play(&config, id.clone(), &request, &engines, &StopFlag::new());
        assert_eq!(tournament.state, TournamentState::Finished);
        assert_eq!(tournament.pairings.len(), 3);
        assert!(
            tournament
                .pairings
                .iter()
                .all(|p| p.wins + p.draws + p.losses == 2)
        );
        assert!(tournament.standings.iter().all(|s| s.games == 4));

        let saved: Tournament =
            serde_json::from_slice(&std::fs::read(path(&config, &id)).unwrap()).unwrap();
        assert_eq!(saved, tournament);
        assert_ne!(new_id(&config), id);

        let stop = StopFlag::new();
        stop.stop();
        let stopped = play(&config, new_id(&config), &request, &engines, &stop);
        assert_eq!(stopped.state, TournamentState::Stopped);
    }
}
