//! Behavioural tests for public time-travel orchestration services.
//!
//! These scenarios exercise the shared `frankie::time_travel` load and
//! navigation APIs without importing `frankie::tui`.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use chrono::Utc;
use frankie::local::{
    CommitMetadata, CommitSha, CommitSnapshot, GitOperationError, GitOperations,
    LineMappingRequest, LineMappingVerification, RepoFilePath,
};
use frankie::time_travel::{
    TimeTravelInitParams, TimeTravelNavigationDirection, TimeTravelParams, TimeTravelState,
    load_time_travel_state, navigate_time_travel_state,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

type StepError = &'static str;
type StepResult = Result<(), StepError>;

#[derive(Debug, Default)]
struct ScriptedGitOps {
    snapshots: Mutex<HashMap<String, Result<CommitSnapshot, GitOperationError>>>,
    commit_history: Mutex<Vec<CommitSha>>,
    line_mapping: Mutex<Option<LineMappingVerification>>,
    snapshot_requests: Mutex<Vec<String>>,
}

impl ScriptedGitOps {
    fn with_snapshot(self, sha: &str, snapshot: CommitSnapshot) -> Self {
        lock_or_recover(&self.snapshots).insert(sha.to_owned(), Ok(snapshot));
        self
    }

    fn with_snapshot_error(self, sha: &str, error: GitOperationError) -> Self {
        lock_or_recover(&self.snapshots).insert(sha.to_owned(), Err(error));
        self
    }

    fn with_commit_history(self, commit_history: Vec<CommitSha>) -> Self {
        *lock_or_recover(&self.commit_history) = commit_history;
        self
    }

    fn with_line_mapping(self, line_mapping: Option<LineMappingVerification>) -> Self {
        *lock_or_recover(&self.line_mapping) = line_mapping;
        self
    }

    fn requested_snapshot_count(&self) -> Result<usize, StepError> {
        self.snapshot_requests
            .lock()
            .map(|requests| requests.len())
            .map_err(|_| "snapshot request mutex should lock")
    }
}

impl GitOperations for ScriptedGitOps {
    fn get_commit_snapshot(
        &self,
        sha: &CommitSha,
        _file_path: Option<&RepoFilePath>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        lock_or_recover(&self.snapshot_requests).push(sha.as_str().to_owned());
        lock_or_recover(&self.snapshots)
            .get(sha.as_str())
            .cloned()
            .unwrap_or_else(|| Err(GitOperationError::CommitNotFound { sha: sha.clone() }))
    }

    fn get_file_at_commit(
        &self,
        sha: &CommitSha,
        _file_path: &RepoFilePath,
    ) -> Result<String, GitOperationError> {
        Err(GitOperationError::CommitAccessFailed {
            sha: sha.clone(),
            message: "not used in time-travel orchestration tests".to_owned(),
        })
    }

    fn verify_line_mapping(
        &self,
        request: &LineMappingRequest,
    ) -> Result<LineMappingVerification, GitOperationError> {
        Ok(lock_or_recover(&self.line_mapping)
            .clone()
            .unwrap_or_else(|| LineMappingVerification::exact(request.line)))
    }

    fn get_parent_commits(
        &self,
        _sha: &CommitSha,
        limit: usize,
    ) -> Result<Vec<CommitSha>, GitOperationError> {
        Ok(lock_or_recover(&self.commit_history)
            .iter()
            .take(limit)
            .cloned()
            .collect())
    }

    fn commit_exists(&self, _sha: &CommitSha) -> bool {
        true
    }
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(ScenarioState, Default)]
struct TimeTravelOrchestrationWorld {
    commit_sha: Slot<String>,
    file_path: Slot<String>,
    line_number: Slot<Option<u32>>,
    head_sha: Slot<Option<CommitSha>>,
    commit_history_limit: Slot<usize>,
    git_ops: Slot<ScriptedGitOps>,
    load_result: Slot<Result<TimeTravelState, GitOperationError>>,
    navigation_result: Slot<Result<Option<TimeTravelState>, GitOperationError>>,
    loaded_state: Slot<TimeTravelState>,
}

#[fixture]
fn state() -> TimeTravelOrchestrationWorld {
    TimeTravelOrchestrationWorld::default()
}

fn build_snapshot(sha: &str, message: &str) -> CommitSnapshot {
    let metadata = CommitMetadata::new(
        sha.to_owned(),
        message.to_owned(),
        "Alice".to_owned(),
        Utc::now(),
    );
    CommitSnapshot::with_file_content(
        metadata,
        "src/main.rs".to_owned(),
        "fn main() {}".to_owned(),
    )
}

fn build_loaded_state(
    index: usize,
    original_line: Option<u32>,
) -> Result<TimeTravelState, StepError> {
    let commit_history = vec![
        CommitSha::new("abc1234567890".to_owned()),
        CommitSha::new("def5678901234".to_owned()),
        CommitSha::new("ghi9012345678".to_owned()),
    ];
    let snapshot_sha = commit_history
        .get(index)
        .ok_or("requested time-travel index should exist")?
        .as_str()
        .to_owned();

    Ok(TimeTravelState::new(TimeTravelInitParams {
        snapshot: build_snapshot(&snapshot_sha, "Loaded state"),
        file_path: RepoFilePath::new("src/main.rs".to_owned()),
        original_line,
        line_mapping: Some(LineMappingVerification::exact(10)),
        commit_history,
        current_index: index,
    }))
}

fn read_navigation_state<T>(
    state: &TimeTravelOrchestrationWorld,
    map: impl FnOnce(&TimeTravelState) -> T,
) -> Result<T, StepError> {
    state
        .navigation_result
        .with_ref(|result| match result {
            Ok(Some(navigation_state)) => Ok(map(navigation_state)),
            Ok(None) => Err("navigation should have returned a state"),
            Err(_) => Err("navigation should have succeeded"),
        })
        .ok_or("navigation_result should be set")?
}

#[given("a time-travel request for commit {sha} and file {file_path}")]
fn given_time_travel_request(state: &TimeTravelOrchestrationWorld, sha: String, file_path: String) {
    state.commit_sha.set(sha);
    state.file_path.set(file_path);
    state.commit_history_limit.set(50);
    state.line_number.set(Some(10));
}

#[given("the requested original line is {line}")]
fn given_requested_original_line(state: &TimeTravelOrchestrationWorld, line: u32) {
    state.line_number.set(Some(line));
}

#[given("the requested original line is absent")]
fn given_requested_original_line_absent(state: &TimeTravelOrchestrationWorld) {
    state.line_number.set(None);
}

#[given("the head SHA is {sha}")]
fn given_head_sha(state: &TimeTravelOrchestrationWorld, sha: String) {
    state.head_sha.set(Some(CommitSha::new(sha)));
}

#[given("no head SHA is available")]
fn given_no_head_sha(state: &TimeTravelOrchestrationWorld) {
    state.head_sha.set(None);
}

#[given("git loads snapshot {sha} with message {message}")]
fn given_git_loads_snapshot(state: &TimeTravelOrchestrationWorld, sha: String, message: String) {
    let git_ops = state
        .git_ops
        .take()
        .unwrap_or_default()
        .with_snapshot(&sha, build_snapshot(&sha, &message));
    state.git_ops.set(git_ops);
}

#[given("git returns commit history {first}, {second}, and {third}")]
fn given_git_returns_commit_history(
    state: &TimeTravelOrchestrationWorld,
    first: String,
    second: String,
    third: String,
) {
    let git_ops = state
        .git_ops
        .take()
        .unwrap_or_default()
        .with_commit_history(vec![
            CommitSha::new(first),
            CommitSha::new(second),
            CommitSha::new(third),
        ]);
    state.git_ops.set(git_ops);
}

#[given("git reports an exact line mapping for line {line}")]
fn given_exact_line_mapping(state: &TimeTravelOrchestrationWorld, line: u32) {
    let git_ops = state
        .git_ops
        .take()
        .unwrap_or_default()
        .with_line_mapping(Some(LineMappingVerification::exact(line)));
    state.git_ops.set(git_ops);
}

#[given("a loaded time-travel state at history index {index}")]
fn given_loaded_time_travel_state(
    state: &TimeTravelOrchestrationWorld,
    index: usize,
) -> StepResult {
    state.loaded_state.set(build_loaded_state(index, Some(10))?);
    Ok(())
}

#[given("a loaded time-travel state without an original line at history index {index}")]
fn given_loaded_time_travel_state_without_original_line(
    state: &TimeTravelOrchestrationWorld,
    index: usize,
) -> StepResult {
    state.loaded_state.set(build_loaded_state(index, None)?);
    Ok(())
}

#[given("git fails to load snapshot {sha} because the commit is missing")]
fn given_git_fails_to_load_snapshot(state: &TimeTravelOrchestrationWorld, sha: String) {
    let error = GitOperationError::CommitNotFound {
        sha: CommitSha::new(sha.clone()),
    };
    let git_ops = state
        .git_ops
        .take()
        .unwrap_or_default()
        .with_snapshot_error(&sha, error);
    state.git_ops.set(git_ops);
}

#[when("the initial time-travel state is loaded")]
fn when_initial_time_travel_state_is_loaded(state: &TimeTravelOrchestrationWorld) -> StepResult {
    let commit_sha = state
        .commit_sha
        .with_ref(Clone::clone)
        .ok_or("commit_sha should be set")?;
    let file_path = state
        .file_path
        .with_ref(Clone::clone)
        .ok_or("file_path should be set")?;
    let line_number = state
        .line_number
        .with_ref(|line_number| *line_number)
        .ok_or("line_number should be set")?;
    let head_sha = state.head_sha.with_ref(Clone::clone).unwrap_or(None);
    let commit_history_limit = state
        .commit_history_limit
        .with_ref(|limit| *limit)
        .ok_or("commit_history_limit should be set")?;

    let params = TimeTravelParams::new(
        CommitSha::new(commit_sha),
        RepoFilePath::new(file_path),
        line_number,
    );
    let result = state
        .git_ops
        .with_ref(|git_ops| {
            load_time_travel_state(git_ops, &params, head_sha.as_ref(), commit_history_limit)
        })
        .ok_or("git_ops should be configured")?;
    state.load_result.set(result);
    Ok(())
}

#[when("the state is navigated to the previous commit")]
fn when_state_navigated_previous(state: &TimeTravelOrchestrationWorld) -> StepResult {
    let loaded_state = state
        .loaded_state
        .with_ref(Clone::clone)
        .ok_or("loaded_state should be set")?;
    let head_sha = state.head_sha.with_ref(Clone::clone).unwrap_or(None);
    let result = state
        .git_ops
        .with_ref(|git_ops| {
            navigate_time_travel_state(
                git_ops,
                &loaded_state,
                TimeTravelNavigationDirection::Previous,
                head_sha.as_ref(),
            )
        })
        .ok_or("git_ops should be configured")?;
    state.navigation_result.set(result);
    Ok(())
}

#[when("the state is navigated to the next commit")]
fn when_state_navigated_next(state: &TimeTravelOrchestrationWorld) -> StepResult {
    let loaded_state = state
        .loaded_state
        .with_ref(Clone::clone)
        .ok_or("loaded_state should be set")?;
    let head_sha = state.head_sha.with_ref(Clone::clone).unwrap_or(None);
    let result = state
        .git_ops
        .with_ref(|git_ops| {
            navigate_time_travel_state(
                git_ops,
                &loaded_state,
                TimeTravelNavigationDirection::Next,
                head_sha.as_ref(),
            )
        })
        .ok_or("git_ops should be configured")?;
    state.navigation_result.set(result);
    Ok(())
}

#[then("the loaded state snapshot SHA is {expected}")]
fn then_loaded_state_snapshot_sha(
    state: &TimeTravelOrchestrationWorld,
    expected: String,
) -> StepResult {
    let snapshot_sha = state
        .load_result
        .with_ref(|result| {
            result.as_ref().map_or(Err(()), |loaded_state| {
                Ok(loaded_state.snapshot().sha().to_owned())
            })
        })
        .ok_or("load_result should be set")?;
    match snapshot_sha {
        Ok(actual_snapshot_sha) if actual_snapshot_sha == expected => Ok(()),
        Ok(_) => Err("loaded state snapshot SHA should match the expected SHA"),
        Err(()) => Err("load should have succeeded"),
    }
}

#[then("the loaded state index is {expected}")]
fn then_loaded_state_index(state: &TimeTravelOrchestrationWorld, expected: usize) -> StepResult {
    let current_index = state
        .load_result
        .with_ref(|result| {
            result
                .as_ref()
                .map_or(Err(()), |loaded_state| Ok(loaded_state.current_index()))
        })
        .ok_or("load_result should be set")?;
    match current_index {
        Ok(actual_index) if actual_index == expected => Ok(()),
        Ok(_) => Err("loaded state index should match the expected index"),
        Err(()) => Err("load should have succeeded"),
    }
}

#[then("the loaded history count is {expected}")]
fn then_loaded_history_count(state: &TimeTravelOrchestrationWorld, expected: usize) -> StepResult {
    let commit_count = state
        .load_result
        .with_ref(|result| {
            result
                .as_ref()
                .map_or(Err(()), |loaded_state| Ok(loaded_state.commit_count()))
        })
        .ok_or("load_result should be set")?;
    match commit_count {
        Ok(actual_commit_count) if actual_commit_count == expected => Ok(()),
        Ok(_) => Err("loaded history count should match the expected value"),
        Err(()) => Err("load should have succeeded"),
    }
}

#[then("the loaded line mapping is exact for line {line}")]
fn then_loaded_line_mapping_is_exact(
    state: &TimeTravelOrchestrationWorld,
    line: u32,
) -> StepResult {
    let line_mapping = state
        .load_result
        .with_ref(|result| {
            result.as_ref().map_or(Err(()), |loaded_state| {
                Ok(loaded_state.line_mapping().map(|line_mapping| {
                    (line_mapping.original_line(), line_mapping.current_line())
                }))
            })
        })
        .ok_or("load_result should be set")?;
    match line_mapping {
        Ok(Some((original_line, current_line)))
            if original_line == line && current_line == Some(line) =>
        {
            Ok(())
        }
        Ok(_) => Err("loaded line mapping should be an exact match"),
        Err(()) => Err("load should have succeeded"),
    }
}

#[then("navigation returns snapshot SHA {expected}")]
fn then_navigation_returns_snapshot_sha(
    state: &TimeTravelOrchestrationWorld,
    expected: String,
) -> StepResult {
    let actual = read_navigation_state(state, |navigation_state| {
        navigation_state.snapshot().sha().to_owned()
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err("navigation snapshot SHA should match the expected value")
    }
}

#[then("navigation returns history index {expected}")]
fn then_navigation_returns_history_index(
    state: &TimeTravelOrchestrationWorld,
    expected: usize,
) -> StepResult {
    let actual = read_navigation_state(state, TimeTravelState::current_index)?;
    if actual == expected {
        Ok(())
    } else {
        Err("navigation history index should match the expected value")
    }
}

#[then("navigation returns no state")]
fn then_navigation_returns_no_state(state: &TimeTravelOrchestrationWorld) -> StepResult {
    let outcome = state
        .navigation_result
        .with_ref(|result| {
            result
                .as_ref()
                .map_or(Err(()), |navigation_state| Ok(navigation_state.is_none()))
        })
        .ok_or("navigation_result should be set")?;
    match outcome {
        Ok(true) => Ok(()),
        Ok(false) => Err("navigation should have returned no state"),
        Err(()) => Err("navigation should not have failed"),
    }
}

#[then("no git snapshot load is attempted")]
fn then_no_git_snapshot_load_attempted(state: &TimeTravelOrchestrationWorld) -> StepResult {
    let requested_snapshot_count = state
        .git_ops
        .with_ref(ScriptedGitOps::requested_snapshot_count)
        .ok_or("git_ops should be configured")??;
    if requested_snapshot_count == 0 {
        Ok(())
    } else {
        Err("boundary navigation should not request a snapshot")
    }
}

#[then("navigation fails with a missing commit for {expected_sha}")]
fn then_navigation_fails_with_missing_commit(
    state: &TimeTravelOrchestrationWorld,
    expected_sha: String,
) -> StepResult {
    let error = state
        .navigation_result
        .with_ref(Clone::clone)
        .ok_or("navigation_result should be set")?;
    match error {
        Err(GitOperationError::CommitNotFound { sha }) if sha.as_str() == expected_sha => Ok(()),
        Err(_) => Err("navigation should have surfaced CommitNotFound unchanged"),
        Ok(_) => Err("navigation should have failed"),
    }
}

#[then("the navigated state has no line mapping")]
fn then_navigated_state_has_no_line_mapping(state: &TimeTravelOrchestrationWorld) -> StepResult {
    let has_line_mapping = read_navigation_state(state, |navigation_state| {
        navigation_state.line_mapping().is_some()
    })?;
    if has_line_mapping {
        Err("navigated state should not include a line mapping")
    } else {
        Ok(())
    }
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Load initial state from comment metadata"
)]
fn load_initial_state_from_comment_metadata(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Navigate to an older commit"
)]
fn navigate_to_an_older_commit(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Navigate back to a newer commit"
)]
fn navigate_back_to_a_newer_commit(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Boundary navigation returns no state"
)]
fn boundary_navigation_returns_no_state(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Navigation surfaces a missing commit unchanged"
)]
fn navigation_surfaces_missing_commit_unchanged(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_orchestration.feature",
    name = "Navigation skips line mapping when head SHA is absent"
)]
fn navigation_skips_line_mapping_when_head_sha_is_absent(state: TimeTravelOrchestrationWorld) {
    let _ = state;
}
