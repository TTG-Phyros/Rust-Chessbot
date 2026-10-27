//! Reviews of Chess.com games: the local cache of games (and, later, of
//! their analyses) and the sync of a player's archives into it.
//!
//! Past months of an archive never change: once read after the month
//! ended, they are never downloaded again. The current month and the list
//! of months are revalidated (conditional requests) after a short TTL.

pub mod config;
mod error;
pub mod store;
pub mod sync;

pub use config::ReviewConfig;
pub use error::Error;
pub use store::Store;
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
