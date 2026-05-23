//! Unit tests for time-travel handlers.
//!
//! These tests verify the message handlers for time-travel navigation,
//! including loading state, commit navigation, and error handling.

use super::*;
use crate::github::models::ReviewComment;
use crate::github::models::test_support::minimal_review;
use crate::local::{
    CommitMetadata, CommitSha, CommitSnapshot, LineMappingRequest, LineMappingVerification,
    RepoFilePath,
};
use crate::time_travel::TimeTravelInitParams;
use crate::tui::app::ViewMode;
use rstest::rstest;
use std::sync::Arc;

#[derive(Debug)]
struct NoopGitOps;

impl GitOperations for NoopGitOps {
    fn get_commit_snapshot(
        &self,
        sha: &CommitSha,
        file_path: Option<&RepoFilePath>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        let metadata = CommitMetadata::new(
            sha.as_str().to_owned(),
            "Loaded".to_owned(),
            "Alice".to_owned(),
            chrono::Utc::now(),
        );
        Ok(if let Some(path) = file_path {
            CommitSnapshot::with_file_content(
                metadata,
                path.as_str().to_owned(),
                "fn main() {}".to_owned(),
            )
        } else {
            CommitSnapshot::new(metadata)
        })
    }

    fn get_file_at_commit(
        &self,
        _sha: &CommitSha,
        _file_path: &RepoFilePath,
    ) -> Result<String, GitOperationError> {
        Ok("fn main() {}".to_owned())
    }

    fn verify_line_mapping(
        &self,
        request: &LineMappingRequest,
    ) -> Result<LineMappingVerification, GitOperationError> {
        Ok(LineMappingVerification::exact(request.line))
    }

    fn get_parent_commits(
        &self,
        _sha: &CommitSha,
        limit: usize,
    ) -> Result<Vec<CommitSha>, GitOperationError> {
        Ok(["abc1234567890", "def5678901234", "ghi9012345678"]
            .into_iter()
            .take(limit)
            .map(|sha| CommitSha::new(sha.to_owned()))
            .collect())
    }

    fn commit_exists(&self, _sha: &CommitSha) -> bool {
        true
    }
}

fn loaded_state_at(index: usize) -> TimeTravelState {
    let history: Vec<_> = ["abc1234567890", "def5678901234", "ghi9012345678"]
        .into_iter()
        .map(|sha| CommitSha::new(sha.to_owned()))
        .collect();
    let sha = history
        .get(index)
        .map_or("abc1234567890", CommitSha::as_str)
        .to_owned();
    let metadata = CommitMetadata::new(
        sha,
        "Loaded".to_owned(),
        "Alice".to_owned(),
        chrono::Utc::now(),
    );
    TimeTravelState::new(TimeTravelInitParams {
        snapshot: CommitSnapshot::with_file_content(
            metadata,
            "src/main.rs".to_owned(),
            "fn main() {}".to_owned(),
        ),
        file_path: RepoFilePath::new("src/main.rs".to_owned()),
        original_line: Some(10),
        line_mapping: None,
        commit_history: history,
        current_index: index,
    })
}

fn time_travel_app_at(index: usize) -> ReviewApp {
    let mut app = ReviewApp::new(vec![minimal_review(1, "Test", "alice")])
        .with_git_ops(Arc::new(NoopGitOps), "HEAD".to_owned());
    app.view_mode = ViewMode::TimeTravel;
    app.time_travel_state = Some(loaded_state_at(index));
    app
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
fn rapid_navigation_is_blocked_while_navigation_is_loading() {
    let mut app = time_travel_app_at(0);

    let first_cmd = app.handle_previous_commit();
    let second_cmd = app.handle_previous_commit();

    assert!(first_cmd.is_some());
    assert!(second_cmd.is_none());
    assert!(
        app.time_travel_state
            .as_ref()
            .is_some_and(TimeTravelState::is_loading)
    );
}

#[test]
fn exit_during_navigation_ignores_late_navigation_result() {
    let mut app = time_travel_app_at(0);

    let cmd = app.handle_previous_commit();
    let exit_cmd = app.handle_exit_time_travel();
    let late_cmd = app.handle_commit_navigated(Box::new(loaded_state_at(1)));

    assert!(cmd.is_some());
    assert!(exit_cmd.is_none());
    assert!(late_cmd.is_none());
    assert!(matches!(app.view_mode, ViewMode::ReviewList));
    assert!(app.time_travel_state.is_none());
}

#[test]
fn exit_during_initial_load_ignores_late_load_result() {
    let mut app = time_travel_app_at(0);

    let exit_cmd = app.handle_exit_time_travel();
    let late_cmd = app.handle_time_travel_loaded(Box::new(loaded_state_at(1)));

    assert!(exit_cmd.is_none());
    assert!(late_cmd.is_none());
    assert!(matches!(app.view_mode, ViewMode::ReviewList));
    assert!(app.time_travel_state.is_none());
}
