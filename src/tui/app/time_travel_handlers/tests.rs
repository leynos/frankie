//! Unit tests for time-travel handlers.
//!
//! These tests verify the message handlers for time-travel navigation,
//! including loading state, commit navigation, and error handling.

use super::*;
use crate::github::models::ReviewComment;
use crate::github::models::test_support::minimal_review;
use crate::local::{
    CommitMetadata, CommitSha, CommitSnapshot, LineMappingVerification, RepoFilePath,
};
use chrono::Utc;
use mockall::mock;

// Mock GitOperations using mockall
mock! {
    pub GitOps {}

    impl std::fmt::Debug for GitOps {
        fn fmt<'a>(&self, f: &mut std::fmt::Formatter<'a>) -> std::fmt::Result;
    }

    impl GitOperations for GitOps {
        fn get_commit_snapshot<'a>(
            &self,
            sha: &'a CommitSha,
            file_path: Option<&'a RepoFilePath>,
        ) -> Result<CommitSnapshot, GitOperationError>;

        fn get_file_at_commit<'a>(
            &self,
            sha: &'a CommitSha,
            file_path: &'a RepoFilePath,
        ) -> Result<String, GitOperationError>;

        fn verify_line_mapping<'a>(
            &self,
            request: &'a LineMappingRequest,
        ) -> Result<LineMappingVerification, GitOperationError>;

        fn get_parent_commits<'a>(
            &self,
            sha: &'a CommitSha,
            limit: usize,
        ) -> Result<Vec<CommitSha>, GitOperationError>;

        fn commit_exists<'a>(&self, sha: &'a CommitSha) -> bool;
    }
}

/// Helper to create a test commit snapshot.
fn create_test_snapshot() -> CommitSnapshot {
    let timestamp = Utc::now();
    let metadata = CommitMetadata::new(
        "abc1234567890".to_owned(),
        "Test commit".to_owned(),
        "Test Author".to_owned(),
        timestamp,
    );
    CommitSnapshot::with_file_content(
        metadata,
        "src/main.rs".to_owned(),
        "fn main() {}".to_owned(),
    )
}

#[test]
fn time_travel_params_from_comment() {
    let comment = ReviewComment {
        commit_sha: Some("abc123".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(42),
        ..minimal_review(1, "Test", "alice")
    };

    let params = TimeTravelParams::from_comment(&comment).unwrap();
    assert_eq!(params.commit_sha.as_str(), "abc123");
    assert_eq!(params.file_path.as_str(), "src/main.rs");
    assert_eq!(params.line_number, Some(42));
}

#[test]
fn time_travel_params_missing_sha() {
    let comment = ReviewComment {
        commit_sha: None,
        file_path: Some("src/main.rs".to_owned()),
        ..minimal_review(1, "Test", "alice")
    };

    assert!(TimeTravelParams::from_comment(&comment).is_none());
}

#[test]
fn load_time_travel_state_success() {
    let mut git_ops = MockGitOps::new();
    let test_snapshot = create_test_snapshot();
    let snapshot_clone = test_snapshot.clone();

    // Expect get_commit_snapshot to be called with the commit SHA
    git_ops
        .expect_get_commit_snapshot()
        .times(1)
        .returning(move |_sha, _file_path| Ok(snapshot_clone.clone()));

    // Expect get_parent_commits to be called
    git_ops
        .expect_get_parent_commits()
        .times(1)
        .returning(|_sha, _limit| {
            Ok(vec![
                CommitSha::new("abc1234567890".to_owned()),
                CommitSha::new("def5678901234".to_owned()),
            ])
        });

    // Expect verify_line_mapping to be called with line number
    git_ops
        .expect_verify_line_mapping()
        .times(1)
        .returning(|request| Ok(LineMappingVerification::exact(request.line)));

    let params = TimeTravelParams {
        commit_sha: CommitSha::new("abc1234567890".to_owned()),
        file_path: RepoFilePath::new("src/main.rs".to_owned()),
        line_number: Some(10),
    };

    let state = load_time_travel_state(&git_ops, &params, Some("HEAD")).unwrap();

    assert_eq!(state.snapshot().message(), "Test commit");
    assert_eq!(state.file_path().as_str(), "src/main.rs");
    assert_eq!(state.original_line(), Some(10));
    assert_eq!(state.commit_count(), 2);
}

#[test]
fn load_time_travel_state_commit_not_found() {
    let mut git_ops = MockGitOps::new();

    // Expect get_commit_snapshot to be called and return CommitNotFound error
    git_ops
        .expect_get_commit_snapshot()
        .times(1)
        .returning(|sha, _file_path| Err(GitOperationError::CommitNotFound { sha: sha.clone() }));

    let params = TimeTravelParams {
        commit_sha: CommitSha::new("nonexistent".to_owned()),
        file_path: RepoFilePath::new("src/main.rs".to_owned()),
        line_number: None,
    };

    let result = load_time_travel_state(&git_ops, &params, None);
    assert!(matches!(
        result,
        Err(GitOperationError::CommitNotFound { .. })
    ));
}
