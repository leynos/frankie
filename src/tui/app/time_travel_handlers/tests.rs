//! Unit tests for time-travel handlers.
//!
//! These tests verify the message handlers for time-travel navigation,
//! including loading state, commit navigation, and error handling.

use super::*;
use crate::github::models::ReviewComment;
use crate::github::models::test_support::minimal_review;
use rstest::rstest;

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
