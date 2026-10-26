//! Training metrics for `chess-server` and the UI.
//!
//! Two files in `<data_dir>/metrics/`:
//! - `events.jsonl`: one JSON object per line (`selfplay`, `superbatch`,
//!   `checkpoint`, ... events), appended;
//! - `status.json`: the current state of the run, rewritten atomically.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Seconds since the Unix epoch.
pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// State of the current run (`status.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Status {
    pub(crate) run: String,
    /// `selfplay`, `training`, `pipeline` or `finished`.
    pub(crate) phase: String,
    pub(crate) device: String,
    pub(crate) started_at: u64,
    pub(crate) updated_at: u64,
    pub(crate) games: u64,
    pub(crate) positions: u64,
    pub(crate) superbatch: u64,
    pub(crate) loss: Option<f64>,
    pub(crate) learning_rate: Option<f64>,
    pub(crate) positions_per_second: Option<f64>,
    pub(crate) vram_used_mb: Option<u64>,
    pub(crate) latest_weights: Option<String>,
    /// Elo of the latest measured network against the classical evaluation.
    pub(crate) elo: Option<f64>,
    pub(crate) elo_error: Option<f64>,
    pub(crate) elo_superbatch: Option<u64>,
}

pub(crate) struct Metrics {
    dir: PathBuf,
    events: Mutex<File>,
    status: Mutex<Status>,
}

impl Metrics {
    pub(crate) fn open(data_dir: &Path, status: Status) -> Result<Self> {
        let dir = data_dir.join("metrics");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create {}", dir.display()))?;
        let path = dir.join("events.jsonl");
        let events = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        let metrics = Self {
            dir,
            events: Mutex::new(events),
            status: Mutex::new(status),
        };
        metrics.update(|_| {});
        Ok(metrics)
    }

    /// Appends an event; `kind` and a timestamp are added to `fields`.
    pub(crate) fn event(&self, kind: &str, fields: Value) {
        let mut line = json!({ "time": now(), "kind": kind });
        if let (Some(line), Value::Object(fields)) = (line.as_object_mut(), fields) {
            line.extend(fields);
        }
        let mut file = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Err(err) = writeln!(file, "{line}") {
            tracing::warn!(%err, "cannot write metrics event");
        }
    }

    /// Changes the status and rewrites `status.json`.
    pub(crate) fn update(&self, change: impl FnOnce(&mut Status)) {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        change(&mut status);
        status.updated_at = now();
        if let Err(err) = self.write_status(&status) {
            tracing::warn!(error = %err, "cannot write status.json");
        }
    }

    #[cfg(test)]
    pub(crate) fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn write_status(&self, status: &Status) -> Result<()> {
        let temp = self.dir.join("status.json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(status)?)?;
        std::fs::rename(&temp, self.dir.join("status.json"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir;

    #[test]
    fn events_and_status_are_written() {
        let dir = test_dir("metrics");
        let metrics = Metrics::open(
            &dir,
            Status {
                run: "r".into(),
                ..Status::default()
            },
        )
        .unwrap();
        metrics.event("superbatch", json!({ "loss": 0.25 }));
        metrics.event("checkpoint", json!({ "superbatch": 3 }));
        metrics.update(|status| status.superbatch = 3);

        let events = std::fs::read_to_string(dir.join("metrics/events.jsonl")).unwrap();
        let lines: Vec<Value> = events
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["kind"], "superbatch");
        assert_eq!(lines[0]["loss"], 0.25);

        let status: Status =
            serde_json::from_slice(&std::fs::read(dir.join("metrics/status.json")).unwrap())
                .unwrap();
        assert_eq!((status.run.as_str(), status.superbatch), ("r", 3));
        assert_eq!(metrics.status().superbatch, 3);
    }
}
