//! Pull request metadata cache backed by `SQLite`.
//!
//! The GitHub intake layer can optionally persist pull request metadata to a
//! local `SQLite` database so subsequent runs can reuse cached data across
//! sessions. The cache supports a simple TTL expiry policy and stores HTTP
//! validators (`ETag` and `Last-Modified`) so callers can perform conditional
//! requests.

use std::time::{SystemTime, UNIX_EPOCH};

use diesel::Connection;
use diesel::OptionalExtension;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Nullable, Text};
use diesel::sqlite::SqliteConnection;

use crate::github::PullRequestLocator;
use crate::github::models::PullRequestMetadata;

use super::PersistenceError;

const PR_METADATA_CACHE_TABLE: &str = "pr_metadata_cache";

/// Cached pull request metadata along with HTTP validators and expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedPullRequestMetadata {
    /// Cached PR metadata.
    pub metadata: PullRequestMetadata,
    /// Cached `ETag` validator when provided by GitHub.
    pub etag: Option<String>,
    /// Cached Last-Modified validator when provided by GitHub.
    pub last_modified: Option<String>,
    /// Unix timestamp when the cache entry was last fetched or validated.
    pub fetched_at_unix: i64,
    /// Unix timestamp when the cache entry should be treated as stale.
    pub expires_at_unix: i64,
}

impl CachedPullRequestMetadata {
    /// Returns true if the entry is expired at the supplied `now_unix`.
    #[must_use]
    pub const fn is_expired(&self, now_unix: i64) -> bool {
        now_unix >= self.expires_at_unix
    }
}

/// SQLite-backed cache for pull request metadata.
#[derive(Debug, Clone)]
pub struct PullRequestMetadataCache {
    database_url: String,
}

/// Data required to insert or update a cached metadata row.
#[derive(Debug, Clone, Copy)]
pub struct PullRequestMetadataCacheWrite<'a> {
    /// Cached PR metadata.
    pub metadata: &'a PullRequestMetadata,
    /// Cached `ETag` validator when provided by GitHub.
    pub etag: Option<&'a str>,
    /// Cached `Last-Modified` validator when provided by GitHub.
    pub last_modified: Option<&'a str>,
    /// Unix timestamp when the cache entry was last fetched or validated.
    pub fetched_at_unix: i64,
    /// Unix timestamp when the cache entry should be treated as stale.
    pub expires_at_unix: i64,
}

impl PullRequestMetadataCache {
    /// Create a cache wrapper targeting the configured `database_url`.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::BlankDatabaseUrl`] when the URL is blank.
    pub fn new(database_url: impl Into<String>) -> Result<Self, PersistenceError> {
        let database_url_string = database_url.into();
        if database_url_string.trim().is_empty() {
            return Err(PersistenceError::BlankDatabaseUrl);
        }
        Ok(Self {
            database_url: database_url_string,
        })
    }

    /// Fetches a cached metadata entry for the given locator.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError`] when the database cannot be opened, the
    /// cache schema is missing, or the query fails.
    pub fn get(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<Option<CachedPullRequestMetadata>, PersistenceError> {
        #[derive(Debug, QueryableByName)]
        struct Row {
            #[diesel(sql_type = Nullable<Text>)]
            title: Option<String>,
            #[diesel(sql_type = Nullable<Text>)]
            state: Option<String>,
            #[diesel(sql_type = Nullable<Text>)]
            html_url: Option<String>,
            #[diesel(sql_type = Nullable<Text>)]
            author: Option<String>,
            #[diesel(sql_type = Nullable<Text>)]
            etag: Option<String>,
            #[diesel(sql_type = Nullable<Text>)]
            last_modified: Option<String>,
            #[diesel(sql_type = BigInt)]
            fetched_at_unix: i64,
            #[diesel(sql_type = BigInt)]
            expires_at_unix: i64,
        }

        let mut connection = self.establish_connection()?;

        let result: Option<Row> = sql_query(
            "SELECT title, state, html_url, author, etag, last_modified, fetched_at_unix, \
             expires_at_unix \
             FROM pr_metadata_cache \
             WHERE api_base = ? AND owner = ? AND repo = ? AND pr_number = ? \
             LIMIT 1;",
        )
        .bind::<Text, _>(locator.api_base().as_str())
        .bind::<Text, _>(locator.owner().as_str())
        .bind::<Text, _>(locator.repository().as_str())
        .bind::<BigInt, _>(Self::pr_number_to_i64(locator))
        .get_result(&mut connection)
        .optional()
        .map_err(|error| Self::map_query_error(&mut connection, &error))?;

        Ok(result.map(|row| CachedPullRequestMetadata {
            metadata: PullRequestMetadata {
                number: locator.number().get(),
                title: row.title,
                state: row.state,
                html_url: row.html_url,
                author: row.author,
            },
            etag: row.etag,
            last_modified: row.last_modified,
            fetched_at_unix: row.fetched_at_unix,
            expires_at_unix: row.expires_at_unix,
        }))
    }

    /// Inserts or updates a cache entry.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError`] when the schema is missing or the write
    /// fails.
    pub fn upsert(
        &self,
        locator: &PullRequestLocator,
        write: PullRequestMetadataCacheWrite<'_>,
    ) -> Result<(), PersistenceError> {
        let mut connection = self.establish_connection()?;

        let metadata = write.metadata;

        sql_query(
            "INSERT INTO pr_metadata_cache \
             (api_base, owner, repo, pr_number, title, state, html_url, author, etag, \
              last_modified, fetched_at_unix, expires_at_unix) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(api_base, owner, repo, pr_number) DO UPDATE SET \
               title = excluded.title, \
               state = excluded.state, \
               html_url = excluded.html_url, \
               author = excluded.author, \
               etag = excluded.etag, \
               last_modified = excluded.last_modified, \
               fetched_at_unix = excluded.fetched_at_unix, \
               expires_at_unix = excluded.expires_at_unix, \
               updated_at = CURRENT_TIMESTAMP;",
        )
        .bind::<Text, _>(locator.api_base().as_str())
        .bind::<Text, _>(locator.owner().as_str())
        .bind::<Text, _>(locator.repository().as_str())
        .bind::<BigInt, _>(Self::pr_number_to_i64(locator))
        .bind::<Nullable<Text>, _>(metadata.title.as_deref())
        .bind::<Nullable<Text>, _>(metadata.state.as_deref())
        .bind::<Nullable<Text>, _>(metadata.html_url.as_deref())
        .bind::<Nullable<Text>, _>(metadata.author.as_deref())
        .bind::<Nullable<Text>, _>(write.etag)
        .bind::<Nullable<Text>, _>(write.last_modified)
        .bind::<BigInt, _>(write.fetched_at_unix)
        .bind::<BigInt, _>(write.expires_at_unix)
        .execute(&mut connection)
        .map(drop)
        .map_err(|error| Self::map_write_error(&mut connection, &error))
    }

    /// Updates the expiry for an existing cache entry (for a 304 response).
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError`] when the schema is missing or the write
    /// fails.
    pub fn touch(
        &self,
        locator: &PullRequestLocator,
        fetched_at_unix: i64,
        expires_at_unix: i64,
    ) -> Result<(), PersistenceError> {
        let mut connection = self.establish_connection()?;

        let affected = sql_query(
            "UPDATE pr_metadata_cache \
             SET fetched_at_unix = ?, expires_at_unix = ?, updated_at = CURRENT_TIMESTAMP \
             WHERE api_base = ? AND owner = ? AND repo = ? AND pr_number = ?;",
        )
        .bind::<BigInt, _>(fetched_at_unix)
        .bind::<BigInt, _>(expires_at_unix)
        .bind::<Text, _>(locator.api_base().as_str())
        .bind::<Text, _>(locator.owner().as_str())
        .bind::<Text, _>(locator.repository().as_str())
        .bind::<BigInt, _>(Self::pr_number_to_i64(locator))
        .execute(&mut connection)
        .map_err(|error| Self::map_write_error(&mut connection, &error))?;

        if affected == 0 {
            return Err(PersistenceError::WriteFailed {
                message: "expected to update 1 row but updated 0".to_owned(),
            });
        }

        Ok(())
    }

    /// Returns the current unix timestamp in seconds.
    #[must_use]
    pub fn now_unix_seconds() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(0)
    }

    fn establish_connection(&self) -> Result<SqliteConnection, PersistenceError> {
        let mut connection = SqliteConnection::establish(&self.database_url).map_err(|error| {
            PersistenceError::ConnectionFailed {
                message: error.to_string(),
            }
        })?;

        sql_query("PRAGMA foreign_keys = ON;")
            .execute(&mut connection)
            .map(drop)
            .map_err(|error| PersistenceError::ForeignKeysEnableFailed {
                message: error.to_string(),
            })?;

        Ok(connection)
    }

    fn pr_number_to_i64(locator: &PullRequestLocator) -> i64 {
        i64::try_from(locator.number().get()).unwrap_or(i64::MAX)
    }

    fn cache_table_exists(
        connection: &mut SqliteConnection,
    ) -> Result<bool, diesel::result::Error> {
        #[derive(Debug, QueryableByName)]
        struct Row {
            #[diesel(sql_type = BigInt)]
            one: i64,
        }

        let exists: Option<Row> = sql_query(
            "SELECT 1 AS one FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1;",
        )
        .bind::<Text, _>(PR_METADATA_CACHE_TABLE)
        .get_result(connection)
        .optional()?;

        let _ = exists.as_ref().map(|row| row.one);
        Ok(exists.is_some())
    }

    fn map_error_with_schema_check<F>(
        connection: &mut SqliteConnection,
        error: &diesel::result::Error,
        create_error: F,
    ) -> PersistenceError
    where
        F: Fn(String) -> PersistenceError,
    {
        match Self::cache_table_exists(connection) {
            Ok(false) => PersistenceError::SchemaNotInitialised,
            Ok(true) => create_error(error.to_string()),
            Err(check_error) => create_error(format!(
                "schema presence check failed: {check_error}; original error: {error}"
            )),
        }
    }

    fn map_query_error(
        connection: &mut SqliteConnection,
        error: &diesel::result::Error,
    ) -> PersistenceError {
        Self::map_error_with_schema_check(connection, error, |message| {
            PersistenceError::QueryFailed { message }
        })
    }

    fn map_write_error(
        connection: &mut SqliteConnection,
        error: &diesel::result::Error,
    ) -> PersistenceError {
        Self::map_error_with_schema_check(connection, error, |message| {
            PersistenceError::WriteFailed { message }
        })
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use tempfile::TempDir;

    use diesel::Connection;
    use diesel::RunQueryDsl;
    use diesel::sql_query;
    use diesel::sqlite::SqliteConnection;

    use super::{
        CachedPullRequestMetadata, PullRequestMetadataCache, PullRequestMetadataCacheWrite,
    };
    use crate::github::{PullRequestLocator, PullRequestMetadata};
    use crate::persistence::{PersistenceError, migrate_database};
    use crate::telemetry::{TelemetryEvent, TelemetrySink};

    #[derive(Debug, Default)]
    struct NoopTelemetry;

    impl TelemetrySink for NoopTelemetry {
        fn record(&self, _event: TelemetryEvent) {}
    }

    #[fixture]
    fn temp_db() -> (TempDir, String) {
        let temp_dir =
            TempDir::new().unwrap_or_else(|error| panic!("temp dir should be created: {error}"));
        let db_path = temp_dir.path().join("frankie.sqlite");
        (temp_dir, db_path.to_string_lossy().to_string())
    }

    #[fixture]
    fn migrated_cache(temp_db: (TempDir, String)) -> (TempDir, PullRequestMetadataCache) {
        let (temp_dir, database_url) = temp_db;
        migrate_database(&database_url, &NoopTelemetry)
            .unwrap_or_else(|error| panic!("migrations should run: {error}"));

        let cache = PullRequestMetadataCache::new(database_url)
            .unwrap_or_else(|error| panic!("cache should build: {error}"));
        (temp_dir, cache)
    }

    fn parse_locator(pr_number: u64) -> PullRequestLocator {
        let url = format!("https://github.com/owner/repo/pull/{pr_number}");
        PullRequestLocator::parse(&url)
            .unwrap_or_else(|error| panic!("locator should parse: {error}"))
    }

    #[rstest]
    fn cache_round_trips_metadata(migrated_cache: (TempDir, PullRequestMetadataCache)) {
        let (_temp_dir, cache) = migrated_cache;
        let locator = parse_locator(42);

        let metadata = PullRequestMetadata {
            number: 42,
            title: Some("Add cache".to_owned()),
            state: Some("open".to_owned()),
            html_url: Some("https://example.invalid".to_owned()),
            author: Some("octocat".to_owned()),
        };

        let fetched_at = 10;
        let expires_at = 20;

        cache
            .upsert(
                &locator,
                PullRequestMetadataCacheWrite {
                    metadata: &metadata,
                    etag: Some("\"etag-1\""),
                    last_modified: Some("Mon, 01 Jan 2025 00:00:00 GMT"),
                    fetched_at_unix: fetched_at,
                    expires_at_unix: expires_at,
                },
            )
            .unwrap_or_else(|error| panic!("upsert should succeed: {error}"));

        let cached = cache
            .get(&locator)
            .unwrap_or_else(|error| panic!("cache get should succeed: {error}"));
        let entry = cached.unwrap_or_else(|| panic!("entry should exist"));

        assert_eq!(
            entry,
            CachedPullRequestMetadata {
                metadata,
                etag: Some("\"etag-1\"".to_owned()),
                last_modified: Some("Mon, 01 Jan 2025 00:00:00 GMT".to_owned()),
                fetched_at_unix: fetched_at,
                expires_at_unix: expires_at,
            }
        );
    }

    #[rstest]
    fn cache_touch_updates_expiry(migrated_cache: (TempDir, PullRequestMetadataCache)) {
        let (_temp_dir, cache) = migrated_cache;
        let locator = parse_locator(1);

        let metadata = PullRequestMetadata {
            number: 1,
            title: None,
            state: None,
            html_url: None,
            author: None,
        };

        cache
            .upsert(
                &locator,
                PullRequestMetadataCacheWrite {
                    metadata: &metadata,
                    etag: None,
                    last_modified: None,
                    fetched_at_unix: 100,
                    expires_at_unix: 200,
                },
            )
            .unwrap_or_else(|error| panic!("upsert should succeed: {error}"));
        cache
            .touch(&locator, 300, 400)
            .unwrap_or_else(|error| panic!("touch should succeed: {error}"));

        let cached = cache
            .get(&locator)
            .unwrap_or_else(|error| panic!("cache get should succeed: {error}"))
            .unwrap_or_else(|| panic!("entry should exist"));

        assert_eq!(cached.fetched_at_unix, 300);
        assert_eq!(cached.expires_at_unix, 400);
    }

    #[rstest]
    fn cache_reports_missing_schema_when_unmigrated(temp_db: (TempDir, String)) {
        let (_temp_dir, database_url) = temp_db;
        let cache = PullRequestMetadataCache::new(database_url)
            .unwrap_or_else(|error| panic!("cache should build: {error}"));
        let locator = parse_locator(1);

        let error = cache
            .get(&locator)
            .expect_err("unmigrated database should fail");

        assert_eq!(error, PersistenceError::SchemaNotInitialised);
    }

    #[rstest]
    fn cache_distinguishes_missing_table_from_query_failures(temp_db: (TempDir, String)) {
        let (_temp_dir, database_url) = temp_db;

        let mut connection = SqliteConnection::establish(&database_url)
            .unwrap_or_else(|error| panic!("connection should succeed: {error}"));
        sql_query("CREATE TABLE pr_metadata_cache (id INTEGER PRIMARY KEY);")
            .execute(&mut connection)
            .unwrap_or_else(|error| panic!("table should be created: {error}"));

        let cache = PullRequestMetadataCache::new(database_url)
            .unwrap_or_else(|error| panic!("cache should build: {error}"));
        let locator = parse_locator(1);

        let error = cache
            .get(&locator)
            .expect_err("malformed schema should still fail");

        assert!(
            matches!(error, PersistenceError::QueryFailed { .. }),
            "expected QueryFailed, got {error:?}"
        );
    }
}
