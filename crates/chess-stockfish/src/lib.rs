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

/// One line of a multi-PV search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Score for the side to move.
    pub score: UciScore,
    /// Depth at which the line was reported.
    pub depth: u32,
    /// The line, in UCI notation, starting with its first move.
    pub pv: Vec<String>,
}

/// A running Stockfish.
pub struct Stockfish {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    level: Level,
    /// `id name` of its UCI greeting, e.g. `Stockfish 19`.
    name: String,
    /// Current `MultiPV` option.
    multipv: usize,
}

impl Stockfish {
    /// Starts `binary` at `level_number`, single-threaded with a small hash table.
    ///
    /// # Errors
    ///
    /// When the binary cannot be started or does not speak UCI.
    pub fn spawn(binary: &Path, level_number: u8) -> Result<Self> {
        let level = level(level_number);
        Self::start(binary, level, 1, 16, Some(level.skill))
    }

    /// Starts `binary` at full strength (no `Skill Level`), for analysis,
    /// with `threads` search threads and a `hash_mb` MiB hash table.
    ///
    /// # Errors
    ///
    /// When the binary cannot be started or does not speak UCI.
    pub fn spawn_full(binary: &Path, threads: usize, hash_mb: usize) -> Result<Self> {
        Self::start(binary, level(10), threads.max(1), hash_mb.max(1), None)
    }

    fn start(
        binary: &Path,
        level: Level,
        threads: usize,
        hash_mb: usize,
        skill: Option<u8>,
    ) -> Result<Self> {
        let mut child = Command::new(binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("cannot start Stockfish ({})", binary.display()))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            bail!("no pipe to Stockfish");
        };
        let mut engine = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            level,
            name: String::from("Stockfish"),
            multipv: 1,
        };
        engine.send("uci")?;
        let mut line = String::new();
        loop {
            engine.read_line(&mut line)?;
            if let Some(name) = line.strip_prefix("id name ") {
                engine.name = name.trim().to_owned();
            }
            if line.starts_with("uciok") {
                break;
            }
        }
        engine.send(&format!("setoption name Threads value {threads}"))?;
        engine.send(&format!("setoption name Hash value {hash_mb}"))?;
        if let Some(skill) = skill {
            engine.send(&format!("setoption name Skill Level value {skill}"))?;
        }
        engine.ready()?;
        Ok(engine)
    }

    /// `id name` of the engine, e.g. `Stockfish 19`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    fn set_multipv(&mut self, multipv: usize) -> Result<()> {
        let multipv = multipv.max(1);
        if multipv != self.multipv {
            self.send(&format!("setoption name MultiPV value {multipv}"))?;
            self.multipv = multipv;
        }
        Ok(())
    }

    /// Sends the position reached by `moves` from `fen` and starts the search.
    fn go(&mut self, fen: Option<&str>, moves: &[String], budget: Budget) -> Result<()> {
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
        })
    }

    /// The `multipv` best lines (fewer if there are fewer legal moves) of
    /// the position reached by `moves` (UCI) from `fen` (the starting
    /// position if `None`), best first, as last reported within `budget`.
    /// Empty when the side to move has no legal move.
    ///
    /// # Errors
    ///
    /// When Stockfish has stopped.
    pub fn analyse_lines(
        &mut self,
        fen: Option<&str>,
        moves: &[String],
        budget: Budget,
        multipv: usize,
    ) -> Result<Vec<Line>> {
        self.set_multipv(multipv)?;
        self.go(fen, moves, budget)?;
        let mut lines: Vec<Option<Line>> = vec![None; self.multipv];
        let mut text = String::new();
        loop {
            self.read_line(&mut text)?;
            if text.starts_with("bestmove") {
                return Ok(lines.into_iter().flatten().collect());
            }
            if let Some((index, line)) = parse_line(&text)
                && let Some(slot) = lines.get_mut(index)
            {
                *slot = Some(line);
            }
        }
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
        self.set_multipv(1)?;
        self.go(fen, moves, budget)?;
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

/// The 0-based multi-PV index and the line of an `info` line, if it holds
/// an exact score and a PV (aspiration-window bounds are skipped).
fn parse_line(text: &str) -> Option<(usize, Line)> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.first() != Some(&"info")
        || words
            .iter()
            .any(|word| *word == "lowerbound" || *word == "upperbound")
    {
        return None;
    }
    let after = |key: &str| {
        words
            .iter()
            .position(|word| *word == key)
            .and_then(|i| words.get(i + 1))
    };
    let index: usize = after("multipv").map_or(Some(1), |n| n.parse().ok())?;
    let depth = after("depth")?.parse().ok()?;
    let score = parse_score(text)?;
    let start = words.iter().position(|word| *word == "pv")?;
    let pv: Vec<String> = words[start + 1..].iter().map(|&mv| mv.to_owned()).collect();
    if pv.is_empty() || index == 0 {
        return None;
    }
    Some((index - 1, Line { score, depth, pv }))
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
    fn multipv_lines_are_parsed() {
        assert_eq!(
            parse_line("info depth 18 seldepth 25 multipv 2 score cp -12 nodes 1 pv d2d4 g8f6"),
            Some((
                1,
                Line {
                    score: UciScore::Centipawns(-12),
                    depth: 18,
                    pv: vec!["d2d4".into(), "g8f6".into()],
                }
            ))
        );
        // Without `multipv`: the first line.
        assert_eq!(
            parse_line("info depth 3 score mate 2 pv h5f7").map(|(i, _)| i),
            Some(0)
        );
        assert_eq!(
            parse_line("info depth 18 multipv 1 score cp 30 lowerbound nodes 9 pv e2e4"),
            None
        );
        assert_eq!(
            parse_line("info depth 18 currmove e2e4 currmovenumber 1"),
            None
        );
        assert_eq!(parse_line("info string NNUE evaluation enabled"), None);
    }

    /// Needs a Stockfish on the `PATH` (in the Docker images); skipped otherwise.
    #[test]
    fn full_strength_multipv_search() {
        let binary = Path::new("stockfish");
        if probe(binary).is_none() {
            return;
        }
        let mut engine = Stockfish::spawn_full(binary, 1, 16).unwrap();
        assert!(engine.name().starts_with("Stockfish"));
        // Scholar's mate threat: Qxf7# is the only mate.
        let fen = "r1bqkbnr/pppp1ppp/2n5/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4";
        let moves = ["h5f7".to_owned()];
        assert!(
            engine
                .analyse_lines(Some(fen), &moves, Budget::Depth(5), 3)
                .unwrap()
                .is_empty()
        );
        let lines = engine
            .analyse_lines(Some(fen), &[], Budget::Depth(8), 3)
            .unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].pv[0], "h5f7");
        assert_eq!(lines[0].score, UciScore::Mate(1));
        assert!(!lines[1].score.eq(&UciScore::Mate(1)));
        // Back to one line.
        let single = engine.analyse(Some(fen), &[], Budget::Depth(4)).unwrap();
        assert_eq!(single.best_move, "h5f7");
    }

    #[test]
    fn missing_binary_is_reported() {
        let missing = Path::new("/no/such/stockfish");
        assert!(probe(missing).is_none());
        assert!(Stockfish::spawn(missing, 1).is_err());
    }
}
