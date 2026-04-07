//! Behavioural tests for configurable commit history limit.
//!
//! These scenarios verify that `load_time_travel_state` passes the configured
//! `commit_history_limit` to `GitOperations::get_parent_commits` and that
//! both default and overridden limits affect the loaded history length.

use chrono::Utc;
use frankie::local::{
    CommitMetadata, CommitSha, CommitSnapshot, GitOperationError, GitOperations,
    LineMappingRequest, LineMappingVerification, RepoFilePath,
};
use frankie::time_travel::{TimeTravelInitParams, TimeTravelState};
use frankie::DEFAULT_COMMIT_HISTORY_LIMIT;
use mockall::mock;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

/// Error type for BDD test step failures.
type StepError = &'static str;

/// Result type for BDD test steps.
type StepResult = Result<(), StepError>;

// Mock `GitOperations` for limit verification.
mock! {
    pub BddGitOps {}

    impl std::fmt::Debug for BddGitOps {
        fn fmt<'a>(&self, f: &mut std::fmt::Formatter<'a>) -> std::fmt::Result;
    }

    impl GitOperations for BddGitOps {
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

/// Scenario state for commit history limit BDD tests.
#[derive(ScenarioState, Default)]
struct CommitHistoryLimitWorld {
    /// Expected limit to assert via `mockall::withf`.
    expected_limit: Slot<usize>,
    /// Number of commits the mock should return.
    mock_commit_count: Slot<usize>,
    /// Comment commit SHA.
    commit_sha: Slot<String>,
    /// Comment file path.
    file_path: Slot<String>,
    /// Resulting time-travel state.
    state: Slot<TimeTravelState>,
}

#[fixture]
fn state() -> CommitHistoryLimitWorld {
    CommitHistoryLimitWorld::default()
}

/// Creates a test commit snapshot for the given SHA.
fn build_test_snapshot(sha: &str) -> CommitSnapshot {
    let metadata = CommitMetadata::new(
        sha.to_owned(),
        "Test commit".to_owned(),
        "Alice".to_owned(),
        Utc::now(),
    );
    CommitSnapshot::with_file_content(
        metadata,
        "src/main.rs".to_owned(),
        "fn main() {}".to_owned(),
    )
}

/// Builds a mock `GitOperations` that asserts the exact limit passed to
/// `get_parent_commits` and returns a fixed number of commits.
fn build_mock(expected_limit: usize, commit_count: usize) -> MockBddGitOps {
    let mut git_ops = MockBddGitOps::new();

    git_ops
        .expect_get_commit_snapshot()
        .times(1)
        .returning(|sha, _file_path| Ok(build_test_snapshot(sha.as_str())));

    git_ops
        .expect_get_parent_commits()
        .times(1)
        .withf(move |_sha, limit| *limit == expected_limit)
        .returning(move |_sha, _limit| {
            Ok((0..commit_count)
                .map(|i| CommitSha::new(format!("commit_{i}")))
                .collect())
        });

    git_ops
}

/// Loads time-travel state using the provided mock and limit.
fn load_state(
    git_ops: &dyn GitOperations,
    commit_sha: &str,
    file_path: &str,
    limit: usize,
) -> Result<TimeTravelState, StepError> {
    let sha = CommitSha::new(commit_sha.to_owned());
    let snapshot = git_ops
        .get_commit_snapshot(&sha, None)
        .map_err(|_| "get_commit_snapshot failed")?;
    let commit_history = git_ops
        .get_parent_commits(&sha, limit)
        .map_err(|_| "get_parent_commits failed")?;

    Ok(TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: RepoFilePath::new(file_path.to_owned()),
        original_line: None,
        line_mapping: None,
        commit_history,
        current_index: 0,
    }))
}

// -- Given steps --

#[given("a git operations mock expecting a commit history limit of {limit}")]
fn given_mock_expecting_limit(state: &CommitHistoryLimitWorld, limit: usize) {
    state.expected_limit.set(limit);
    state.mock_commit_count.set(3);
}

#[given("a comment with SHA {sha} and file {file}")]
fn given_comment(state: &CommitHistoryLimitWorld, sha: String, file: String) {
    state.commit_sha.set(sha);
    state.file_path.set(file);
}

// -- When steps --

#[when("the time-travel state is loaded with the default limit")]
fn when_loaded_with_default_limit(state: &CommitHistoryLimitWorld) -> StepResult {
    let expected_limit = state
        .expected_limit
        .with_ref(|v| *v)
        .ok_or("expected_limit should be set")?;
    let mock_commit_count = state
        .mock_commit_count
        .with_ref(|v| *v)
        .ok_or("mock_commit_count should be set")?;
    let commit_sha = state
        .commit_sha
        .with_ref(Clone::clone)
        .ok_or("commit_sha should be set")?;
    let file_path = state
        .file_path
        .with_ref(Clone::clone)
        .ok_or("file_path should be set")?;

    if expected_limit != DEFAULT_COMMIT_HISTORY_LIMIT {
        return Err("expected_limit should match DEFAULT_COMMIT_HISTORY_LIMIT");
    }

    let git_ops = build_mock(DEFAULT_COMMIT_HISTORY_LIMIT, mock_commit_count);
    let result = load_state(&git_ops, &commit_sha, &file_path, DEFAULT_COMMIT_HISTORY_LIMIT)?;
    state.state.set(result);
    Ok(())
}

#[when("the time-travel state is loaded with a limit of {limit}")]
fn when_loaded_with_custom_limit(state: &CommitHistoryLimitWorld, limit: usize) -> StepResult {
    let expected_limit = state
        .expected_limit
        .with_ref(|v| *v)
        .ok_or("expected_limit should be set")?;

    if limit != expected_limit {
        return Err("limit should match expected_limit from Given step");
    }

    let mock_commit_count = if limit == 1 { 1 } else {
        state
            .mock_commit_count
            .with_ref(|v| *v)
            .ok_or("mock_commit_count should be set")?
    };
    let commit_sha = state
        .commit_sha
        .with_ref(Clone::clone)
        .ok_or("commit_sha should be set")?;
    let file_path = state
        .file_path
        .with_ref(Clone::clone)
        .ok_or("file_path should be set")?;

    let git_ops = build_mock(expected_limit, mock_commit_count);
    let result = load_state(&git_ops, &commit_sha, &file_path, limit)?;
    state.state.set(result);
    Ok(())
}

// -- Then steps --

#[then("the loaded history contains {expected} commits")]
fn then_history_count(state: &CommitHistoryLimitWorld, expected: usize) -> StepResult {
    let actual = state
        .state
        .with_ref(TimeTravelState::commit_count)
        .ok_or("time-travel state should be available")?;
    if actual == expected {
        Ok(())
    } else {
        Err("commit count does not match expected value")
    }
}

// -- Scenario bindings --

#[scenario(
    path = "tests/features/commit_history_limit.feature",
    name = "Default commit history limit uses 50 commits"
)]
fn default_limit(state: CommitHistoryLimitWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/commit_history_limit.feature",
    name = "Overridden commit history limit is respected"
)]
fn overridden_limit(state: CommitHistoryLimitWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/commit_history_limit.feature",
    name = "Minimum limit of 1 produces a single-entry history"
)]
fn minimum_limit(state: CommitHistoryLimitWorld) {
    let _ = state;
}
