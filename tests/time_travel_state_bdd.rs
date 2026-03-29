//! Behavioural tests for the public time-travel state API.
//!
//! These scenarios import `frankie::time_travel::{TimeTravelInitParams,
//! TimeTravelState}` directly to prove that callers outside `crate::tui` can
//! construct and inspect time-travel state.

use chrono::Utc;
use frankie::local::{
    CommitMetadata, CommitSha, CommitSnapshot, LineMappingVerification, RepoFilePath,
};
use frankie::time_travel::{TimeTravelInitParams, TimeTravelState};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

/// Error type for BDD test step failures.
type StepError = &'static str;

/// Result type for BDD test steps.
type StepResult = Result<(), StepError>;

/// Scenario state for public time-travel state API tests.
#[derive(ScenarioState, Default)]
struct TimeTravelStateWorld {
    /// Snapshot metadata SHA.
    snapshot_sha: Slot<String>,
    /// Snapshot metadata message.
    snapshot_message: Slot<String>,
    /// File content stored in the snapshot.
    file_content: Slot<String>,
    /// File path stored in the state.
    file_path: Slot<String>,
    /// Original line to highlight.
    original_line: Slot<Option<u32>>,
    /// Line mapping stored in the state.
    line_mapping: Slot<Option<LineMappingVerification>>,
    /// Commit history used for navigation.
    commit_history: Slot<Vec<CommitSha>>,
    /// Current history index.
    current_index: Slot<usize>,
    /// Constructed state under test.
    state: Slot<TimeTravelState>,
}

#[fixture]
fn state() -> TimeTravelStateWorld {
    TimeTravelStateWorld::default()
}

// -- Given steps --

#[given("a snapshot for commit SHA {sha} with message {message}")]
fn given_snapshot_metadata(state: &TimeTravelStateWorld, sha: String, message: String) {
    state.snapshot_sha.set(sha);
    state.snapshot_message.set(message);
}

#[given("the snapshot contains file content {content}")]
fn given_snapshot_content(state: &TimeTravelStateWorld, content: String) {
    state.file_content.set(content);
}

#[given("the file path is {path}")]
fn given_file_path(state: &TimeTravelStateWorld, path: String) {
    state.file_path.set(path);
}

#[given("the original line is {line}")]
fn given_original_line(state: &TimeTravelStateWorld, line: u32) {
    state.original_line.set(Some(line));
}

#[given("there is no original line")]
fn given_no_original_line(state: &TimeTravelStateWorld) {
    state.original_line.set(None);
}

#[given("the line mapping is an exact match for line {line}")]
fn given_exact_line_mapping(state: &TimeTravelStateWorld, line: u32) {
    state
        .line_mapping
        .set(Some(LineMappingVerification::exact(line)));
}

#[given("there is no line mapping")]
fn given_no_line_mapping(state: &TimeTravelStateWorld) {
    state.line_mapping.set(None);
}

#[given("the commit history is {first}, {second}, and {third}")]
fn given_commit_history(
    state: &TimeTravelStateWorld,
    first: String,
    second: String,
    third: String,
) {
    state.commit_history.set(vec![
        CommitSha::new(first),
        CommitSha::new(second),
        CommitSha::new(third),
    ]);
}

#[given("the current history index is {index}")]
fn given_current_index(state: &TimeTravelStateWorld, index: usize) {
    state.current_index.set(index);
}

// -- When steps --

#[when("the time-travel state is constructed")]
fn when_state_is_constructed(state: &TimeTravelStateWorld) -> StepResult {
    let snapshot_sha = state
        .snapshot_sha
        .with_ref(Clone::clone)
        .ok_or("snapshot SHA should be set before construction")?;
    let snapshot_message = state
        .snapshot_message
        .with_ref(Clone::clone)
        .ok_or("snapshot message should be set before construction")?;
    let file_content = state
        .file_content
        .with_ref(Clone::clone)
        .ok_or("file content should be set before construction")?;
    let file_path = state
        .file_path
        .with_ref(Clone::clone)
        .ok_or("file path should be set before construction")?;
    let original_line = state.original_line.with_ref(|line| *line).unwrap_or(None);
    let line_mapping = state.line_mapping.with_ref(Clone::clone).flatten();
    let commit_history = state
        .commit_history
        .with_ref(Clone::clone)
        .ok_or("commit history should be set before construction")?;
    let current_index = state
        .current_index
        .with_ref(|index| *index)
        .ok_or("current index should be set before construction")?;

    let metadata = CommitMetadata::new(
        snapshot_sha,
        snapshot_message,
        "Alice".to_owned(),
        Utc::now(),
    );
    let snapshot = CommitSnapshot::with_file_content(metadata, file_path.clone(), file_content);

    state.state.set(TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: RepoFilePath::new(file_path),
        original_line,
        line_mapping,
        commit_history,
        current_index,
    }));
    Ok(())
}

#[when("the snapshot is updated to SHA {sha} with message {message} and index {index}")]
fn when_snapshot_is_updated(
    state: &TimeTravelStateWorld,
    sha: String,
    message: String,
    index: usize,
) -> StepResult {
    let file_path = state
        .file_path
        .with_ref(Clone::clone)
        .ok_or("file path should be set before update")?;
    let snapshot_content = state
        .file_content
        .with_ref(Clone::clone)
        .ok_or("file content should be set before update")?;
    let updated_metadata = CommitMetadata::new(sha, message, "Alice".to_owned(), Utc::now());
    let updated_snapshot =
        CommitSnapshot::with_file_content(updated_metadata, file_path, snapshot_content);

    state
        .state
        .with_mut(|time_travel_state| {
            time_travel_state.update_snapshot(updated_snapshot, None, index);
        })
        .ok_or("state should be constructed before update")?;
    Ok(())
}

fn with_state<T>(
    state: &TimeTravelStateWorld,
    extract: impl FnOnce(&TimeTravelState) -> T,
) -> Result<T, StepError> {
    state
        .state
        .with_ref(extract)
        .ok_or("time-travel state should be available")
}

fn check_eq<T: PartialEq>(actual: &T, expected: &T, msg: &'static str) -> StepResult {
    if actual == expected { Ok(()) } else { Err(msg) }
}

// -- Then steps --

#[then("the snapshot SHA is {expected}")]
fn then_snapshot_sha(state: &TimeTravelStateWorld, expected: String) -> StepResult {
    let actual = with_state(state, |s| s.snapshot().sha().to_owned())?;
    check_eq(&actual, &expected, "snapshot SHA does not match")
}

#[then("the snapshot message is {expected}")]
fn then_snapshot_message(state: &TimeTravelStateWorld, expected: String) -> StepResult {
    let actual = with_state(state, |s| s.snapshot().message().to_owned())?;
    check_eq(&actual, &expected, "snapshot message does not match")
}

#[then("the public file path is {expected}")]
fn then_public_file_path(state: &TimeTravelStateWorld, expected: String) -> StepResult {
    let actual = with_state(state, |s| s.file_path().as_str().to_owned())?;
    check_eq(&actual, &expected, "file path does not match")
}

#[then("the public original line is {expected}")]
fn then_public_original_line(state: &TimeTravelStateWorld, expected: u32) -> StepResult {
    let actual = with_state(state, TimeTravelState::original_line)?;
    if actual == Some(expected) {
        Ok(())
    } else {
        Err("original line does not match")
    }
}

#[then("the state reports {expected} commits in history")]
fn then_commit_count(state: &TimeTravelStateWorld, expected: usize) -> StepResult {
    let actual = with_state(state, TimeTravelState::commit_count)?;
    if actual == expected {
        Ok(())
    } else {
        Err("commit count does not match")
    }
}

#[then("the current index is {expected}")]
fn then_current_index(state: &TimeTravelStateWorld, expected: usize) -> StepResult {
    let actual = with_state(state, TimeTravelState::current_index)?;
    if actual == expected {
        Ok(())
    } else {
        Err("current index does not match")
    }
}

#[then("previous navigation is available")]
fn then_previous_navigation_available(state: &TimeTravelStateWorld) -> StepResult {
    let actual = with_state(state, TimeTravelState::can_go_previous)?;
    if actual {
        Ok(())
    } else {
        Err("previous navigation should be available")
    }
}

#[then("next navigation is unavailable")]
fn then_next_navigation_unavailable(state: &TimeTravelStateWorld) -> StepResult {
    let actual = with_state(state, TimeTravelState::can_go_next)?;
    if actual {
        Err("next navigation should be unavailable")
    } else {
        Ok(())
    }
}

#[then("the previous commit SHA is {expected}")]
fn then_previous_commit_sha(state: &TimeTravelStateWorld, expected: String) -> StepResult {
    let actual = with_state(state, |s| {
        s.previous_commit_sha().map(|sha| sha.as_str().to_owned())
    })?;
    let expected_sha = Some(expected);
    check_eq(&actual, &expected_sha, "previous commit SHA does not match")
}

#[then("the next commit SHA is {expected}")]
fn then_next_commit_sha(state: &TimeTravelStateWorld, expected: String) -> StepResult {
    let actual = with_state(state, |s| {
        s.next_commit_sha().map(|sha| sha.as_str().to_owned())
    })?;
    let expected_sha = Some(expected);
    check_eq(&actual, &expected_sha, "next commit SHA does not match")
}

#[then("no next commit SHA is available")]
fn then_next_commit_sha_absent(state: &TimeTravelStateWorld) -> StepResult {
    let is_absent = state
        .state
        .with_ref(|time_travel_state| time_travel_state.next_commit_sha().is_none())
        .ok_or("time-travel state should be available")?;
    if is_absent {
        Ok(())
    } else {
        Err("next commit SHA should be absent")
    }
}

#[then("the state exposes an exact line mapping for line {expected}")]
fn then_exact_line_mapping(state: &TimeTravelStateWorld, expected: u32) -> StepResult {
    let actual = with_state(state, |s| {
        s.line_mapping().map(LineMappingVerification::original_line)
    })?;
    let expected_line = Some(expected);
    check_eq(&actual, &expected_line, "line mapping does not match")
}

// -- Scenario bindings --

#[scenario(
    path = "tests/features/time_travel_state.feature",
    name = "Construct a time-travel state and read public accessors"
)]
fn construct_and_read_public_accessors(state: TimeTravelStateWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_state.feature",
    name = "Inspect navigation from the middle of commit history"
)]
fn inspect_middle_navigation(state: TimeTravelStateWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_state.feature",
    name = "Update a snapshot and clamp the requested index"
)]
fn update_snapshot_and_clamp(state: TimeTravelStateWorld) {
    let _ = state;
}
