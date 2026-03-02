//! Tests for the review comment verification cache.

#![expect(
    clippy::panic_in_result_fn,
    reason = "Test fixtures return Result to use ?, assertions are expected to panic on failure."
)]

use std::collections::HashMap;

use rstest::{fixture, rstest};
use tempfile::TempDir;

use crate::persistence::{PersistenceError, migrate_database};
use crate::telemetry::NoopTelemetrySink;
use crate::verification::{
    CommentVerificationEvidence, CommentVerificationEvidenceKind, CommentVerificationResult,
    CommentVerificationStatus,
};

use super::{ReviewCommentVerificationCache, ReviewCommentVerificationCacheWrite};

type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;

#[fixture]
fn temp_db() -> FixtureResult<(TempDir, String)> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("frankie.sqlite");
    let database_url = db_path.to_string_lossy().to_string();
    Ok((temp_dir, database_url))
}

#[fixture]
fn migrated_cache(
    temp_db: FixtureResult<(TempDir, String)>,
) -> FixtureResult<(TempDir, ReviewCommentVerificationCache)> {
    let (temp_dir, database_url) = temp_db?;
    migrate_database(&database_url, &NoopTelemetrySink)?;
    let cache = ReviewCommentVerificationCache::new(database_url)?;
    Ok((temp_dir, cache))
}

fn sample_result(
    comment_id: u64,
    target_sha: &str,
    status: CommentVerificationStatus,
    evidence: CommentVerificationEvidence,
) -> CommentVerificationResult {
    CommentVerificationResult::new(comment_id, target_sha.to_owned(), status, evidence)
}

#[rstest]
fn cache_round_trips_results(
    migrated_cache: FixtureResult<(TempDir, ReviewCommentVerificationCache)>,
) -> FixtureResult<()> {
    let (_temp_dir, cache) = migrated_cache?;

    let target = "head123";
    let result = sample_result(
        10,
        target,
        CommentVerificationStatus::Verified,
        CommentVerificationEvidence {
            kind: CommentVerificationEvidenceKind::LineChanged,
            message: Some("changed".to_owned()),
        },
    );
    cache.upsert(ReviewCommentVerificationCacheWrite {
        result: &result,
        verified_at_unix: 123,
    })?;

    let loaded = cache.get_for_comments(&[10], target)?;
    let record = loaded.get(&10).expect("record should exist");
    assert_eq!(record.github_comment_id, 10);
    assert_eq!(record.target_sha, target);
    assert_eq!(record.status, CommentVerificationStatus::Verified);
    assert_eq!(
        record.evidence_kind,
        CommentVerificationEvidenceKind::LineChanged
    );
    assert_eq!(record.evidence_message.as_deref(), Some("changed"));
    assert_eq!(record.verified_at_unix, 123);
    Ok(())
}

#[rstest]
fn cache_returns_empty_map_for_empty_input(
    migrated_cache: FixtureResult<(TempDir, ReviewCommentVerificationCache)>,
) -> FixtureResult<()> {
    let (_temp_dir, cache) = migrated_cache?;
    let loaded = cache.get_for_comments(&[], "head")?;
    assert_eq!(loaded, HashMap::new());
    Ok(())
}

#[rstest]
fn cache_reports_missing_schema_when_unmigrated(temp_db: FixtureResult<(TempDir, String)>) {
    let (_temp_dir, database_url) = temp_db.expect("fixture should succeed");
    let cache = ReviewCommentVerificationCache::new(database_url).expect("cache should build");

    let error = cache
        .get_for_comments(&[1], "head")
        .expect_err("expected schema missing error");
    assert_eq!(error, PersistenceError::SchemaNotInitialised);
}
