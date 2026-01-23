//! Unit tests for time-travel state management.
//!
//! These tests verify the `TimeTravelState` struct's navigation logic,
//! loading/error states, and parameter extraction from review comments.

#![expect(clippy::unwrap_used, reason = "Test assertions panic on failure")]

use chrono::Utc;
use rstest::{fixture, rstest};

use super::*;
use crate::github::models::test_support::minimal_review;
use crate::local::LineMappingStatus;
// TimeTravelInitParams is already in scope via `use super::*`

/// Expected navigation properties for test assertions.
#[derive(Debug, Clone)]
struct ExpectedNavigation<'a> {
    can_previous: bool,
    can_next: bool,
    next_sha: Option<&'a str>,
    prev_sha: Option<&'a str>,
}

impl<'a> ExpectedNavigation<'a> {
    /// Returns navigation state at index 0 (can go previous, cannot go next).
    const fn at_newest(prev_sha: &'a str) -> Self {
        Self {
            can_previous: true,
            can_next: false,
            next_sha: None,
            prev_sha: Some(prev_sha),
        }
    }

    /// Returns navigation state in the middle (can go both ways).
    const fn at_middle(next_sha: &'a str, prev_sha: &'a str) -> Self {
        Self {
            can_previous: true,
            can_next: true,
            next_sha: Some(next_sha),
            prev_sha: Some(prev_sha),
        }
    }

    /// Returns navigation state at last index (cannot go previous, can go next).
    const fn at_oldest(next_sha: &'a str) -> Self {
        Self {
            can_previous: false,
            can_next: true,
            next_sha: Some(next_sha),
            prev_sha: None,
        }
    }

    /// Returns navigation state when loading (cannot navigate either way).
    const fn blocked(prev_sha: Option<&'a str>) -> Self {
        Self {
            can_previous: false,
            can_next: false,
            next_sha: None,
            prev_sha,
        }
    }
}

/// Creates a `TimeTravelState` at the specified commit index.
fn state_at_index(snapshot: CommitSnapshot, history: Vec<String>, index: usize) -> TimeTravelState {
    TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: "src/auth.rs".to_owned(),
        original_line: None,
        line_mapping: None,
        commit_history: history,
        current_index: index,
    })
}

/// Asserts all navigation-related properties of a `TimeTravelState`.
fn assert_navigation(state: &TimeTravelState, expected: &ExpectedNavigation<'_>) {
    assert_eq!(state.can_go_previous(), expected.can_previous);
    assert_eq!(state.can_go_next(), expected.can_next);
    assert_eq!(state.next_commit_sha(), expected.next_sha);
    assert_eq!(state.previous_commit_sha(), expected.prev_sha);
}

#[fixture]
fn sample_snapshot() -> CommitSnapshot {
    let metadata = CommitMetadata::new(
        "abc1234567890".to_owned(),
        "Fix login bug".to_owned(),
        "Alice".to_owned(),
        Utc::now(),
    );
    CommitSnapshot::with_file_content(
        metadata,
        "src/auth.rs".to_owned(),
        "fn login() {}".to_owned(),
    )
}

#[fixture]
fn sample_history() -> Vec<String> {
    vec![
        "abc1234567890".to_owned(),
        "def5678901234".to_owned(),
        "ghi9012345678".to_owned(),
    ]
}

#[rstest]
fn new_state_initialised(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
    let state = TimeTravelState::new(TimeTravelInitParams {
        snapshot: sample_snapshot.clone(),
        file_path: "src/auth.rs".to_owned(),
        original_line: Some(42),
        line_mapping: None,
        commit_history: sample_history.clone(),
        current_index: 0,
    });

    assert_eq!(state.snapshot().sha(), sample_snapshot.sha());
    assert_eq!(state.file_path(), "src/auth.rs");
    assert_eq!(state.original_line(), Some(42));
    assert!(state.line_mapping().is_none());
    assert_eq!(state.commit_count(), 3);
    assert_eq!(state.current_index(), 0);
    assert!(!state.is_loading());
    assert!(state.error_message().is_none());
}

#[rstest]
fn loading_state() {
    let state = TimeTravelState::loading("src/main.rs".to_owned(), Some(10));

    assert!(state.is_loading());
    assert_eq!(state.snapshot().message(), "Loading...");
    assert_eq!(state.file_path(), "src/main.rs");
    assert_eq!(state.original_line(), Some(10));
}

#[rstest]
fn error_state() {
    let state = TimeTravelState::error("Commit not found".to_owned(), "src/lib.rs".to_owned());

    assert!(!state.is_loading());
    assert_eq!(state.error_message(), Some("Commit not found"));
}

#[rstest]
fn navigation_available(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
    let state = state_at_index(sample_snapshot, sample_history, 0);

    // At index 0 (most recent): can go previous, cannot go next
    assert_navigation(&state, &ExpectedNavigation::at_newest("def5678901234"));
}

#[rstest]
fn navigation_at_middle(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
    let state = state_at_index(sample_snapshot, sample_history, 1);

    // At index 1 (middle): can go both ways
    assert_navigation(
        &state,
        &ExpectedNavigation::at_middle("abc1234567890", "ghi9012345678"),
    );
}

#[rstest]
fn navigation_at_oldest(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
    let state = state_at_index(sample_snapshot, sample_history, 2);

    // At index 2 (oldest): cannot go previous, can go next
    assert_navigation(&state, &ExpectedNavigation::at_oldest("def5678901234"));
}

#[rstest]
fn loading_blocks_navigation(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
    let mut state = state_at_index(sample_snapshot, sample_history, 0);
    state.set_loading(true);

    assert_navigation(&state, &ExpectedNavigation::blocked(Some("def5678901234")));
}

#[rstest]
fn update_snapshot_clamps_index(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
    let mut state = TimeTravelState::new(TimeTravelInitParams {
        snapshot: sample_snapshot.clone(),
        file_path: "src/auth.rs".to_owned(),
        original_line: None,
        line_mapping: None,
        commit_history: sample_history,
        current_index: 0,
    });

    // Try to update with an out-of-bounds index
    state.update_snapshot(sample_snapshot, None, 100);

    assert_eq!(state.current_index(), 2); // Clamped to last index
}

#[rstest]
fn params_from_comment_full() {
    let comment = ReviewComment {
        commit_sha: Some("abc123".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(42),
        original_line_number: Some(40),
        ..minimal_review(1, "Test comment", "alice")
    };

    let params = TimeTravelParams::from_comment(&comment).unwrap();

    assert_eq!(params.commit_sha, "abc123");
    assert_eq!(params.file_path, "src/main.rs");
    assert_eq!(params.line_number, Some(42)); // Prefers line_number
}

#[rstest]
fn params_from_comment_original_line() {
    let comment = ReviewComment {
        commit_sha: Some("abc123".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: None,
        original_line_number: Some(40),
        ..minimal_review(1, "Test comment", "alice")
    };

    let params = TimeTravelParams::from_comment(&comment).unwrap();

    assert_eq!(params.line_number, Some(40)); // Falls back to original_line_number
}

#[rstest]
fn params_from_comment_missing_sha() {
    let comment = ReviewComment {
        commit_sha: None,
        file_path: Some("src/main.rs".to_owned()),
        ..minimal_review(1, "Test comment", "alice")
    };

    assert!(TimeTravelParams::from_comment(&comment).is_none());
}

#[rstest]
fn params_from_comment_missing_path() {
    let comment = ReviewComment {
        commit_sha: Some("abc123".to_owned()),
        file_path: None,
        ..minimal_review(1, "Test comment", "alice")
    };

    assert!(TimeTravelParams::from_comment(&comment).is_none());
}

#[rstest]
fn line_mapping_stored(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
    let mapping = LineMappingVerification::moved(42, 50);
    let state = TimeTravelState::new(TimeTravelInitParams {
        snapshot: sample_snapshot,
        file_path: "src/auth.rs".to_owned(),
        original_line: Some(42),
        line_mapping: Some(mapping.clone()),
        commit_history: sample_history,
        current_index: 0,
    });

    let stored = state.line_mapping().unwrap();
    assert_eq!(stored.status(), LineMappingStatus::Moved);
    assert_eq!(stored.original_line(), 42);
    assert_eq!(stored.current_line(), Some(50));
}

#[test]
fn clamp_index_empty() {
    assert_eq!(clamp_index(0, 0), 0);
    assert_eq!(clamp_index(5, 0), 0);
}

#[test]
fn clamp_index_normal() {
    assert_eq!(clamp_index(0, 3), 0);
    assert_eq!(clamp_index(2, 3), 2);
    assert_eq!(clamp_index(5, 3), 2);
}
