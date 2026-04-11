//! Unit tests for `ReviewApp` builder methods and default values.
//!
//! These tests verify builder method chaining for `with_commit_history_limit`
//! and the `default_commit_history_limit` fallback used during construction.

use crate::config::DEFAULT_COMMIT_HISTORY_LIMIT;
use crate::github::models::test_support::minimal_review;

use super::ReviewApp;
use super::builder;

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

/// Verifies that `with_commit_history_limit` overrides the default value.
#[test]
fn with_commit_history_limit_overrides_default() {
    let reviews = vec![minimal_review(1, "Test", "alice")];
    let app = ReviewApp::with_dimensions(reviews, 80, 24).with_commit_history_limit(25);

    assert_eq!(
        app.commit_history_limit, 25,
        "builder should override the default commit history limit"
    );
}

/// Verifies that chaining `with_commit_history_limit` twice takes the
/// last value, confirming idempotent setter semantics.
#[test]
fn with_commit_history_limit_last_write_wins() {
    let app = ReviewApp::with_dimensions(Vec::new(), 80, 24)
        .with_commit_history_limit(10)
        .with_commit_history_limit(75);

    assert_eq!(
        app.commit_history_limit, 75,
        "last call to with_commit_history_limit should take precedence"
    );
}

/// Verifies that `with_commit_history_limit(1)` is accepted as the
/// minimum meaningful limit (zero is clamped at load time, but the
/// builder itself does not clamp).
#[test]
fn with_commit_history_limit_accepts_minimum_value() {
    let app = ReviewApp::with_dimensions(Vec::new(), 80, 24).with_commit_history_limit(1);

    assert_eq!(app.commit_history_limit, 1);
}

/// Verifies that builder chaining composes correctly when
/// `with_commit_history_limit` is combined with other builder methods.
#[test]
fn builder_chaining_preserves_commit_history_limit() {
    let reviews = vec![minimal_review(1, "Test", "alice")];
    let app = ReviewApp::with_dimensions(reviews, 120, 40).with_commit_history_limit(30);

    assert_eq!(app.commit_history_limit, 30);
    assert_eq!(app.width, 120);
    assert_eq!(app.height, 40);
    assert_eq!(app.filtered_count(), 1);
}
