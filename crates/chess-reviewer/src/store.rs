//! SQLite cache: the months of each player's archive (with their cache
//! validators) and the standard games read from them.
//!
//! Operations are short (an index lookup, one transaction per month), so
//! async callers run them inline.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use chess_chesscom::{ApiGame, GameFilter, Month, Validators};
use chess_review::{GameAnalysis, SearchSettings};
use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

/// Schema, one entry per version (`PRAGMA user_version`).
const MIGRATIONS: &[&str] = &[
    // Usernames are stored lowercase (Chess.com usernames are case-insensitive).
    "CREATE TABLE players (
        username      TEXT PRIMARY KEY,
        months        TEXT NOT NULL,   -- JSON array of \"YYYY-MM\"
        etag          TEXT,
        last_modified TEXT,
        fetched_at    INTEGER NOT NULL
    );
    CREATE TABLE archives (
        username      TEXT NOT NULL,
        month         TEXT NOT NULL,   -- YYYY-MM
        etag          TEXT,
        last_modified TEXT,
        fetched_at    INTEGER NOT NULL,
        PRIMARY KEY (username, month)
    );
    CREATE TABLE games (
        id         TEXT PRIMARY KEY,
        white      TEXT NOT NULL,
        black      TEXT NOT NULL,
        end_time   INTEGER NOT NULL,
        time_class TEXT NOT NULL,
        rated      INTEGER NOT NULL,
        data       TEXT NOT NULL       -- the archived game (JSON), PGN included
    );
    CREATE INDEX games_by_white ON games (white, end_time);
    CREATE INDEX games_by_black ON games (black, end_time);",
    // One analysis per game and engine configuration (see `analysis_key`).
    "CREATE TABLE analyses (
        game_id    TEXT NOT NULL,
        engine_key TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        data       TEXT NOT NULL,      -- chess_review::GameAnalysis (JSON)
        PRIMARY KEY (game_id, engine_key)
    );",
    // Time the engines take per position, per configuration, for estimates.
    "CREATE TABLE speeds (
        engine_key TEXT PRIMARY KEY,
        positions  REAL NOT NULL,
        seconds    REAL NOT NULL       -- engine time (one worker) for those positions
    );",
];

/// Above this many positions, older speed measurements weigh less: the
/// totals are halved, so the average follows changes of the machine.
const SPEED_HISTORY: f64 = 5_000.0;

/// Version of the stored analyses: bump it when their meaning changes, so
/// that older ones are not reused.
const ANALYSIS_FORMAT: u32 = 1;

/// Cache key of the analyses made by `engine` (its [`AnalysisEngine::id`])
/// with `settings`: two analyses with the same key are interchangeable.
///
/// [`AnalysisEngine::id`]: chess_review::AnalysisEngine::id
#[must_use]
pub fn analysis_key(engine: &str, settings: &SearchSettings) -> String {
    format!(
        "{engine} | {} | multipv {} | v{ANALYSIS_FORMAT}",
        settings.budget, settings.multipv
    )
}

/// Cached list of a player's months.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedMonths {
    /// Months with games, oldest first.
    pub months: Vec<Month>,
    /// Validators of the response.
    pub validators: Validators,
    /// When it was last downloaded or revalidated (Unix seconds).
    pub fetched_at: i64,
}

/// Cache state of one month of a player's archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedMonth {
    /// Validators of the response.
    pub validators: Validators,
    /// When it was last downloaded or revalidated (Unix seconds).
    pub fetched_at: i64,
}

/// The cache database.
#[derive(Debug)]
pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    /// Opens (or creates) the database at `path`, creating its directory.
    ///
    /// # Errors
    ///
    /// When the file cannot be created or opened, or its schema migrated.
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        // WAL: readers do not wait for the writer.
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Self::with_connection(connection)
    }

    /// An empty database in memory (tests).
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn in_memory() -> Result<Self, Error> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(mut connection: Connection) -> Result<Self, Error> {
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let transaction = connection.transaction()?;
        for (version, migration) in (1..).zip(MIGRATIONS).skip_while(|(v, _)| *v <= version) {
            transaction.execute_batch(migration)?;
            transaction.pragma_update(None, "user_version", version)?;
        }
        transaction.commit()?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves SQLite consistent (transactions roll back).
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Cached months of `username`.
    ///
    /// # Errors
    ///
    /// When SQLite fails or the row is corrupt.
    pub fn months(&self, username: &str) -> Result<Option<CachedMonths>, Error> {
        let row = self
            .connection()
            .query_row(
                "SELECT months, etag, last_modified, fetched_at FROM players WHERE username = ?1",
                [key(username)],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        Validators {
                            etag: row.get(1)?,
                            last_modified: row.get(2)?,
                        },
                        row.get(3)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(months, validators, fetched_at)| {
            let months: Vec<String> =
                serde_json::from_str(&months).map_err(|err| Error::Corrupt(err.to_string()))?;
            let months = months
                .iter()
                .map(|month| month.parse().map_err(Error::Corrupt))
                .collect::<Result<_, _>>()?;
            Ok(CachedMonths {
                months,
                validators,
                fetched_at,
            })
        })
        .transpose()
    }

    /// Records the months of `username`.
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn save_months(
        &self,
        username: &str,
        months: &[Month],
        validators: &Validators,
        fetched_at: i64,
    ) -> Result<(), Error> {
        let months: Vec<String> = months.iter().map(ToString::to_string).collect();
        let months =
            serde_json::to_string(&months).map_err(|err| Error::Corrupt(err.to_string()))?;
        self.connection().execute(
            "INSERT INTO players (username, months, etag, last_modified, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (username) DO UPDATE SET months = ?2, etag = ?3, last_modified = ?4,
                 fetched_at = ?5",
            params![
                key(username),
                months,
                validators.etag,
                validators.last_modified,
                fetched_at
            ],
        )?;
        Ok(())
    }

    /// Marks the months of `username` as still current at `fetched_at`.
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn touch_months(&self, username: &str, fetched_at: i64) -> Result<(), Error> {
        self.connection().execute(
            "UPDATE players SET fetched_at = ?2 WHERE username = ?1",
            params![key(username), fetched_at],
        )?;
        Ok(())
    }

    /// Cache state of `month` of `username`'s archive.
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn month(&self, username: &str, month: Month) -> Result<Option<CachedMonth>, Error> {
        Ok(self
            .connection()
            .query_row(
                "SELECT etag, last_modified, fetched_at FROM archives
                 WHERE username = ?1 AND month = ?2",
                params![key(username), month.to_string()],
                |row| {
                    Ok(CachedMonth {
                        validators: Validators {
                            etag: row.get(0)?,
                            last_modified: row.get(1)?,
                        },
                        fetched_at: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Stores the games of `month` of `username`'s archive (the standard
    /// ones; others cannot be analysed) and its cache state, in one
    /// transaction. Returns the number of games stored.
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn save_month(
        &self,
        username: &str,
        month: Month,
        games: &[ApiGame],
        validators: &Validators,
        fetched_at: i64,
    ) -> Result<usize, Error> {
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        let mut stored = 0;
        {
            let mut insert = transaction.prepare(
                "INSERT INTO games (id, white, black, end_time, time_class, rated, data)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT (id) DO UPDATE SET data = ?7",
            )?;
            for game in games.iter().filter(|game| game.is_standard()) {
                let data =
                    serde_json::to_string(game).map_err(|err| Error::Corrupt(err.to_string()))?;
                insert.execute(params![
                    game.id(),
                    key(&game.white.username),
                    key(&game.black.username),
                    game.end_time,
                    game.time_class.as_str(),
                    game.rated,
                    data,
                ])?;
                stored += 1;
            }
        }
        transaction.execute(
            "INSERT INTO archives (username, month, etag, last_modified, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (username, month) DO UPDATE SET etag = ?3, last_modified = ?4,
                 fetched_at = ?5",
            params![
                key(username),
                month.to_string(),
                validators.etag,
                validators.last_modified,
                fetched_at
            ],
        )?;
        transaction.commit()?;
        Ok(stored)
    }

    /// Marks `month` of `username`'s archive as still current at `fetched_at`.
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn touch_month(&self, username: &str, month: Month, fetched_at: i64) -> Result<(), Error> {
        self.connection().execute(
            "UPDATE archives SET fetched_at = ?3 WHERE username = ?1 AND month = ?2",
            params![key(username), month.to_string(), fetched_at],
        )?;
        Ok(())
    }

    /// Cached games of `username` kept by `filter`, most recent first, at
    /// most `filter.max_games`.
    ///
    /// # Errors
    ///
    /// When SQLite fails or a row is corrupt.
    pub fn games(&self, username: &str, filter: &GameFilter) -> Result<Vec<ApiGame>, Error> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT data FROM games WHERE {} ORDER BY end_time DESC, id DESC LIMIT ?4",
            where_clause(filter)
        ))?;
        let rows = statement.query_map(
            params![
                key(username),
                filter.since.unwrap_or(i64::MIN),
                filter.until.unwrap_or(i64::MAX),
                i64::try_from(filter.max_games).unwrap_or(i64::MAX),
            ],
            |row| row.get::<_, String>(0),
        )?;
        rows.map(|data| serde_json::from_str(&data?).map_err(|err| Error::Corrupt(err.to_string())))
            .collect()
    }

    /// Number of cached games of `username` kept by `filter` (ignoring
    /// `max_games`).
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn count_games(&self, username: &str, filter: &GameFilter) -> Result<usize, Error> {
        let count: i64 = self.connection().query_row(
            &format!("SELECT COUNT(*) FROM games WHERE {}", where_clause(filter)),
            params![
                key(username),
                filter.since.unwrap_or(i64::MIN),
                filter.until.unwrap_or(i64::MAX),
            ],
            |row| row.get(0),
        )?;
        Ok(usize::try_from(count).unwrap_or_default())
    }

    /// The cached game `id`.
    ///
    /// # Errors
    ///
    /// When SQLite fails or the row is corrupt.
    pub fn game(&self, id: &str) -> Result<Option<ApiGame>, Error> {
        let data: Option<String> = self
            .connection()
            .query_row("SELECT data FROM games WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()?;
        data.map(|data| serde_json::from_str(&data).map_err(|err| Error::Corrupt(err.to_string())))
            .transpose()
    }

    /// The analysis of game `id` with cache key `key` ([`analysis_key`]).
    ///
    /// # Errors
    ///
    /// When SQLite fails or the row is corrupt.
    pub fn analysis(&self, id: &str, key: &str) -> Result<Option<GameAnalysis>, Error> {
        let data: Option<String> = self
            .connection()
            .query_row(
                "SELECT data FROM analyses WHERE game_id = ?1 AND engine_key = ?2",
                [id, key],
                |row| row.get(0),
            )
            .optional()?;
        data.map(|data| serde_json::from_str(&data).map_err(|err| Error::Corrupt(err.to_string())))
            .transpose()
    }

    /// The most recent analysis of game `id`, whatever its engine.
    ///
    /// # Errors
    ///
    /// When SQLite fails or the row is corrupt.
    pub fn latest_analysis(&self, id: &str) -> Result<Option<GameAnalysis>, Error> {
        let data: Option<String> = self
            .connection()
            .query_row(
                "SELECT data FROM analyses WHERE game_id = ?1
                 ORDER BY created_at DESC, rowid DESC LIMIT 1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        data.map(|data| serde_json::from_str(&data).map_err(|err| Error::Corrupt(err.to_string())))
            .transpose()
    }

    /// Every analysis of game `id`, the latest first.
    ///
    /// # Errors
    ///
    /// When SQLite fails or a row is corrupt.
    pub fn analyses(&self, id: &str) -> Result<Vec<GameAnalysis>, Error> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT data FROM analyses WHERE game_id = ?1 ORDER BY created_at DESC, rowid DESC",
        )?;
        let rows = statement.query_map([id], |row| row.get::<_, String>(0))?;
        rows.map(|data| serde_json::from_str(&data?).map_err(|err| Error::Corrupt(err.to_string())))
            .collect()
    }

    /// Stores the analysis of game `id` (replacing one with the same key).
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn save_analysis(
        &self,
        id: &str,
        analysis: &GameAnalysis,
        created_at: i64,
    ) -> Result<(), Error> {
        let key = analysis_key(&analysis.engine, &analysis.settings);
        let data =
            serde_json::to_string(analysis).map_err(|err| Error::Corrupt(err.to_string()))?;
        self.connection().execute(
            "INSERT INTO analyses (game_id, engine_key, created_at, data) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (game_id, engine_key) DO UPDATE SET created_at = ?3, data = ?4",
            params![id, key, created_at, data],
        )?;
        Ok(())
    }

    /// Average engine time per position (seconds, one worker) measured
    /// for the configuration `key`, if any.
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn speed(&self, key: &str) -> Result<Option<f64>, Error> {
        let totals: Option<(f64, f64)> = self
            .connection()
            .query_row(
                "SELECT positions, seconds FROM speeds WHERE engine_key = ?1",
                [key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(totals
            .filter(|(positions, _)| *positions > 0.0)
            .map(|(positions, seconds)| seconds / positions))
    }

    /// Adds a measurement: `positions` analysed in `seconds` of engine time.
    ///
    /// # Errors
    ///
    /// When SQLite fails.
    pub fn record_speed(&self, key: &str, positions: f64, seconds: f64) -> Result<(), Error> {
        if positions <= 0.0 || !seconds.is_finite() || seconds < 0.0 {
            return Ok(());
        }
        self.connection().execute(
            "INSERT INTO speeds (engine_key, positions, seconds) VALUES (?1, ?2, ?3)
             ON CONFLICT (engine_key) DO UPDATE SET
                 positions = CASE WHEN positions > ?4 THEN positions / 2 ELSE positions END + ?2,
                 seconds   = CASE WHEN positions > ?4 THEN seconds / 2 ELSE seconds END + ?3",
            params![key, positions, seconds, SPEED_HISTORY],
        )?;
        Ok(())
    }
}

/// Usernames are compared lowercase.
fn key(username: &str) -> String {
    username.to_ascii_lowercase()
}

/// Conditions of `filter` on parameters ?1 (username), ?2 (since), ?3 (until).
fn where_clause(filter: &GameFilter) -> String {
    let mut clause =
        String::from("(white = ?1 OR black = ?1) AND end_time >= ?2 AND end_time < ?3");
    if filter.rated_only {
        clause.push_str(" AND rated = 1");
    }
    if !filter.time_classes.is_empty() {
        // Fixed names from an enum: safe to inline.
        let names: Vec<String> = filter
            .time_classes
            .iter()
            .map(|class| format!("'{}'", class.as_str()))
            .collect();
        clause.push_str(&format!(" AND time_class IN ({})", names.join(", ")));
    }
    clause
}

#[cfg(test)]
mod tests {
    use chess_chesscom::{ApiPlayer, TimeClass};

    use super::*;

    pub(crate) fn game(
        id: &str,
        white: &str,
        black: &str,
        end_time: i64,
        class: TimeClass,
    ) -> ApiGame {
        let player = |name: &str| ApiPlayer {
            username: name.into(),
            rating: 1500,
            result: "win".into(),
        };
        ApiGame {
            url: format!("https://www.chess.com/game/live/{id}"),
            uuid: id.into(),
            pgn: Some("1. e4 e5 *".into()),
            time_control: "180".into(),
            end_time,
            rated: end_time % 2 == 0,
            time_class: class,
            rules: "chess".into(),
            white: player(white),
            black: player(black),
            accuracies: None,
        }
    }

    const OCTOBER: Month = Month {
        year: 2026,
        month: 10,
    };

    #[test]
    fn stores_and_filters_games() {
        let store = Store::in_memory().unwrap();
        let mut variant = game("v", "Alice", "bob", 50, TimeClass::Blitz);
        variant.rules = "chess960".into();
        let games = [
            game("a", "Alice", "bob", 10, TimeClass::Blitz),
            game("b", "carol", "alice", 20, TimeClass::Rapid),
            game("c", "alice", "dave", 30, TimeClass::Blitz),
            game("d", "erin", "frank", 40, TimeClass::Blitz),
            variant,
        ];
        let stored = store
            .save_month("ALICE", OCTOBER, &games, &Validators::default(), 1)
            .unwrap();
        assert_eq!(stored, 4, "the chess960 game is not kept");

        let ids = |filter: &GameFilter| -> Vec<String> {
            store
                .games("alice", filter)
                .unwrap()
                .into_iter()
                .map(|g| g.uuid)
                .collect()
        };
        assert_eq!(ids(&GameFilter::default()), ["c", "b", "a"]);
        let blitz = GameFilter {
            time_classes: vec![TimeClass::Blitz],
            ..GameFilter::default()
        };
        assert_eq!(ids(&blitz), ["c", "a"]);
        let period = GameFilter {
            since: Some(15),
            until: Some(30),
            ..GameFilter::default()
        };
        assert_eq!(ids(&period), ["b"]);
        let rated = GameFilter {
            rated_only: true,
            max_games: 1,
            ..GameFilter::default()
        };
        assert_eq!(ids(&rated), ["c"]);
        assert_eq!(
            store.count_games("alice", &GameFilter::default()).unwrap(),
            3
        );
        assert_eq!(store.game("d").unwrap().map(|g| g.end_time), Some(40));
        assert_eq!(store.game("zzz").unwrap(), None);
    }

    #[test]
    fn remembers_validators() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.months("alice").unwrap(), None);
        let validators = Validators {
            etag: Some("\"v1\"".into()),
            last_modified: None,
        };
        store
            .save_months("Alice", &[OCTOBER], &validators, 100)
            .unwrap();
        store.touch_months("alice", 200).unwrap();
        let cached = store.months("alice").unwrap().unwrap();
        assert_eq!(cached.months, [OCTOBER]);
        assert_eq!(cached.validators, validators);
        assert_eq!(cached.fetched_at, 200);

        assert_eq!(store.month("alice", OCTOBER).unwrap(), None);
        store
            .save_month("alice", OCTOBER, &[], &validators, 300)
            .unwrap();
        store.touch_month("alice", OCTOBER, 400).unwrap();
        let month = store.month("alice", OCTOBER).unwrap().unwrap();
        assert_eq!((month.validators, month.fetched_at), (validators, 400));
    }

    #[test]
    fn caches_analyses_by_engine_and_settings() {
        use chess_review::{Budget, Line, PositionEval, Score};

        let store = Store::in_memory().unwrap();
        let analysis = |engine: &str, depth| GameAnalysis {
            engine: engine.into(),
            settings: SearchSettings {
                budget: Budget::Depth(depth),
                multipv: 2,
            },
            positions: vec![PositionEval {
                lines: vec![Line {
                    score: Score::Cp(depth.try_into().unwrap()),
                    depth,
                    pv: vec!["e2e4".into()],
                }],
                checkmated: false,
            }],
        };
        let deep = analysis("Stockfish 19", 16);
        store.save_analysis("g1", &deep, 10).unwrap();
        store
            .save_analysis("g1", &analysis("chess-bot classic", 8), 20)
            .unwrap();

        let key = analysis_key("Stockfish 19", &deep.settings);
        assert_eq!(key, "Stockfish 19 | depth 16 | multipv 2 | v1");
        assert_eq!(store.analysis("g1", &key).unwrap(), Some(deep));
        assert_eq!(store.analysis("g2", &key).unwrap(), None);
        let other = SearchSettings {
            budget: Budget::Depth(12),
            multipv: 2,
        };
        assert_eq!(
            store
                .analysis("g1", &analysis_key("Stockfish 19", &other))
                .unwrap(),
            None
        );
        assert_eq!(
            store.latest_analysis("g1").unwrap().map(|a| a.engine),
            Some("chess-bot classic".into())
        );
        let engines: Vec<String> = store
            .analyses("g1")
            .unwrap()
            .into_iter()
            .map(|a| a.engine)
            .collect();
        assert_eq!(engines, ["chess-bot classic", "Stockfish 19"]);
        assert!(store.analyses("g2").unwrap().is_empty());
    }

    #[test]
    fn averages_engine_speeds() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.speed("sf").unwrap(), None);
        store.record_speed("sf", 100.0, 50.0).unwrap();
        store.record_speed("sf", 100.0, 30.0).unwrap();
        assert_eq!(store.speed("sf").unwrap(), Some(0.4));
        store.record_speed("sf", 0.0, 9.0).unwrap();
        assert_eq!(store.speed("sf").unwrap(), Some(0.4));
        // A long history: the old measurements count half.
        store.record_speed("big", 6_000.0, 6_000.0).unwrap();
        store.record_speed("big", 1_000.0, 4_000.0).unwrap();
        assert_eq!(store.speed("big").unwrap(), Some(7_000.0 / 4_000.0));
        assert_eq!(store.speed("other").unwrap(), None);
    }

    #[test]
    fn reopens_an_existing_database() {
        // Not under the system temp dir: the config tests clear the
        // environment (TEMP included) while they run. Unique: a previous run
        // may still be deleting its directory.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-tmp")
            .join(format!("reviewer-{}-{nanos}", std::process::id()));
        let path = dir.join("cache.sqlite");
        {
            let store = Store::open(&path).unwrap();
            let games = [game("a", "alice", "bob", 10, TimeClass::Blitz)];
            store
                .save_month("alice", OCTOBER, &games, &Validators::default(), 1)
                .unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.count_games("alice", &GameFilter::default()).unwrap(),
            1
        );
        drop(store);
        let _ = std::fs::remove_dir_all(dir);
    }
}
