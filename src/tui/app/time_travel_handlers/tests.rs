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
use rstest::rstest;

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

#[rstest]
#[case(None, Some("src/main.rs".to_owned()), "review comment is missing a commit SHA")]
#[case(Some("abc123".to_owned()), None, "review comment is missing a file path")]
fn handle_enter_time_travel_surfaces_metadata_error(
    #[case] commit_sha: Option<String>,
    #[case] file_path: Option<String>,
    #[case] expected_error: &str,
) {
    let comment = ReviewComment {
        file_path,
        commit_sha,
        ..minimal_review(1, "Test", "alice")
    };
    let mut app = ReviewApp::new(vec![comment]);

    let cmd = app.handle_enter_time_travel();

    assert!(cmd.is_none());
    assert_eq!(app.error.as_deref(), Some(expected_error));
}

#[test]
fn load_time_travel_state_success() -> Result<(), Box<dyn std::error::Error>> {
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

    let comment = ReviewComment {
        commit_sha: Some("abc1234567890".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(10),
        ..minimal_review(2, "Load test", "bob")
    };
    let params = TimeTravelParams::from_comment(&comment)?;

    let head_sha = CommitSha::new("HEAD".to_owned());
    let state = load_time_travel_state(&git_ops, &params, Some(&head_sha))?;

    if state.snapshot().message() != "Test commit" {
        return Err("expected snapshot message to match test commit".into());
    }
    if state.file_path().as_str() != "src/main.rs" {
        return Err("expected file path to match source file".into());
    }
    if state.original_line() != Some(10) {
        return Err("expected original line to match requested line".into());
    }
    if state.commit_count() != 2 {
        return Err("expected commit count to include current and parent commit".into());
    }

    Ok(())
}

#[test]
fn load_time_travel_state_commit_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let mut git_ops = MockGitOps::new();

    // Expect get_commit_snapshot to be called and return CommitNotFound error
    git_ops
        .expect_get_commit_snapshot()
        .times(1)
        .returning(|sha, _file_path| Err(GitOperationError::CommitNotFound { sha: sha.clone() }));

    let comment = ReviewComment {
        commit_sha: Some("nonexistent".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: None,
        ..minimal_review(3, "Missing commit test", "charlie")
    };
    let params = TimeTravelParams::from_comment(&comment)?;

    let result = load_time_travel_state(&git_ops, &params, None);
    if !matches!(result, Err(GitOperationError::CommitNotFound { .. })) {
        return Err("expected missing commit to surface CommitNotFound".into());
    }

    Ok(())
}
