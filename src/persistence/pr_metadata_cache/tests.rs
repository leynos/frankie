//! Tests for the pull request metadata cache.

use diesel::Connection;
use diesel::RunQueryDsl;
use diesel::sql_query;
use diesel::sqlite::SqliteConnection;
use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::{CachedPullRequestMetadata, PullRequestMetadataCache, PullRequestMetadataCacheWrite};
use crate::github::{PullRequestLocator, PullRequestMetadata};
use crate::persistence::{PersistenceError, migrate_database};
use crate::telemetry::NoopTelemetrySink;

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
    migrate_database(&database_url, &NoopTelemetrySink)
        .unwrap_or_else(|error| panic!("migrations should run: {error}"));

    let cache = PullRequestMetadataCache::new(database_url)
        .unwrap_or_else(|error| panic!("cache should build: {error}"));
    (temp_dir, cache)
}

fn parse_locator(pr_number: u64) -> PullRequestLocator {
    let url = format!("https://github.com/owner/repo/pull/{pr_number}");
    PullRequestLocator::parse(&url).unwrap_or_else(|error| panic!("locator should parse: {error}"))
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
