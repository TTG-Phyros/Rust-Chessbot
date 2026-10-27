//! Stockfish as an opponent or a reference: an external UCI process,
//! weakened to one of 10 levels in the manner of lichess (skill level,
//! depth and time per move grow together).
//!
//! Used by the arena of `chess-train` (tournaments) and by `chess-server`
//! (comparison with our engine on the Play page). The binary is configured
//! by each of them (`stockfish` on the `PATH` by default; the Docker images
//! ship Stockfish 19).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail};

/// Lowest and highest level.
pub const LEVELS: std::ops::RangeInclusive<u8> = 1..=10;

/// How a level plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    /// UCI `Skill Level`, 0 to 20: below 20, Stockfish sometimes picks a
    /// weaker move on purpose.
    pub skill: u8,
    /// Depth of its searches.
    pub depth: u32,
    /// Time of its searches.
    pub movetime: Duration,
}

/// Levels 1 to 8 search as deep and as long as lichess's 8 levels (depth 1
/// to 22, 50 ms to 1 s); 9 and 10 go further. The skill level rises evenly
/// over the 0..=20 range Stockfish accepts.
#[must_use]
pub fn level(level: u8) -> Level {
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

/// Score of the side to move, as Stockfish reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UciScore {
    /// Advantage in centipawns.
    Centipawns(i32),
    /// Mate in this many moves (negative: being mated).
    Mate(i32),
}

/// How long a search may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Budget {
    /// The depth and time of the level.
    Level,
    /// A fixed depth.
    Depth(u32),
    /// A fixed number of nodes.
    Nodes(u64),
    /// A fixed time.
    MoveTime(Duration),
}

/// Result of a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    /// Move played, in UCI notation (`(none)` when there is no legal move).
    pub best_move: String,
    /// Last score reported, for the side to move.
    pub score: Option<UciScore>,
    /// Last depth reported.
    pub depth: u32,
    /// Last principal variation reported, in UCI notation.
    pub pv: Vec<String>,
}

/// A running Stockfish.
pub struct Stockfish {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    level: Level,
}

impl Stockfish {
    /// Starts `binary` at `level_number`, single-threaded with a small hash table.
    ///
    /// # Errors
    ///
    /// When the binary cannot be started or does not speak UCI.
    pub fn spawn(binary: &Path, level_number: u8) -> Result<Self> {
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

    /// Reads one line; fails when Stockfish has exited.
    fn read_line(&mut self, line: &mut String) -> Result<()> {
        line.clear();
        if self.stdout.read_line(line).context("Stockfish stopped")? == 0 {
            bail!("Stockfish exited");
        }
        Ok(())
    }

    /// Reads lines until one starts with `prefix`, which it returns.
    fn wait_for(&mut self, prefix: &str) -> Result<String> {
        let mut line = String::new();
        loop {
            self.read_line(&mut line)?;
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
    ///
    /// # Errors
    ///
    /// When Stockfish has stopped.
    pub fn new_game(&mut self) -> Result<()> {
        self.send("ucinewgame")?;
        self.ready()
    }

    /// Best move in UCI notation for the position reached by `moves` (UCI)
    /// from the starting position, at the level's depth and time, with the
    /// last score reported.
    ///
    /// # Errors
    ///
    /// When Stockfish has stopped.
    pub fn best_move(&mut self, moves: &[String]) -> Result<(String, Option<UciScore>)> {
        let analysis = self.analyse(None, moves, Budget::Level)?;
        Ok((analysis.best_move, analysis.score))
    }

    /// Searches the position reached by `moves` (UCI) from `fen` (the
    /// starting position if `None`) within `budget`.
    ///
    /// # Errors
    ///
    /// When Stockfish has stopped.
    pub fn analyse(
        &mut self,
        fen: Option<&str>,
        moves: &[String],
        budget: Budget,
    ) -> Result<Analysis> {
        let mut position = match fen {
            Some(fen) => format!("position fen {fen}"),
            None => String::from("position startpos"),
        };
        if !moves.is_empty() {
            position.push_str(" moves ");
            position.push_str(&moves.join(" "));
        }
        self.send(&position)?;
        self.send(&match budget {
            Budget::Level => format!(
                "go depth {} movetime {}",
                self.level.depth,
                self.level.movetime.as_millis()
            ),
            Budget::Depth(depth) => format!("go depth {}", depth.max(1)),
            Budget::Nodes(nodes) => format!("go nodes {}", nodes.max(1)),
            Budget::MoveTime(time) => format!("go movetime {}", time.as_millis().max(1)),
        })?;
        let mut analysis = Analysis {
            best_move: String::new(),
            score: None,
            depth: 0,
            pv: Vec::new(),
        };
        let mut line = String::new();
        loop {
            self.read_line(&mut line)?;
            let mut words = line.split_whitespace();
            match words.next() {
                Some("info") => update(&mut analysis, &line),
                Some("bestmove") => {
                    analysis.best_move = words.next().unwrap_or("(none)").to_owned();
                    return Ok(analysis);
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

/// Takes the score, depth and PV of an `info` line (of its first line, in
/// multi-PV), when it has them.
fn update(analysis: &mut Analysis, line: &str) {
    if line.contains(" multipv ") && !line.contains(" multipv 1 ") {
        return;
    }
    if let Some(score) = parse_score(line) {
        analysis.score = Some(score);
    }
    let words: Vec<&str> = line.split_whitespace().collect();
    if let Some(depth) = words
        .iter()
        .position(|word| *word == "depth")
        .and_then(|i| words.get(i + 1)?.parse().ok())
    {
        analysis.depth = depth;
    }
    if let Some(start) = words.iter().position(|word| *word == "pv") {
        analysis.pv = words[start + 1..].iter().map(|&mv| mv.to_owned()).collect();
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
#[must_use]
pub fn probe(binary: &Path) -> Option<String> {
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
    fn levels_grow_together() {
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
    fn info_lines_update_the_analysis() {
        let mut analysis = Analysis {
            best_move: String::new(),
            score: None,
            depth: 0,
            pv: Vec::new(),
        };
        update(
            &mut analysis,
            "info depth 12 seldepth 18 multipv 1 score cp 41 nodes 9000 pv e2e4 e7e5 g1f3",
        );
        update(
            &mut analysis,
            "info depth 12 seldepth 16 multipv 2 score cp 20 nodes 9000 pv d2d4",
        );
        update(&mut analysis, "info string NNUE evaluation enabled");
        assert_eq!(analysis.score, Some(UciScore::Centipawns(41)));
        assert_eq!(analysis.depth, 12);
        assert_eq!(analysis.pv, ["e2e4", "e7e5", "g1f3"]);
    }

    #[test]
    fn missing_binary_is_reported() {
        let missing = Path::new("/no/such/stockfish");
        assert!(probe(missing).is_none());
        assert!(Stockfish::spawn(missing, 1).is_err());
    }
}
