//! Responses to requests sent with an `Idempotency-Key`: a retried request
//! (same key, same content) gets the first response instead of running again.

use std::time::Duration;

use rusqlite::{OptionalExtension, params};

use super::now;
use crate::{Lab, LabError};

/// How long a key and its response are kept.
pub const IDEMPOTENCY_TTL: Duration = Duration::from_secs(24 * 3600);

/// What to do with a request carrying an idempotency key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Idempotency {
    /// First time: run it, then [`Lab::idempotency_finish`] (or
    /// [`Lab::idempotency_abandon`] if it failed in a way worth retrying).
    New,
    /// Already answered: send this response again.
    Replay {
        /// HTTP status.
        status: u16,
        /// Media type of the body.
        content_type: Option<String>,
        /// Body.
        body: Vec<u8>,
    },
    /// The first request with this key is still running.
    InProgress,
    /// The key was used for a different request.
    Mismatch,
}

impl Lab {
    /// Registers `key` for a request of `scope` (who sent it, method, path)
    /// whose content hashes to `fingerprint`.
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn idempotency_begin(
        &self,
        scope: &str,
        key: &str,
        fingerprint: &str,
    ) -> Result<Idempotency, LabError> {
        let connection = self.lock();
        let now = now();
        connection.execute(
            "DELETE FROM idempotency WHERE created_at < ?1",
            [now - IDEMPOTENCY_TTL.as_millis() as i64],
        )?;
        let inserted = connection.execute(
            "INSERT INTO idempotency (scope, key, fingerprint, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (scope, key) DO NOTHING",
            params![scope, key, fingerprint, now],
        )?;
        if inserted == 1 {
            return Ok(Idempotency::New);
        }
        let stored = connection
            .query_row(
                "SELECT fingerprint, status, content_type, body FROM idempotency
                 WHERE scope = ?1 AND key = ?2",
                params![scope, key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<u16>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<Vec<u8>>>(3)?,
                    ))
                },
            )
            .optional()?;
        Ok(match stored {
            // Purged between the two statements: as good as new next time.
            None => Idempotency::InProgress,
            Some((stored, _, _, _)) if stored != fingerprint => Idempotency::Mismatch,
            Some((_, None, _, _)) => Idempotency::InProgress,
            Some((_, Some(status), content_type, body)) => Idempotency::Replay {
                status,
                content_type,
                body: body.unwrap_or_default(),
            },
        })
    }

    /// Stores the response to replay for `key`.
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn idempotency_finish(
        &self,
        scope: &str,
        key: &str,
        status: u16,
        content_type: Option<&str>,
        body: &[u8],
    ) -> Result<(), LabError> {
        self.lock().execute(
            "UPDATE idempotency SET status = ?3, content_type = ?4, body = ?5
             WHERE scope = ?1 AND key = ?2",
            params![scope, key, status, content_type, body],
        )?;
        Ok(())
    }

    /// Forgets `key`: the request may be sent again with it.
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn idempotency_abandon(&self, scope: &str, key: &str) -> Result<(), LabError> {
        self.lock().execute(
            "DELETE FROM idempotency WHERE scope = ?1 AND key = ?2",
            params![scope, key],
        )?;
        Ok(())
    }
}
