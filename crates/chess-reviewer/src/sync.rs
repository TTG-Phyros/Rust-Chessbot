//! Brings the cache up to date with a player's Chess.com archives, reading
//! only what is needed for the requested games.

use chess_chesscom::{ApiGame, Client, Fetched, GameFilter, Month};

use crate::{Error, Store};

/// A month is final once read this long after it ended (seconds): games
/// are filed by their end time, and late filings are rare after a day.
const FINAL_AFTER: i64 = 86_400;

/// Where the games of a month came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonthSource {
    /// The cache, without any request.
    Cache,
    /// The cache, confirmed current by a conditional request.
    Revalidated,
    /// Downloaded.
    Downloaded,
    /// Listed by Chess.com but not served (`404`): skipped, not cached.
    Unavailable,
}

/// Progress of [`sync_player`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    /// The list of months is known.
    Months {
        /// Months in the period.
        total: usize,
        /// Whether the list came from the cache.
        cached: bool,
    },
    /// One more month is in the cache.
    Month {
        /// The month.
        month: Month,
        /// 1-based position among the months read.
        index: usize,
        /// Months in the period (reading may stop before the last).
        total: usize,
        /// Where its games came from.
        source: MonthSource,
        /// Games of the filter found so far.
        found: usize,
    },
}

/// Whether data read at `fetched_at` must be revalidated at `now`: never for
/// a month read after it was over, after `ttl` seconds otherwise.
#[must_use]
pub fn is_stale(month: Option<Month>, fetched_at: i64, now: i64, ttl: i64) -> bool {
    if month.is_some_and(|month| fetched_at >= month.end() + FINAL_AFTER) {
        return false;
    }
    now.saturating_sub(fetched_at) >= ttl
}

/// The games of `username` kept by `filter`, most recent first, downloading
/// the months missing from the cache (most recent first, stopping as soon
/// as there are enough games) and revalidating recent data older than
/// `ttl` seconds.
///
/// # Errors
///
/// Chess.com errors (unknown player, network…) and cache errors.
pub async fn sync_player(
    client: &Client,
    store: &Store,
    username: &str,
    filter: &GameFilter,
    ttl: i64,
    mut on_event: impl FnMut(SyncEvent),
) -> Result<Vec<ApiGame>, Error> {
    let (months, cached) = months(client, store, username, ttl).await?;
    let months = filter.months(&months);
    let total = months.len();
    on_event(SyncEvent::Months { total, cached });

    for (index, month) in months.into_iter().enumerate() {
        let source = sync_month(client, store, username, month, ttl).await?;
        let found = store.count_games(username, filter)?;
        on_event(SyncEvent::Month {
            month,
            index: index + 1,
            total,
            source,
            found,
        });
        // Older months only hold older games.
        if found >= filter.max_games {
            break;
        }
    }
    store.games(username, filter)
}

/// The months of `username`, and whether they came from the cache.
async fn months(
    client: &Client,
    store: &Store,
    username: &str,
    ttl: i64,
) -> Result<(Vec<Month>, bool), Error> {
    let cached = store.months(username)?;
    let now = crate::now();
    if let Some(cached) = &cached
        && !is_stale(None, cached.fetched_at, now, ttl)
    {
        return Ok((cached.months.clone(), true));
    }
    let validators = cached
        .as_ref()
        .map(|cached| cached.validators.clone())
        .unwrap_or_default();
    match (client.archives(username, &validators).await?, cached) {
        (Fetched::Fresh(months, validators), _) => {
            store.save_months(username, &months, &validators, now)?;
            Ok((months, false))
        }
        (Fetched::NotModified, Some(cached)) => {
            store.touch_months(username, now)?;
            Ok((cached.months, true))
        }
        // 304 without anything cached: cannot happen (no validators sent).
        (Fetched::NotModified, None) => Ok((Vec::new(), false)),
    }
}

async fn sync_month(
    client: &Client,
    store: &Store,
    username: &str,
    month: Month,
    ttl: i64,
) -> Result<MonthSource, Error> {
    let cached = store.month(username, month)?;
    let now = crate::now();
    if let Some(cached) = &cached
        && !is_stale(Some(month), cached.fetched_at, now, ttl)
    {
        return Ok(MonthSource::Cache);
    }
    let validators = cached.map(|cached| cached.validators).unwrap_or_default();
    let fetched = match client.month(username, month, &validators).await {
        Ok(fetched) => fetched,
        // Chess.com lists the month but cannot serve it (it happens to the
        // current month of very active players): skip it, retry next time.
        Err(chess_chesscom::Error::Status { status: 404, .. }) => {
            tracing::warn!(%username, %month, "Chess.com cannot serve this month, skipped");
            return Ok(MonthSource::Unavailable);
        }
        Err(err) => return Err(err.into()),
    };
    match fetched {
        Fetched::Fresh(games, validators) => {
            let stored = store.save_month(username, month, &games, &validators, now)?;
            tracing::debug!(%username, %month, games = games.len(), stored, "month downloaded");
            Ok(MonthSource::Downloaded)
        }
        Fetched::NotModified => {
            store.touch_month(username, month, now)?;
            Ok(MonthSource::Revalidated)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn past_months_never_go_stale() {
        let september = Month {
            year: 2026,
            month: 9,
        };
        let end = september.end();
        // Read during the month: revalidated after the TTL.
        assert!(!is_stale(Some(september), end - 100, end - 50, 600));
        assert!(is_stale(Some(september), end - 100, end + 600, 600));
        // Read a day after it ended: final.
        assert!(!is_stale(Some(september), end + FINAL_AFTER, i64::MAX, 600));
        // The list of months always goes stale.
        assert!(is_stale(None, 0, 600, 600));
        assert!(!is_stale(None, 0, 599, 600));
    }
}
