//! Tests for the pull request metadata cache.

type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;

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
fn temp_db() -> FixtureResult<(TempDir, String)> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("frankie.sqlite");
    Ok((temp_dir, db_path.to_string_lossy().to_string()))
}

#[fixture]
fn migrated_cache(
    temp_db: FixtureResult<(TempDir, String)>,
) -> FixtureResult<(TempDir, PullRequestMetadataCache)> {
    let (temp_dir, database_url) = temp_db?;
    migrate_database(&database_url, &NoopTelemetrySink)?;

    let cache = PullRequestMetadataCache::new(database_url)?;
    Ok((temp_dir, cache))
}

fn parse_locator(pr_number: u64) -> FixtureResult<PullRequestLocator> {
    let url = format!("https://github.com/owner/repo/pull/{pr_number}");
    Ok(PullRequestLocator::parse(&url)?)
}

#[rstest]
fn cache_round_trips_metadata(migrated_cache: FixtureResult<(TempDir, PullRequestMetadataCache)>) {
    let (_temp_dir, cache) = migrated_cache.expect("fixture should succeed");
    let locator = parse_locator(42).expect("locator should parse");

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
        .expect("upsert should succeed");

    let cached = cache.get(&locator).expect("cache get should succeed");
    let entry = cached.expect("entry should exist");

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
fn cache_touch_updates_expiry(migrated_cache: FixtureResult<(TempDir, PullRequestMetadataCache)>) {
    let (_temp_dir, cache) = migrated_cache.expect("fixture should succeed");
    let locator = parse_locator(1).expect("locator should parse");

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
        .expect("upsert should succeed");
    cache
        .touch(&locator, 300, 400)
        .expect("touch should succeed");

    let cached = cache
        .get(&locator)
        .expect("cache get should succeed")
        .expect("entry should exist");

    assert_eq!(cached.fetched_at_unix, 300);
    assert_eq!(cached.expires_at_unix, 400);
}

#[rstest]
fn cache_reports_missing_schema_when_unmigrated(temp_db: FixtureResult<(TempDir, String)>) {
    let (_temp_dir, database_url) = temp_db.expect("fixture should succeed");
    let cache = PullRequestMetadataCache::new(database_url).expect("cache should build");
    let locator = parse_locator(1).expect("locator should parse");

    let error = cache
        .get(&locator)
        .expect_err("unmigrated database should fail");

    assert_eq!(error, PersistenceError::SchemaNotInitialised);
}

#[rstest]
fn cache_distinguishes_missing_table_from_query_failures(
    temp_db: FixtureResult<(TempDir, String)>,
) {
    let (_temp_dir, database_url) = temp_db.expect("fixture should succeed");

    let mut connection =
        SqliteConnection::establish(&database_url).expect("connection should succeed");
    sql_query("CREATE TABLE pr_metadata_cache (id INTEGER PRIMARY KEY);")
        .execute(&mut connection)
        .expect("table should be created");

    let cache = PullRequestMetadataCache::new(database_url).expect("cache should build");
    let locator = parse_locator(1).expect("locator should parse");

    let error = cache
        .get(&locator)
        .expect_err("malformed schema should still fail");

    assert!(
        matches!(error, PersistenceError::QueryFailed { .. }),
        "expected QueryFailed, got {error:?}"
    );
}
