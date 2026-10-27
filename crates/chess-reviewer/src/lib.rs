//! Reviews of Chess.com games: the local cache of games and of their
//! analyses, the sync of a player's archives into it, and the jobs that
//! analyse their games on a pool of engines.
//!
//! Past months of an archive never change: once read after the month
//! ended, they are never downloaded again. The current month and the list
//! of months are revalidated (conditional requests) after a short TTL. An
//! analysis is stored per game and engine configuration, and never redone.

pub mod config;
mod error;
pub mod jobs;
pub mod profile;
pub mod store;
pub mod sync;

pub use config::{EngineKind, Preset, ReviewConfig};
pub use error::Error;
pub use jobs::{Estimate, Job, JobProgress, JobRequest, JobState, ReviewService};
pub use store::{Store, analysis_key};
pub use sync::{MonthSource, SyncEvent, sync_player};

/// Current Unix time, in seconds.
#[must_use]
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}
