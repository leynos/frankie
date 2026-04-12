//! Unit tests for `ReviewApp` builder methods and default values.
//!
//! These tests verify builder method chaining for `with_commit_history_limit`
//! and the `default_commit_history_limit` fallback used during construction.

use rstest::{fixture, rstest};

use crate::config::DEFAULT_COMMIT_HISTORY_LIMIT;
use crate::github::models::ReviewComment;
use crate::github::models::test_support::minimal_review;

use super::ReviewApp;
use super::builder;

struct CommitHistoryLimitCase {
    limits: Vec<usize>,
    width: u16,
    height: u16,
    expected_limit: usize,
    expected_filtered_count: usize,
}

#[fixture]
fn base_reviews() -> Vec<ReviewComment> {
    vec![minimal_review(1, "Test", "alice")]
}

/// Verifies that a freshly constructed `ReviewApp` uses the
/// `DEFAULT_COMMIT_HISTORY_LIMIT` constant as its commit history limit.
#[test]
fn new_app_uses_default_commit_history_limit() {
    let app = ReviewApp::with_dimensions(Vec::new(), 80, 24);

    assert_eq!(
        app.commit_history_limit, DEFAULT_COMMIT_HISTORY_LIMIT,
        "new app should use DEFAULT_COMMIT_HISTORY_LIMIT ({DEFAULT_COMMIT_HISTORY_LIMIT})"
    );
}

/// Verifies that `default_commit_history_limit()` returns the same value
/// as the public constant `DEFAULT_COMMIT_HISTORY_LIMIT`.
#[test]
fn default_commit_history_limit_matches_constant() {
    assert_eq!(
        builder::default_commit_history_limit(),
        DEFAULT_COMMIT_HISTORY_LIMIT,
        "builder helper should delegate to the config constant"
    );
}

/// Verifies commit-history-limit builder behaviour across the common
/// override and chaining scenarios.
#[rstest]
#[case::override_limit(CommitHistoryLimitCase {
    limits: vec![25],
    width: 80,
    height: 24,
    expected_limit: 25,
    expected_filtered_count: 1,
})]
#[case::last_write_wins(CommitHistoryLimitCase {
    limits: vec![10, 75],
    width: 80,
    height: 24,
    expected_limit: 75,
    expected_filtered_count: 1,
})]
#[case::minimum_value(CommitHistoryLimitCase {
    limits: vec![1],
    width: 80,
    height: 24,
    expected_limit: 1,
    expected_filtered_count: 1,
})]
#[case::builder_chaining(CommitHistoryLimitCase {
    limits: vec![30],
    width: 120,
    height: 40,
    expected_limit: 30,
    expected_filtered_count: 1,
})]
fn with_commit_history_limit_cases(
    base_reviews: Vec<ReviewComment>,
    #[case] case: CommitHistoryLimitCase,
) {
    let mut app = ReviewApp::with_dimensions(base_reviews, case.width, case.height);

    for limit in case.limits {
        app = app.with_commit_history_limit(limit);
    }

    assert_eq!(
        app.commit_history_limit, case.expected_limit,
        "builder should preserve the configured commit history limit"
    );
    assert_eq!(
        app.width, case.width,
        "builder should preserve explicit width"
    );
    assert_eq!(
        app.height, case.height,
        "builder should preserve explicit height"
    );
    assert_eq!(
        app.filtered_count(),
        case.expected_filtered_count,
        "builder should preserve the filtered review count"
    );
}
