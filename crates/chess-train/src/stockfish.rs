//! Stockfish as a tournament opponent: an external UCI process, weakened to
//! one of 10 levels in the manner of lichess (skill level, depth and time
//! per move grow together).
//!
//! The binary is `[arena] stockfish` in the configuration (`stockfish` on
//! the `PATH` by default; the Docker images ship Stockfish 19).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail};

/// Lowest and highest level.
pub(crate) const LEVELS: std::ops::RangeInclusive<u8> = 1..=10;

/// Bot name of a level: `stockfish-1` to `stockfish-10`.
pub(crate) fn bot_name(level: u8) -> String {
    format!("stockfish-{level}")
}

/// The level a bot name designates, if it is a Stockfish one.
pub(crate) fn level_of(name: &str) -> Option<u8> {
    let digits = name.strip_prefix("stockfish-")?;
    if digits.starts_with('0') {
        return None;
    }
    digits.parse().ok().filter(|level| LEVELS.contains(level))
}

/// How a level plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Level {
    /// UCI `Skill Level`, 0 to 20: below 20, Stockfish sometimes picks a
    /// weaker move on purpose.
    pub(crate) skill: u8,
    pub(crate) depth: u32,
    pub(crate) movetime: Duration,
}

/// Levels 1 to 8 search as deep and as long as lichess's 8 levels (depth 1
/// to 22, 50 ms to 1 s); 9 and 10 go further. The skill level rises evenly
/// over the 0..=20 range Stockfish accepts.
pub(crate) fn level(level: u8) -> Level {
    const TABLE: [(u8, u32, u64); 10] = [
        (0, 1, 50),
        (2, 1, 100),
        (4, 2, 150),
        (6, 3, 200),
        (8, 5, 300),
        (10, 8, 400),
        (13, 13, 500),
        (16, 22, 1_000),
        (18, 26, 1_500),
        (20, 30, 2_000),
    ];
    let (skill, depth, ms) = TABLE[usize::from(level.clamp(1, 10) - 1)];
    Level {
        skill,
        depth,
        movetime: Duration::from_millis(ms),
    }
}

/// A running Stockfish, one game at a time.
pub(crate) struct Stockfish {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    level: Level,
}

/// Score of the side to move, as Stockfish reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UciScore {
    Centipawns(i32),
    /// Mate in this many moves (negative: being mated).
    Mate(i32),
}

impl Stockfish {
    /// Starts `binary` at `level`, single-threaded with a small hash table.
    pub(crate) fn spawn(binary: &Path, level_number: u8) -> Result<Self> {
        let mut child = Command::new(binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("cannot start Stockfish ({})", binary.display()))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            bail!("no pipe to Stockfish");
        };
        let level = level(level_number);
        let mut engine = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            level,
        };
        engine.send("uci")?;
        engine.wait_for("uciok")?;
        engine.send("setoption name Threads value 1")?;
        engine.send("setoption name Hash value 16")?;
        engine.send(&format!("setoption name Skill Level value {}", level.skill))?;
        engine.ready()?;
        Ok(engine)
    }

    fn send(&mut self, command: &str) -> Result<()> {
        writeln!(self.stdin, "{command}")
            .and_then(|()| self.stdin.flush())
            .context("Stockfish stopped")
    }

    /// Reads lines until one starts with `prefix`, which it returns.
    fn wait_for(&mut self, prefix: &str) -> Result<String> {
        let mut line = String::new();
        loop {
            line.clear();
            if self
                .stdout
                .read_line(&mut line)
                .context("Stockfish stopped")?
                == 0
            {
                bail!("Stockfish exited");
            }
            if line.starts_with(prefix) {
                return Ok(line.trim_end().to_owned());
            }
        }
    }

    fn ready(&mut self) -> Result<()> {
        self.send("isready")?;
        self.wait_for("readyok").map(drop)
    }

    /// Forgets the previous game (hash table, history).
    pub(crate) fn new_game(&mut self) -> Result<()> {
        self.send("ucinewgame")?;
        self.ready()
    }

    /// Best move in UCI notation for the position reached by `moves` (UCI)
    /// from the starting position, with the last score reported.
    pub(crate) fn best_move(&mut self, moves: &[String]) -> Result<(String, Option<UciScore>)> {
        let mut position = String::from("position startpos");
        if !moves.is_empty() {
            position.push_str(" moves ");
            position.push_str(&moves.join(" "));
        }
        self.send(&position)?;
        self.send(&format!(
            "go depth {} movetime {}",
            self.level.depth,
            self.level.movetime.as_millis()
        ))?;
        let mut score = None;
        let mut line = String::new();
        loop {
            line.clear();
            if self
                .stdout
                .read_line(&mut line)
                .context("Stockfish stopped")?
                == 0
            {
                bail!("Stockfish exited during a search");
            }
            let mut words = line.split_whitespace();
            match words.next() {
                Some("info") => {
                    if let Some(parsed) = parse_score(&line) {
                        score = Some(parsed);
                    }
                }
                Some("bestmove") => {
                    let best = words.next().unwrap_or("(none)").to_owned();
                    return Ok((best, score));
                }
                _ => {}
            }
        }
    }
}

impl Drop for Stockfish {
    fn drop(&mut self) {
        let _ = self.send("quit");
        // Give it a moment to exit on its own, then make sure it does.
        for _ in 0..20 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `score cp 34` or `score mate -3` of an `info` line.
fn parse_score(line: &str) -> Option<UciScore> {
    let mut words = line.split_whitespace().skip_while(|word| *word != "score");
    words.next()?;
    let kind = words.next()?;
    let value: i32 = words.next()?.parse().ok()?;
    match kind {
        "cp" => Some(UciScore::Centipawns(value)),
        "mate" => Some(UciScore::Mate(value)),
        _ => None,
    }
}

/// Name and version of the Stockfish at `binary` (`id name` of its UCI
/// greeting), or `None` if it cannot be run.
pub(crate) fn probe(binary: &Path) -> Option<String> {
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let stdout = child.stdout.take()?;
    let _ = writeln!(stdin, "uci");
    let _ = stdin.flush();
    let mut name = None;
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if let Some(id) = line.strip_prefix("id name ") {
            name = Some(id.trim().to_owned());
        }
        if line.starts_with("uciok") {
            break;
        }
    }
    let _ = writeln!(stdin, "quit");
    drop(stdin);
    let _ = child.wait();
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_levels() {
        assert_eq!(level_of("stockfish-1"), Some(1));
        assert_eq!(level_of("stockfish-10"), Some(10));
        for name in [
            "stockfish-0",
            "stockfish-11",
            "stockfish-01",
            "stockfish",
            "sf-3",
        ] {
            assert_eq!(level_of(name), None, "{name}");
        }
        assert_eq!(bot_name(7), "stockfish-7");
        let levels: Vec<Level> = LEVELS.map(level).collect();
        assert!(levels.windows(2).all(|w| w[0].skill <= w[1].skill
            && w[0].depth <= w[1].depth
            && w[0].movetime <= w[1].movetime));
        assert_eq!((levels[0].skill, levels[9].skill), (0, 20));
        assert_eq!(
            levels[7].depth, 22,
            "level 8 searches like lichess's level 8"
        );
    }

    #[test]
    fn scores_are_parsed() {
        assert_eq!(
            parse_score("info depth 5 seldepth 7 multipv 1 score cp -34 nodes 812 pv e2e4"),
            Some(UciScore::Centipawns(-34))
        );
        assert_eq!(
            parse_score("info depth 9 score mate 3 pv d1h5"),
            Some(UciScore::Mate(3))
        );
        assert_eq!(parse_score("info string NNUE evaluation enabled"), None);
    }

    #[test]
    fn missing_binary_is_reported() {
        let missing = Path::new("/no/such/stockfish");
        assert!(probe(missing).is_none());
        assert!(Stockfish::spawn(missing, 1).is_err());
    }
}
