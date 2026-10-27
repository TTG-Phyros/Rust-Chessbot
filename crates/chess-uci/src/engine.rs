//! UCI state machine. Searches run on a worker thread so that `stop` and
//! `isready` are answered while the engine thinks.

use std::io::Write;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chess_core::{Position, perft_divide};
use chess_eval::EngineEval;
use chess_search::{
    DEFAULT_HASH_MB, MAX_HASH_MB, SearchInfo, SearchLimits, SearchResult, Searcher, StopFlag, bench,
};

use crate::command::{Go, parse_go, parse_position, parse_setoption};

const ENGINE_NAME: &str = concat!("chess-bot ", env!("CARGO_PKG_VERSION"));
const DEFAULT_MOVE_OVERHEAD_MS: u64 = 30;
const MAX_MOVE_OVERHEAD_MS: u64 = 5_000;
/// Most lines the `MultiPV` option accepts.
const MAX_MULTIPV: u64 = 64;
/// Depth of the `bench` command when none is given.
pub(crate) const DEFAULT_BENCH_DEPTH: u32 = 10;

/// Where protocol lines go (stdout in production, a buffer in tests);
/// shared with the search thread.
pub(crate) type Output = Arc<Mutex<dyn Write + Send>>;

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic in the search thread must not take the protocol loop down.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Writes one protocol line and flushes it.
pub(crate) fn send(out: &Output, line: &str) {
    let mut out = lock(out);
    if let Err(err) = writeln!(out, "{line}").and_then(|()| out.flush()) {
        tracing::error!(%err, "cannot write to the GUI");
    }
}

pub(crate) struct Engine {
    out: Output,
    position: Position,
    searcher: Arc<Mutex<Searcher<EngineEval>>>,
    /// Value of the `EvalFile` option.
    eval_file: String,
    stop: StopFlag,
    worker: Option<JoinHandle<()>>,
    /// Whether the search in progress is `go infinite` (it never ends by itself).
    infinite: bool,
    move_overhead: Duration,
    /// Value of the `MultiPV` option.
    multipv: usize,
}

impl Engine {
    pub(crate) fn new(out: Output) -> Self {
        let stop = StopFlag::new();
        Self {
            out,
            position: Position::startpos(),
            searcher: Arc::new(Mutex::new(Searcher::new(
                EngineEval::default(),
                stop.clone(),
            ))),
            eval_file: String::new(),
            stop,
            worker: None,
            infinite: false,
            move_overhead: Duration::from_millis(DEFAULT_MOVE_OVERHEAD_MS),
            multipv: 1,
        }
    }

    fn send(&self, line: &str) {
        send(&self.out, line);
    }

    /// Handles one command line. Returns `false` when the engine must exit.
    pub(crate) fn handle(&mut self, line: &str) -> bool {
        let line = line.trim();
        let (command, args) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let outcome = match command {
            "uci" => {
                self.identify();
                Ok(())
            }
            "isready" => {
                self.send("readyok");
                Ok(())
            }
            "ucinewgame" => {
                self.finish_search();
                lock(&self.searcher).clear();
                self.position = Position::startpos();
                Ok(())
            }
            "setoption" => self.set_option(args),
            "position" => parse_position(args).map(|position| self.position = position),
            "go" => self.go(args),
            "stop" => {
                self.finish_search();
                Ok(())
            }
            "d" => {
                self.send(&self.position.to_string());
                self.send(&format!("Evaluation: {}", self.evaluation()));
                Ok(())
            }
            "bench" => self.bench(args),
            "quit" => {
                self.finish_search();
                return false;
            }
            "" => Ok(()),
            other => {
                tracing::warn!(command = other, "unknown command");
                Ok(())
            }
        };
        // UCI has no error reply: report the problem as information and
        // keep the previous state.
        if let Err(err) = outcome {
            tracing::warn!(%line, error = %format!("{err:#}"), "invalid command");
            self.send(&format!("info string error: {err:#}"));
        }
        true
    }

    fn identify(&self) {
        self.send(&format!("id name {ENGINE_NAME}"));
        self.send("id author chess-bot contributors");
        self.send(&format!(
            "option name Hash type spin default {DEFAULT_HASH_MB} min 1 max {MAX_HASH_MB}"
        ));
        self.send(&format!(
            "option name Move Overhead type spin default {DEFAULT_MOVE_OVERHEAD_MS} min 0 max {MAX_MOVE_OVERHEAD_MS}"
        ));
        self.send(&format!(
            "option name MultiPV type spin default 1 min 1 max {MAX_MULTIPV}"
        ));
        self.send("option name Clear Hash type button");
        self.send("option name EvalFile type string default <empty>");
        self.send("uciok");
    }

    fn set_option(&mut self, args: &str) -> Result<()> {
        let (name, value) = parse_setoption(args)?;
        self.finish_search();
        let number = |value: Option<String>| -> Result<u64> {
            value
                .context("missing value")?
                .parse()
                .context("expected a number")
        };
        match name.to_ascii_lowercase().as_str() {
            "hash" => {
                let mb = number(value)?.clamp(1, MAX_HASH_MB as u64) as usize;
                lock(&self.searcher).set_hash_size(mb);
            }
            "move overhead" => {
                self.move_overhead =
                    Duration::from_millis(number(value)?.min(MAX_MOVE_OVERHEAD_MS));
            }
            "multipv" => self.multipv = number(value)?.clamp(1, MAX_MULTIPV) as usize,
            "clear hash" => lock(&self.searcher).clear(),
            "evalfile" => {
                let loaded = self.load_eval_file(value.as_deref().unwrap_or_default());
                self.send(&format!("info string evaluation: {}", self.evaluation()));
                loaded?;
            }
            _ => bail!("unknown option {name:?}"),
        }
        Ok(())
    }

    /// Loads a network for the `EvalFile` option (also set from the
    /// `CHESS_EVAL_FILE` variable at startup). An empty value (or `<empty>`)
    /// selects the classical evaluation; an unreadable or invalid file falls
    /// back to it too, and the error is returned.
    pub(crate) fn load_eval_file(&mut self, path: &str) -> Result<()> {
        let path = path.trim();
        self.finish_search();
        let loaded = if path.is_empty() || path == "<empty>" {
            Ok(None)
        } else {
            std::fs::read(path)
                .with_context(|| format!("cannot read {path}"))
                .and_then(|bytes| EngineEval::from_nnue_bytes(&bytes).context("invalid network"))
                .map(Some)
        };
        let (eval, file, result) = match loaded {
            Ok(Some(eval)) => (eval, path.to_owned(), Ok(())),
            Ok(None) => (EngineEval::default(), String::new(), Ok(())),
            Err(err) => (EngineEval::default(), String::new(), Err(err)),
        };
        lock(&self.searcher).set_evaluator(eval);
        self.eval_file = file;
        result
    }

    /// `nnue (<file>)` or `classic`.
    fn evaluation(&self) -> String {
        if self.eval_file.is_empty() {
            "classic".into()
        } else {
            format!("nnue ({})", self.eval_file)
        }
    }

    fn go(&mut self, args: &str) -> Result<()> {
        let limits = match parse_go(args, self.move_overhead)? {
            Go::Perft(depth) => {
                self.finish_search();
                self.perft(depth);
                return Ok(());
            }
            Go::Search(limits) => SearchLimits {
                multipv: self.multipv,
                ..limits
            },
        };
        self.finish_search();
        self.stop.reset();

        let searcher = Arc::clone(&self.searcher);
        let out = Arc::clone(&self.out);
        let stop = self.stop.clone();
        let position = self.position.clone();
        self.infinite = limits.infinite;
        let worker = std::thread::Builder::new()
            .name("search".into())
            // Deep recursion: give the search thread room.
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                let result = lock(&searcher).search(&position, &limits, |info| {
                    for line in format_info(info) {
                        send(&out, &line);
                    }
                });
                // An infinite search must not answer before being told to stop.
                if limits.infinite {
                    while !stop.is_stopped() {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                send(&out, &format_bestmove(&result));
            })
            .context("cannot start the search thread")?;
        self.worker = Some(worker);
        Ok(())
    }

    /// End of input: lets a finite search complete (so that
    /// `echo "go depth 10" | chess-uci` prints its result), stops an infinite one.
    pub(crate) fn finish_input(&mut self) {
        if self.infinite {
            self.finish_search();
        } else if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::error!("the search thread panicked");
        }
    }

    /// Stops the search in progress, if any, and waits for its `bestmove`.
    pub(crate) fn finish_search(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.stop.stop();
            if worker.join().is_err() {
                tracing::error!("the search thread panicked");
            }
        }
    }

    /// Waits for the search in progress to end by itself.
    #[cfg(test)]
    fn wait_for_search(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    /// Prints the node count of each root move, then the total (Stockfish format).
    fn perft(&mut self, depth: u32) {
        let start = Instant::now();
        let divide = perft_divide(&mut self.position, depth);
        let total: u64 = divide.iter().map(|(_, nodes)| nodes).sum();
        for (mv, nodes) in &divide {
            self.send(&format!("{mv}: {nodes}"));
        }
        self.send(&format!("\nNodes searched: {total}"));
        tracing::info!(depth, total, ms = start.elapsed().as_millis(), "perft");
    }

    fn bench(&mut self, args: &str) -> Result<()> {
        let depth = match args.trim() {
            "" => DEFAULT_BENCH_DEPTH,
            text => text.parse().context("bench depth")?,
        };
        self.finish_search();
        let mut searcher = lock(&self.searcher);
        let result = bench(&mut searcher, depth);
        searcher.clear();
        self.send(&format!("Nodes searched: {}", result.nodes));
        self.send(&format!("Nodes/second: {}", result.nps()));
        self.send(&format!("Time: {} ms", result.elapsed.as_millis()));
        Ok(())
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.finish_search();
    }
}

/// The `info` lines of an iteration: one, or one per line with `multipv k`
/// when several lines were asked for.
pub(crate) fn format_info(info: &SearchInfo) -> Vec<String> {
    let line = |multipv: Option<usize>, score: chess_search::Score, pv: &[chess_core::Move]| {
        let pv: Vec<String> = pv.iter().map(ToString::to_string).collect();
        format!(
            "info depth {} seldepth {}{} score {} nodes {} nps {} hashfull {} time {} pv {}",
            info.depth,
            info.seldepth,
            multipv.map_or_else(String::new, |k| format!(" multipv {k}")),
            score,
            info.nodes,
            info.nps,
            info.hashfull,
            info.time.as_millis(),
            pv.join(" ")
        )
    };
    if info.lines.len() <= 1 {
        return vec![line(None, info.score, &info.pv)];
    }
    info.lines
        .iter()
        .enumerate()
        .map(|(k, pv_line)| line(Some(k + 1), pv_line.score, &pv_line.pv))
        .collect()
}

fn format_bestmove(result: &SearchResult) -> String {
    let best = result.best_move.unwrap_or_default();
    match result.ponder {
        Some(ponder) => format!("bestmove {best} ponder {ponder}"),
        None => format!("bestmove {best}"),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    /// A `Write` whose content the test can read back.
    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl Write for Buffer {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            lock(&self.0).extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Buffer {
        fn lines(&self) -> Vec<String> {
            String::from_utf8_lossy(&lock(&self.0))
                .lines()
                .map(str::to_owned)
                .collect()
        }
    }

    fn engine() -> (Engine, Buffer) {
        let buffer = Buffer::default();
        let out: Output = Arc::new(Mutex::new(buffer.clone()));
        (Engine::new(out), buffer)
    }

    #[test]
    fn uci_lists_options() {
        let (mut engine, out) = engine();
        engine.handle("uci");
        let lines = out.lines();
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("option name Hash type spin"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l == "option name Clear Hash type button")
        );
        assert_eq!(lines.last().map(String::as_str), Some("uciok"));
    }

    #[test]
    fn multipv_reports_each_line() {
        let (mut engine, out) = engine();
        engine.handle("setoption name MultiPV value 3");
        engine.handle("position startpos");
        engine.handle("go depth 4");
        engine.wait_for_search();
        let lines = out.lines();
        for k in 1..=3 {
            assert!(
                lines.iter().any(|l| l.starts_with("info depth 4 seldepth")
                    && l.contains(&format!(" multipv {k} "))),
                "{lines:?}"
            );
        }
    }

    #[test]
    fn search_answers_with_info_and_bestmove() {
        let (mut engine, out) = engine();
        engine.handle("position startpos moves e2e4");
        engine.handle("go depth 5");
        engine.wait_for_search();
        let lines = out.lines();
        assert!(lines.iter().any(|l| l.starts_with("info depth 5 seldepth")));
        let best = lines
            .iter()
            .find_map(|l| l.strip_prefix("bestmove "))
            .unwrap();
        assert!(
            best.split_whitespace()
                .next()
                .is_some_and(|mv| mv.len() == 4)
        );
    }

    #[test]
    fn options_are_applied_and_validated() {
        let (mut engine, out) = engine();
        engine.handle("setoption name Hash value 8");
        engine.handle("setoption name Move Overhead value 100");
        assert_eq!(engine.move_overhead, Duration::from_millis(100));
        engine.handle("setoption name Clear Hash");
        engine.handle("setoption name Nope value 1");
        engine.handle("setoption name Hash value lots");
        let errors = out
            .lines()
            .iter()
            .filter(|l| l.starts_with("info string error"))
            .count();
        assert_eq!(errors, 2);
    }

    #[test]
    fn info_format() {
        let info = SearchInfo {
            depth: 3,
            seldepth: 5,
            score: chess_search::Score::Mate(2),
            nodes: 1234,
            time: Duration::from_millis(12),
            nps: 102_833,
            hashfull: 1,
            pv: Vec::new(),
            lines: Vec::new(),
        };
        assert_eq!(
            format_info(&info)[0],
            "info depth 3 seldepth 5 score mate 2 nodes 1234 nps 102833 hashfull 1 time 12 pv "
        );
    }
}
