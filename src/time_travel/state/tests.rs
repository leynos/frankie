//! Unit tests for time-travel state management.
//!
//! These tests verify the `TimeTravelState` struct's navigation logic,
//! loading/error states, and index clamping. Parameter extraction from
//! review comments is tested in `crate::time_travel::tests`.

use chrono::Utc;
use rstest::{fixture, rstest};

use super::*;
use crate::local::LineMappingStatus;

/// Structured error for fallible test helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StepError(String);

impl StepError {
    /// Creates a new helper error with the given message.
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Expected navigation properties for test assertions.
#[derive(Debug, Clone)]
struct ExpectedNavigation {
    can_previous: bool,
    can_next: bool,
    next_sha: Option<CommitSha>,
    prev_sha: Option<CommitSha>,
}

impl ExpectedNavigation {
    /// Returns navigation state at index 0 (can go previous, cannot go next).
    fn at_newest(prev_sha: &str) -> Self {
        Self {
            can_previous: true,
            can_next: false,
            next_sha: None,
            prev_sha: Some(CommitSha::new(prev_sha.to_owned())),
        }
    }

    /// Returns navigation state in the middle (can go both ways).
    fn at_middle(next_sha: &str, prev_sha: &str) -> Self {
        Self {
            can_previous: true,
            can_next: true,
            next_sha: Some(CommitSha::new(next_sha.to_owned())),
            prev_sha: Some(CommitSha::new(prev_sha.to_owned())),
        }
    }

    /// Returns navigation state at last index (cannot go previous, can go next).
    fn at_oldest(next_sha: &str) -> Self {
        Self {
            can_previous: false,
            can_next: true,
            next_sha: Some(CommitSha::new(next_sha.to_owned())),
            prev_sha: None,
        }
    }

    /// Returns navigation state when loading (cannot navigate either way).
    fn blocked(prev_sha: Option<&str>) -> Self {
        Self {
            can_previous: false,
            can_next: false,
            next_sha: None,
            prev_sha: prev_sha.map(|s| CommitSha::new(s.to_owned())),
        }
    }
}

/// Creates a `TimeTravelState` at the specified commit index.
fn state_at_index(
    snapshot: &CommitSnapshot,
    history: Vec<CommitSha>,
    index: usize,
) -> Result<TimeTravelState, StepError> {
    let file_path = snapshot
        .file_path()
        .ok_or_else(|| StepError::new("test snapshots should include a file path"))?
        .to_owned();
    let file_content = snapshot
        .file_content()
        .ok_or_else(|| StepError::new("test snapshots should include file content"))?
        .to_owned();
    let snapshot_sha = history
        .get(index)
        .ok_or_else(|| StepError::new("test index should be within commit history bounds"))?
        .as_str()
        .to_owned();
    let metadata = CommitMetadata::new(
        snapshot_sha,
        format!("Commit {index}"),
        "Alice".to_owned(),
        Utc::now(),
    );
    let indexed_snapshot =
        CommitSnapshot::with_file_content(metadata, file_path.clone(), file_content);

    Ok(TimeTravelState::new(TimeTravelInitParams {
        snapshot: indexed_snapshot,
        file_path: RepoFilePath::new(file_path),
        original_line: None,
        line_mapping: None,
        commit_history: history,
        current_index: index,
    }))
}

/// Constructs a `TimeTravelState` from `sample_snapshot` with standard test
/// defaults (`file_path = "src/auth.rs"`, no original line, no line mapping).
fn default_state(
    snapshot: CommitSnapshot,
    history: Vec<CommitSha>,
    index: usize,
) -> TimeTravelState {
    TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: RepoFilePath::new("src/auth.rs".to_owned()),
        original_line: None,
        line_mapping: None,
        commit_history: history,
        current_index: index,
    })
}

/// Asserts all navigation-related properties of a `TimeTravelState`.
fn assert_navigation(state: &TimeTravelState, expected: &ExpectedNavigation) {
    assert_eq!(state.can_go_previous(), expected.can_previous);
    assert_eq!(state.can_go_next(), expected.can_next);
    assert_eq!(state.next_commit_sha(), expected.next_sha.as_ref());
    assert_eq!(state.previous_commit_sha(), expected.prev_sha.as_ref());
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
fn sample_history() -> Vec<CommitSha> {
    vec![
        CommitSha::new("abc1234567890".to_owned()),
        CommitSha::new("def5678901234".to_owned()),
        CommitSha::new("ghi9012345678".to_owned()),
    ]
}

#[rstest]
fn new_state_stores_snapshot_metadata(
    sample_snapshot: CommitSnapshot,
    sample_history: Vec<CommitSha>,
) {
    let state = TimeTravelState::new(TimeTravelInitParams {
        snapshot: sample_snapshot.clone(),
        file_path: RepoFilePath::new("src/auth.rs".to_owned()),
        original_line: Some(42),
        line_mapping: None,
        commit_history: sample_history,
        current_index: 0,
    });

    assert_eq!(state.snapshot().sha(), sample_snapshot.sha());
    assert_eq!(state.file_path().as_str(), "src/auth.rs");
    assert_eq!(state.original_line(), Some(42));
    assert!(state.line_mapping().is_none());
}

#[rstest]
fn new_state_stores_history_index(sample_snapshot: CommitSnapshot, sample_history: Vec<CommitSha>) {
    let state = default_state(sample_snapshot, sample_history, 0);

    assert_eq!(state.commit_count(), 3);
    assert_eq!(state.current_index(), 0);
}

#[rstest]
fn new_state_is_not_loading_or_error(
    sample_snapshot: CommitSnapshot,
    sample_history: Vec<CommitSha>,
) {
    let state = default_state(sample_snapshot, sample_history, 0);

    assert!(!state.is_loading());
    assert!(state.error_message().is_none());
}

#[rstest]
fn new_state_aligns_index_with_snapshot_sha(
    sample_snapshot: CommitSnapshot,
    sample_history: Vec<CommitSha>,
) {
    let state = default_state(sample_snapshot, sample_history, 99);

    assert_eq!(state.current_index(), 0);
    assert_navigation(&state, &ExpectedNavigation::at_newest("def5678901234"));
}

#[rstest]
fn loading_state() {
    let state = TimeTravelState::loading(RepoFilePath::new("src/main.rs".to_owned()), Some(10));

    assert!(state.is_loading());
    assert_eq!(state.snapshot().message(), "Loading...");
    assert_eq!(state.file_path().as_str(), "src/main.rs");
    assert_eq!(state.original_line(), Some(10));
}

#[rstest]
fn error_state() {
    let state = TimeTravelState::error(
        "Commit not found".to_owned(),
        RepoFilePath::new("src/lib.rs".to_owned()),
    );

    assert!(!state.is_loading());
    assert_eq!(state.error_message(), Some("Commit not found"));
}

#[rstest]
#[case(0, ExpectedNavigation::at_newest("def5678901234"))]
#[case(1, ExpectedNavigation::at_middle("abc1234567890", "ghi9012345678"))]
#[case(2, ExpectedNavigation::at_oldest("def5678901234"))]
fn navigation_states(
    sample_snapshot: CommitSnapshot,
    sample_history: Vec<CommitSha>,
    #[case] index: usize,
    #[case] expected_navigation: ExpectedNavigation,
) -> Result<(), StepError> {
    let state = state_at_index(&sample_snapshot, sample_history, index)?;

    assert_navigation(&state, &expected_navigation);
    Ok(())
}

#[rstest]
fn loading_blocks_navigation(
    sample_snapshot: CommitSnapshot,
    sample_history: Vec<CommitSha>,
) -> Result<(), StepError> {
    let mut state = state_at_index(&sample_snapshot, sample_history, 0)?;
    state.set_loading(true);

    assert_navigation(&state, &ExpectedNavigation::blocked(Some("def5678901234")));
    Ok(())
}

#[rstest]
fn update_snapshot_syncs_index_to_snapshot_sha(
    sample_snapshot: CommitSnapshot,
    sample_history: Vec<CommitSha>,
) {
    let mut state = TimeTravelState::new(TimeTravelInitParams {
        snapshot: sample_snapshot.clone(),
        file_path: RepoFilePath::new("src/auth.rs".to_owned()),
        original_line: None,
        line_mapping: None,
        commit_history: sample_history,
        current_index: 0,
    });

    let metadata = CommitMetadata::new(
        "ghi9012345678".to_owned(),
        "Initial implementation".to_owned(),
        "Alice".to_owned(),
        Utc::now(),
    );
    let updated_snapshot = CommitSnapshot::with_file_content(
        metadata,
        "src/auth.rs".to_owned(),
        "fn login() {}".to_owned(),
    );

    state.update_snapshot(updated_snapshot, None, 100);

    assert_eq!(state.current_index(), 2);
}

#[rstest]
fn new_state_sets_error_for_snapshot_sha_missing_from_history(sample_history: Vec<CommitSha>) {
    let metadata = CommitMetadata::new(
        "zzz9999999999".to_owned(),
        "Detached snapshot".to_owned(),
        "Alice".to_owned(),
        Utc::now(),
    );
    let snapshot = CommitSnapshot::with_file_content(
        metadata,
        "src/auth.rs".to_owned(),
        "fn login() {}".to_owned(),
    );
    let state = TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: RepoFilePath::new("src/auth.rs".to_owned()),
        original_line: None,
        line_mapping: None,
        commit_history: sample_history,
        current_index: 1,
    });

    assert_eq!(state.current_index(), 1);
    assert_eq!(
        state.error_message(),
        Some(
            "snapshot SHA zzz9999999999 does not match commit history entry def5678901234 at index 1"
        )
    );
}

#[rstest]
fn line_mapping_stored(sample_snapshot: CommitSnapshot, sample_history: Vec<CommitSha>) {
    let mapping = LineMappingVerification::moved(42, 50);
    let state = TimeTravelState::new(TimeTravelInitParams {
        snapshot: sample_snapshot,
        file_path: RepoFilePath::new("src/auth.rs".to_owned()),
        original_line: Some(42),
        line_mapping: Some(mapping.clone()),
        commit_history: sample_history,
        current_index: 0,
    });

    let stored = state
        .line_mapping()
        .expect("line mapping should be stored in state");
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
