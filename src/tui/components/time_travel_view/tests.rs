//! Unit tests for time-travel view component.

use chrono::Utc;
use rstest::{fixture, rstest};

use super::*;
use crate::local::{
    CommitMetadata, CommitSha, CommitSnapshot, LineMappingVerification, RepoFilePath,
};
use crate::tui::state::TimeTravelInitParams;

/// Creates a standard context with common settings.
fn create_test_context(state: &TimeTravelState) -> TimeTravelViewContext<'_> {
    TimeTravelViewContext {
        state: Some(state),
        max_width: 80,
        max_height: 0,
    }
}

/// Creates context and renders the view.
fn render_view_with_state(state: &TimeTravelState) -> String {
    let ctx = create_test_context(state);
    TimeTravelViewComponent::view(&ctx)
}

#[fixture]
fn sample_state() -> TimeTravelState {
    let metadata = CommitMetadata::new(
        "abc1234567890".to_owned(),
        "Fix login validation".to_owned(),
        "Alice".to_owned(),
        Utc::now(),
    );
    let snapshot = CommitSnapshot::with_file_content(
        metadata,
        "src/auth.rs".to_owned(),
        "fn login() {\n    validate();\n}\n".to_owned(),
    );

    TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: RepoFilePath::new("src/auth.rs".to_owned()),
        original_line: Some(2),
        line_mapping: Some(LineMappingVerification::exact(2)),
        commit_history: vec![
            CommitSha::new("abc1234567890".to_owned()),
            CommitSha::new("def5678901234".to_owned()),
        ],
        current_index: 0,
    })
}

#[rstest]
fn view_shows_commit_header(sample_state: TimeTravelState) {
    let output = render_view_with_state(&sample_state);

    assert!(output.contains("abc1234"));
    assert!(output.contains("Fix login validation"));
}

#[rstest]
fn view_shows_file_path(sample_state: TimeTravelState) {
    let output = render_view_with_state(&sample_state);

    assert!(output.contains("src/auth.rs"));
}

#[rstest]
fn view_shows_line_mapping(sample_state: TimeTravelState) {
    let output = render_view_with_state(&sample_state);

    assert!(output.contains("exact match"));
}

#[rstest]
fn view_shows_navigation(sample_state: TimeTravelState) {
    let output = render_view_with_state(&sample_state);

    assert!(output.contains("Commit 1/2"));
    assert!(output.contains("[h] Previous"));
}

#[rstest]
fn view_shows_file_content(sample_state: TimeTravelState) {
    let output = render_view_with_state(&sample_state);

    assert!(output.contains("fn login()"));
    assert!(output.contains("validate()"));
}

#[rstest]
fn view_highlights_target_line(sample_state: TimeTravelState) {
    let output = render_view_with_state(&sample_state);

    // Line 2 should have the > marker
    assert!(output.contains(">  2 |"));
}

#[test]
fn view_shows_placeholder_when_no_state() {
    let ctx = TimeTravelViewContext {
        state: None,
        max_width: 80,
        max_height: 0,
    };

    let output = TimeTravelViewComponent::view(&ctx);

    assert!(output.contains(NO_STATE_PLACEHOLDER));
}

#[test]
fn view_shows_loading() {
    let state = TimeTravelState::loading(RepoFilePath::new("src/main.rs".to_owned()), Some(10));
    let output = render_view_with_state(&state);

    assert!(output.contains(LOADING_PLACEHOLDER));
}

#[test]
fn view_shows_error() {
    let state = TimeTravelState::error(
        "Commit not found".to_owned(),
        RepoFilePath::new("src/main.rs".to_owned()),
    );
    let output = render_view_with_state(&state);

    assert!(output.contains("Error: Commit not found"));
}

#[test]
fn view_preserves_source_line_numbers_through_wrapping() {
    // Create content where line 2 is very long and will wrap
    let long_line = "x".repeat(100);
    let content = format!("short\n{long_line}\nthird");

    let metadata = CommitMetadata::new(
        "abc1234567890".to_owned(),
        "Test".to_owned(),
        "Alice".to_owned(),
        Utc::now(),
    );
    let snapshot =
        CommitSnapshot::with_file_content(metadata, "test.rs".to_owned(), content.clone());

    // Target line 3 (the "third" line)
    let state = TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: RepoFilePath::new("test.rs".to_owned()),
        original_line: Some(3),
        line_mapping: None,
        commit_history: vec![CommitSha::new("abc1234567890".to_owned())],
        current_index: 0,
    });

    // Use narrow width to force wrapping
    let ctx = TimeTravelViewContext {
        state: Some(&state),
        max_width: 40,
        max_height: 0,
    };
    let output = TimeTravelViewComponent::view(&ctx);

    // Line 3 should be highlighted even though line 2 wraps into multiple visual lines
    assert!(
        output.contains(">  3 | third"),
        "Line 3 should be highlighted. Output:\n{output}"
    );

    // Line 2 (wrapped) should NOT have the > marker
    assert!(
        !output.contains(">  2 |"),
        "Line 2 should not be highlighted. Output:\n{output}"
    );

    // Continuation lines should show dots
    assert!(
        output.contains(".. |"),
        "Wrapped continuation should show dots. Output:\n{output}"
    );
}
