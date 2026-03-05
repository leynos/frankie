//! Tests for the review comment verification cache.

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
#[expect(
    clippy::panic_in_result_fn,
    reason = "Fixture-based test returns Result and still uses assertions for state checks."
)]
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
#[expect(
    clippy::panic_in_result_fn,
    reason = "Fixture-based test returns Result and still uses assertions for state checks."
)]
fn cache_returns_empty_map_for_empty_input(
    migrated_cache: FixtureResult<(TempDir, ReviewCommentVerificationCache)>,
) -> FixtureResult<()> {
    let (_temp_dir, cache) = migrated_cache?;
    let loaded = cache.get_for_comments(&[], "head")?;
    assert_eq!(loaded, HashMap::new());
    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Fixture-based test returns Result and still uses assertions for state checks."
)]
fn upsert_overwrites_existing_row_for_same_comment_and_target(
    migrated_cache: FixtureResult<(TempDir, ReviewCommentVerificationCache)>,
) -> FixtureResult<()> {
    let (_temp_dir, cache) = migrated_cache?;

    let target = "head123";
    let comment_id = 42_u64;
    let first_result = sample_result(
        comment_id,
        target,
        CommentVerificationStatus::Verified,
        CommentVerificationEvidence {
            kind: CommentVerificationEvidenceKind::LineChanged,
            message: Some("first evidence".to_owned()),
        },
    );
    cache.upsert(ReviewCommentVerificationCacheWrite {
        result: &first_result,
        verified_at_unix: 100,
    })?;

    let second_result = sample_result(
        comment_id,
        target,
        CommentVerificationStatus::Unverified,
        CommentVerificationEvidence {
            kind: CommentVerificationEvidenceKind::LineUnchanged,
            message: Some("second evidence".to_owned()),
        },
    );
    cache.upsert(ReviewCommentVerificationCacheWrite {
        result: &second_result,
        verified_at_unix: 200,
    })?;

    let loaded = cache.get_for_comments(&[comment_id], target)?;
    let record = loaded.get(&comment_id).expect("record should exist");
    assert_eq!(record.status, CommentVerificationStatus::Unverified);
    assert_eq!(
        record.evidence_kind,
        CommentVerificationEvidenceKind::LineUnchanged
    );
    assert_eq!(record.evidence_message.as_deref(), Some("second evidence"));
    assert_eq!(record.verified_at_unix, 200);
    Ok(())
}

#[rstest]
fn cache_reports_missing_schema_when_unmigrated(
    temp_db: FixtureResult<(TempDir, String)>,
) -> FixtureResult<()> {
    let (_temp_dir, database_url) = temp_db?;
    let cache = ReviewCommentVerificationCache::new(database_url)?;

    let result = cache.get_for_comments(&[1], "head");
    if !matches!(result, Err(PersistenceError::SchemaNotInitialised)) {
        return Err(format!("expected SchemaNotInitialised, got {:?}", result.err()).into());
    }
    Ok(())
}

#[test]
fn cache_new_rejects_blank_database_url() {
    let result = ReviewCommentVerificationCache::new("   ");
    assert!(
        matches!(result, Err(PersistenceError::BlankDatabaseUrl)),
        "expected BlankDatabaseUrl, got {result:?}"
    );
}
