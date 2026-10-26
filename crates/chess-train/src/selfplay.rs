//! Self-play data generation: the engine plays itself on every CPU core and
//! records quiet positions with their search score and the game result.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{Context, Result};
use chess_core::{Color, GameStatus, Position};
use chess_eval::{EngineEval, Evaluator};
use chess_search::{Score, SearchLimits, Searcher, StopFlag};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::dataset::{DatasetWriter, Outcome, Record};
use crate::rng::Rng;

/// Self-play parameters (`[selfplay]` section of the configuration).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct SelfplayConfig {
    /// Games to play; 0 = until stopped.
    pub(crate) games: u64,
    /// Nodes searched per move.
    pub(crate) nodes: u64,
    /// Random plies played first, for opening diversity (inclusive range).
    pub(crate) random_plies_min: u32,
    pub(crate) random_plies_max: u32,
    /// Games longer than this are adjudicated a draw.
    pub(crate) max_plies: u32,
    /// Worker threads; 0 = all cores.
    pub(crate) threads: usize,
    /// Transposition table per worker, in MiB.
    pub(crate) hash_mb: usize,
    /// Positions per dataset file.
    pub(crate) chunk_positions: u64,
    /// Base seed; game `i` uses `seed + i`.
    pub(crate) seed: u64,
    /// Network playing the games, reloaded whenever the file changes (the
    /// pipeline points it at the latest export). Absent or unreadable: the
    /// classical evaluation plays.
    pub(crate) network: Option<PathBuf>,
}

impl Default for SelfplayConfig {
    fn default() -> Self {
        Self {
            games: 0,
            nodes: 5_000,
            random_plies_min: 6,
            random_plies_max: 10,
            max_plies: 400,
            threads: 0,
            hash_mb: 4,
            chunk_positions: 1_000_000,
            seed: 0x5EED,
            network: None,
        }
    }
}

/// Live counters, shared with the metrics writer.
#[derive(Debug, Default)]
pub(crate) struct SelfplayStats {
    pub(crate) games: AtomicU64,
    pub(crate) positions: AtomicU64,
}

/// A score this large (centipawns, side to move) for long enough ends the game.
const RESIGN_SCORE: i32 = 1_000;
const RESIGN_PLIES: u32 = 4;
/// Near-zero scores for long enough after `DRAW_MIN_PLY` end the game as a draw.
const DRAW_SCORE: i32 = 10;
const DRAW_PLIES: u32 = 8;
const DRAW_MIN_PLY: usize = 80;
/// Positions with a larger score are not recorded (the game is decided anyway).
const MAX_RECORDED_SCORE: i32 = 3_000;

/// Plays one game from a random opening. Returns the recorded positions.
pub(crate) fn play_game<E: Evaluator>(
    searcher: &mut Searcher<E>,
    config: &SelfplayConfig,
    seed: u64,
) -> Vec<Record> {
    let mut rng = Rng::new(seed);
    let mut pos = Position::startpos();
    for _ in 0..rng.between(config.random_plies_min, config.random_plies_max) {
        let moves = pos.legal_moves();
        if moves.is_empty() {
            return Vec::new();
        }
        pos.make_move(moves[rng.below(moves.len())]);
    }

    searcher.clear();
    let limits = SearchLimits::nodes(config.nodes.max(1));
    let mut pending: Vec<Record> = Vec::new();
    let mut resign_streak = 0;
    let mut draw_streak = 0;

    let outcome = loop {
        match pos.status() {
            GameStatus::Ongoing => {}
            GameStatus::Checkmate {
                winner: Color::White,
            } => break Outcome::WhiteWins,
            GameStatus::Checkmate {
                winner: Color::Black,
            } => break Outcome::BlackWins,
            GameStatus::Stalemate | GameStatus::Draw(_) => break Outcome::Draw,
        }
        if pos.ply() >= config.max_plies as usize {
            break Outcome::Draw;
        }

        let result = searcher.search(&pos, &limits, |_| {});
        let Some(best) = result.best_move else {
            break Outcome::Draw;
        };
        let stm_score = match result.score {
            Score::Centipawns(cp) => cp,
            Score::Mate(moves) if moves > 0 => 30_000,
            Score::Mate(_) => -30_000,
        };

        // Quiet positions only: a pending capture makes the static evaluation
        // meaningless, which would teach the network noise.
        let quiet = !pos.in_check() && !best.is_capture() && best.promotion().is_none();
        if quiet && stm_score.abs() < MAX_RECORDED_SCORE {
            let white_score = (stm_score * pos.side_to_move().sign()) as i16;
            pending.push(Record::new(&pos, white_score, Outcome::Draw));
        }

        // Adjudication: both sides agree for a while that the game is decided.
        resign_streak = if stm_score.abs() >= RESIGN_SCORE {
            resign_streak + 1
        } else {
            0
        };
        draw_streak = if stm_score.abs() <= DRAW_SCORE {
            draw_streak + 1
        } else {
            0
        };
        if resign_streak >= RESIGN_PLIES {
            let white_winning = stm_score * pos.side_to_move().sign() > 0;
            break if white_winning {
                Outcome::WhiteWins
            } else {
                Outcome::BlackWins
            };
        }
        if draw_streak >= DRAW_PLIES && pos.ply() >= DRAW_MIN_PLY {
            break Outcome::Draw;
        }

        pos.make_move(best);
    };

    pending
        .into_iter()
        .map(|mut record| {
            record.outcome = outcome;
            record
        })
        .collect()
}

/// Plays games on all cores and writes them to `<out_dir>/<prefix>-NNNN.bin`
/// files of `chunk_positions` records. Returns when `config.games` are
/// played or `stop` is raised; `on_file` is called for each published file.
pub(crate) fn run(
    config: &SelfplayConfig,
    out_dir: &Path,
    prefix: &str,
    stop: &StopFlag,
    stats: &SelfplayStats,
    on_file: impl Fn(&Path) + Send,
) -> Result<()> {
    let threads = if config.threads == 0 {
        rayon::current_num_threads()
    } else {
        config.threads
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("selfplay-{i}"))
        .build()
        .context("cannot start the self-play threads")?;
    tracing::info!(threads, nodes = config.nodes, "self-play started");

    let (tx, rx) = mpsc::sync_channel::<Vec<Record>>(threads * 4);
    let first_chunk = next_chunk_index(out_dir, prefix);
    let writer = std::thread::scope(|scope| -> Result<()> {
        let writer = scope
            .spawn(move || write_chunks(rx, out_dir, prefix, first_chunk, config, stats, &on_file));
        let games = if config.games == 0 {
            u64::MAX
        } else {
            config.games
        };
        let start = Instant::now();
        let hash_mb = config.hash_mb;
        // Ending early is signalled with Err(()), which makes rayon stop
        // scheduling new games.
        let _ = pool.install(|| {
            (0..games).into_par_iter().try_for_each_init(
                || {
                    let mut searcher = Searcher::new(EngineEval::default(), StopFlag::new());
                    searcher.set_hash_size(hash_mb);
                    (searcher, None)
                },
                |(searcher, loaded), index| {
                    if stop.is_stopped() {
                        return Err(());
                    }
                    if let Some(network) = &config.network {
                        refresh_network(searcher, loaded, network);
                    }
                    let records = play_game(searcher, config, config.seed.wrapping_add(index));
                    let played = stats.games.fetch_add(1, Ordering::Relaxed) + 1;
                    if played.is_multiple_of(100) {
                        let rate = played as f64 / start.elapsed().as_secs_f64().max(1e-3);
                        tracing::info!(
                            games = played,
                            games_per_sec = format!("{rate:.1}"),
                            "self-play"
                        );
                    }
                    tx.send(records).map_err(|_| ())
                },
            )
        });
        drop(tx);
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("the dataset writer panicked"))?
    });
    writer?;
    tracing::info!(
        games = stats.games.load(Ordering::Relaxed),
        "self-play finished"
    );
    Ok(())
}

/// Loads `path` into `searcher` if the file changed since `loaded` (its
/// modification time). A missing or invalid file keeps the current evaluator.
fn refresh_network(
    searcher: &mut Searcher<EngineEval>,
    loaded: &mut Option<std::time::SystemTime>,
    path: &Path,
) {
    let Ok(modified) = std::fs::metadata(path).and_then(|meta| meta.modified()) else {
        return;
    };
    if *loaded == Some(modified) {
        return;
    }
    match std::fs::read(path).map(|bytes| EngineEval::from_nnue_bytes(&bytes)) {
        Ok(Ok(eval)) => {
            searcher.set_evaluator(eval);
            *loaded = Some(modified);
            tracing::debug!(file = %path.display(), "self-play network reloaded");
        }
        // Possibly a file being replaced: try again next game.
        Ok(Err(err)) => tracing::debug!(%err, "self-play network not loadable yet"),
        Err(err) => tracing::debug!(%err, "self-play network not readable yet"),
    }
}

/// First unused chunk number for `prefix` in `dir`, so that runs append.
fn next_chunk_index(dir: &Path, prefix: &str) -> u32 {
    crate::dataset::dataset_files(dir, Some(prefix))
        .iter()
        .filter_map(|path| {
            path.file_stem()?
                .to_str()?
                .strip_prefix(prefix)?
                .strip_prefix('-')?
                .parse::<u32>()
                .ok()
        })
        .max()
        .map_or(0, |last| last + 1)
}

fn chunk_path(dir: &Path, prefix: &str, index: u32) -> PathBuf {
    dir.join(format!("{prefix}-{index:04}.{}", crate::dataset::EXTENSION))
}

fn write_chunks(
    rx: mpsc::Receiver<Vec<Record>>,
    dir: &Path,
    prefix: &str,
    mut index: u32,
    config: &SelfplayConfig,
    stats: &SelfplayStats,
    on_file: &(impl Fn(&Path) + Send),
) -> Result<()> {
    let mut writer: Option<DatasetWriter> = None;
    for records in rx {
        for record in &records {
            let current = match &mut writer {
                Some(current) => current,
                None => writer.insert(DatasetWriter::create(&chunk_path(dir, prefix, index))?),
            };
            current.write(record)?;
            stats.positions.fetch_add(1, Ordering::Relaxed);
            if current.count() >= config.chunk_positions.max(1)
                && let Some(full) = writer.take()
            {
                let (path, count) = full.finish()?;
                tracing::info!(file = %path.display(), positions = count, "dataset chunk written");
                on_file(&path);
                index += 1;
            }
        }
    }
    if let Some(last) = writer.take() {
        let (path, count) = last.finish()?;
        tracing::info!(file = %path.display(), positions = count, "dataset chunk written");
        on_file(&path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{DatasetReader, dataset_files, record_count};
    use crate::test_dir;

    fn small() -> SelfplayConfig {
        SelfplayConfig {
            nodes: 300,
            max_plies: 120,
            threads: 2,
            ..SelfplayConfig::default()
        }
    }

    #[test]
    fn games_produce_consistent_records() {
        let mut searcher = Searcher::new(chess_eval::ClassicEval, StopFlag::new());
        let records = play_game(&mut searcher, &small(), 1);
        assert!(!records.is_empty());
        let outcome = records[0].outcome;
        assert!(records.iter().all(|r| r.outcome == outcome));
        assert!(
            records
                .iter()
                .all(|r| i32::from(r.score).abs() < MAX_RECORDED_SCORE)
        );
        // Deterministic for a given seed.
        assert_eq!(play_game(&mut searcher, &small(), 1), records);
    }

    #[test]
    fn run_writes_chunks_and_counts() {
        let dir = test_dir("selfplay-run");
        let config = SelfplayConfig {
            games: 6,
            chunk_positions: 100,
            ..small()
        };
        let stats = SelfplayStats::default();
        let published = std::sync::Mutex::new(Vec::new());
        run(&config, &dir, "test", &StopFlag::new(), &stats, |path| {
            published.lock().unwrap().push(path.to_owned());
        })
        .unwrap();
        let files = dataset_files(&dir, Some("test"));
        assert_eq!(files, *published.lock().unwrap());
        assert_eq!(stats.games.load(Ordering::Relaxed), 6);
        let total: u64 = files.iter().map(|f| record_count(f)).sum();
        assert_eq!(total, stats.positions.load(Ordering::Relaxed));
        assert!(files.iter().rev().skip(1).all(|f| record_count(f) == 100));
        assert!(
            !DatasetReader::open(&files[0])
                .unwrap()
                .read_chunk(10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(next_chunk_index(&dir, "test"), files.len() as u32);
    }

    #[test]
    fn stops_on_request() {
        let dir = test_dir("selfplay-stop");
        let stop = StopFlag::new();
        stop.stop();
        let stats = SelfplayStats::default();
        run(&small(), &dir, "stopped", &stop, &stats, |_| {}).unwrap();
        assert_eq!(stats.games.load(Ordering::Relaxed), 0);
    }
}
