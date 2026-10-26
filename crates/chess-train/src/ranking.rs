//! The best snapshots of each network: every measured export is ranked by
//! its Elo against the classical evaluation in `weights/<run>/best.json`,
//! and the measured snapshots that fall out of the top are deleted.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::trainer::write_atomically;

const RANKING_FILE: &str = "best.json";

/// A measured snapshot. Written in camelCase: `chess-server` reads the
/// ranking as it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Ranked {
    pub(crate) superbatch: u64,
    /// Elo against the classical evaluation.
    pub(crate) elo: f64,
    /// Half-width of the 95 % interval.
    pub(crate) elo_error: f64,
    pub(crate) games: u32,
    /// Unix seconds.
    pub(crate) measured_at: u64,
}

/// File name of the snapshot exported after `superbatch`.
pub(crate) fn snapshot_name(superbatch: u64) -> String {
    format!("sb{superbatch:05}.nnue")
}

fn snapshot_superbatch(file_name: &str) -> Option<u64> {
    let digits = file_name.strip_prefix("sb")?.strip_suffix(".nnue")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The ranking of the snapshots in `dir`, best first; `None` if there is none yet.
pub(crate) fn load(dir: &Path) -> Option<Vec<Ranked>> {
    let bytes = std::fs::read(dir.join(RANKING_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Ranks a measured snapshot among the best `keep`, then deletes the
/// snapshots up to `entry.superbatch` that are not in the ranking (later
/// ones are still waiting for their measurement).
pub(crate) fn record(dir: &Path, entry: Ranked, keep: usize) -> Result<Vec<Ranked>> {
    let measured = entry.superbatch;
    let mut ranking = load(dir).unwrap_or_default();
    ranking.retain(|ranked| ranked.superbatch != measured);
    ranking.push(entry);
    save(dir, ranking, keep, measured)
}

fn save(
    dir: &Path,
    mut ranking: Vec<Ranked>,
    keep: usize,
    prune_up_to: u64,
) -> Result<Vec<Ranked>> {
    ranking.sort_by(|a, b| {
        b.elo
            .total_cmp(&a.elo)
            .then(b.superbatch.cmp(&a.superbatch))
    });
    ranking.truncate(keep.max(1));
    write_atomically(
        &dir.join(RANKING_FILE),
        &serde_json::to_vec_pretty(&ranking)?,
    )?;
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let Some(superbatch) = name.to_str().and_then(snapshot_superbatch) else {
            continue;
        };
        if superbatch <= prune_up_to && ranking.iter().all(|r| r.superbatch != superbatch) {
            std::fs::remove_file(entry.path())
                .with_context(|| format!("cannot delete {}", entry.path().display()))?;
        }
    }
    Ok(ranking)
}

/// Forgets the ranking and deletes the snapshots of `dir` (a network
/// restarted from scratch numbers its superbatches from 1 again).
pub(crate) fn reset(dir: &Path) -> Result<()> {
    match std::fs::remove_file(dir.join(RANKING_FILE)) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err.into()),
        _ => {}
    }
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if entry
            .file_name()
            .to_str()
            .and_then(snapshot_superbatch)
            .is_some()
        {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// One `match` line of `events.jsonl`.
#[derive(Deserialize)]
struct MatchEvent {
    kind: String,
    run: Option<String>,
    superbatch: Option<u64>,
    elo: Option<f64>,
    elo_error: Option<f64>,
    games: Option<u32>,
    #[serde(default)]
    time: u64,
}

/// Builds the ranking of a network trained before rankings existed, from the
/// matches recorded in `events` whose snapshot is still there. Does nothing
/// when the network already has a ranking or was never measured.
pub(crate) fn seed_from_events(dir: &Path, events: &Path, run: &str, keep: usize) -> Result<()> {
    if dir.join(RANKING_FILE).exists() {
        return Ok(());
    }
    let Ok(text) = std::fs::read_to_string(events) else {
        return Ok(());
    };
    let mut ranking: Vec<Ranked> = Vec::new();
    for event in text
        .lines()
        .filter_map(|line| serde_json::from_str::<MatchEvent>(line).ok())
        .filter(|event| event.kind == "match" && event.run.as_deref() == Some(run))
    {
        let (Some(superbatch), Some(elo)) = (event.superbatch, event.elo) else {
            continue;
        };
        if !dir.join(snapshot_name(superbatch)).exists() {
            continue;
        }
        // The latest measurement of a superbatch wins (a restarted run reuses numbers).
        ranking.retain(|ranked| ranked.superbatch != superbatch);
        ranking.push(Ranked {
            superbatch,
            elo,
            elo_error: event.elo_error.unwrap_or_default(),
            games: event.games.unwrap_or_default(),
            measured_at: event.time,
        });
    }
    let Some(last) = ranking.iter().map(|ranked| ranked.superbatch).max() else {
        return Ok(());
    };
    let ranking = save(dir, ranking, keep, last)?;
    tracing::info!(
        network = run,
        kept = ranking.len(),
        "ranking of the snapshots rebuilt from the match history"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir;

    fn ranked(superbatch: u64, elo: f64) -> Ranked {
        Ranked {
            superbatch,
            elo,
            elo_error: 50.0,
            games: 64,
            measured_at: 0,
        }
    }

    fn snapshots(dir: &Path) -> Vec<u64> {
        let mut found: Vec<u64> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter_map(|e| snapshot_superbatch(e.file_name().to_str()?))
            .collect();
        found.sort_unstable();
        found
    }

    #[test]
    fn keeps_the_best_measured_snapshots() {
        let dir = test_dir("ranking");
        for superbatch in (5..=40).step_by(5) {
            std::fs::write(dir.join(snapshot_name(superbatch)), b"net").unwrap();
        }
        std::fs::write(dir.join("notes.nnue"), b"kept").unwrap();
        let elos = [(5, -50.0), (10, 20.0), (15, 10.0), (20, 80.0), (25, -5.0)];
        for (superbatch, elo) in elos {
            record(&dir, ranked(superbatch, elo), 3).unwrap();
        }
        let ranking = load(&dir).unwrap();
        assert_eq!(
            ranking.iter().map(|r| r.superbatch).collect::<Vec<_>>(),
            [20, 10, 15]
        );
        // Measured and out of the top: deleted; 30..40 still wait for a match.
        assert_eq!(snapshots(&dir), [10, 15, 20, 30, 35, 40]);
        assert!(dir.join("notes.nnue").exists());

        // A better one pushes the last out.
        record(&dir, ranked(30, 50.0), 3).unwrap();
        assert_eq!(snapshots(&dir), [10, 20, 30, 35, 40]);

        reset(&dir).unwrap();
        assert!(load(&dir).is_none());
        assert!(snapshots(&dir).is_empty());
        assert!(dir.join("notes.nnue").exists());
        assert_eq!(snapshot_superbatch("sb00012.nnue"), Some(12));
        assert_eq!(snapshot_superbatch("sb12.nnue.tmp"), None);
        assert_eq!(snapshot_superbatch("sbx.nnue"), None);
    }

    #[test]
    fn seeds_from_the_match_history() {
        let dir = test_dir("ranking-seed");
        let weights = dir.join("main");
        std::fs::create_dir_all(&weights).unwrap();
        for superbatch in [5, 10, 15, 20] {
            std::fs::write(weights.join(snapshot_name(superbatch)), b"net").unwrap();
        }
        let events = dir.join("events.jsonl");
        std::fs::write(
            &events,
            [
                r#"{"time":1,"kind":"match","run":"main","superbatch":5,"elo":10.0,"elo_error":80.0,"games":64}"#,
                r#"{"time":2,"kind":"match","run":"other","superbatch":10,"elo":900.0}"#,
                r#"{"time":3,"kind":"match","run":"main","superbatch":10,"elo":-20.0}"#,
                r#"{"time":4,"kind":"match","run":"main","superbatch":15,"elo":60.0}"#,
                r#"{"time":5,"kind":"match","run":"main","superbatch":99,"elo":500.0}"#,
                r#"{"time":6,"kind":"superbatch","run":"main","superbatch":16}"#,
            ]
            .join("\n"),
        )
        .unwrap();
        seed_from_events(&weights, &events, "main", 2).unwrap();
        let ranking = load(&weights).unwrap();
        // 99 has no snapshot any more; 20 was never measured and stays.
        assert_eq!(
            ranking.iter().map(|r| r.superbatch).collect::<Vec<_>>(),
            [15, 5]
        );
        assert_eq!(snapshots(&weights), [5, 15, 20]);
        // Only once: an existing ranking is never rebuilt.
        std::fs::write(&events, "").unwrap();
        seed_from_events(&weights, &events, "main", 2).unwrap();
        assert_eq!(load(&weights).unwrap().len(), 2);
    }
}
