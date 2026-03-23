//! Unit tests for the public time-travel parameter extraction API.

use rstest::rstest;

use crate::github::models::ReviewComment;
use crate::github::models::test_support::minimal_review;

use super::{TimeTravelParams, TimeTravelParamsError};

#[rstest]
fn from_comment_with_all_fields() {
    let comment = ReviewComment {
        commit_sha: Some("abc123".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(42),
        original_line_number: Some(40),
        ..minimal_review(1, "Test comment", "alice")
    };

    let params = TimeTravelParams::from_comment(&comment)
        .expect("should extract params from comment with full metadata");

    assert_eq!(params.commit_sha().as_str(), "abc123");
    assert_eq!(params.file_path().as_str(), "src/main.rs");
    assert_eq!(params.line_number(), Some(42));
}

#[rstest]
fn from_comment_falls_back_to_original_line() {
    let comment = ReviewComment {
        commit_sha: Some("abc123".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: None,
        original_line_number: Some(40),
        ..minimal_review(1, "Test comment", "alice")
    };

    let params = TimeTravelParams::from_comment(&comment)
        .expect("should extract params using original line fallback");

    assert_eq!(params.line_number(), Some(40));
}

#[rstest]
#[case::missing_commit_sha(None, Some("src/main.rs"), TimeTravelParamsError::MissingCommitSha)]
#[case::missing_file_path(Some("abc123"), None, TimeTravelParamsError::MissingFilePath)]
#[case::missing_commit_sha_takes_precedence_when_both_required_fields_are_absent(
    None,
    None,
    TimeTravelParamsError::MissingCommitSha
)]
fn from_comment_missing_fields(
    #[case] commit_sha: Option<&str>,
    #[case] file_path: Option<&str>,
    #[case] expected_error: TimeTravelParamsError,
) {
    let comment = ReviewComment {
        commit_sha: commit_sha.map(str::to_owned),
        file_path: file_path.map(str::to_owned),
        line_number: None,
        original_line_number: None,
        ..minimal_review(1, "Test comment", "alice")
    };

    let err = TimeTravelParams::from_comment(&comment)
        .expect_err("should fail when required time-travel metadata is missing");

    assert_eq!(err, expected_error);
}

#[rstest]
fn from_comment_both_line_fields_absent() {
    let comment = ReviewComment {
        commit_sha: Some("abc123".to_owned()),
        file_path: Some("src/lib.rs".to_owned()),
        line_number: None,
        original_line_number: None,
        ..minimal_review(1, "Test comment", "alice")
    };

    let params = TimeTravelParams::from_comment(&comment)
        .expect("should succeed when both line fields are absent");

    assert_eq!(params.commit_sha().as_str(), "abc123");
    assert_eq!(params.file_path().as_str(), "src/lib.rs");
    assert_eq!(params.line_number(), None);
}
