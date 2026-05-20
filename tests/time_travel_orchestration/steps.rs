//! Step definitions for time-travel orchestration scenarios.

use frankie::local::{CommitSha, GitOperationError, LineMappingVerification, RepoFilePath};
use frankie::time_travel::{TimeTravelNavigationDirection, TimeTravelParams, TimeTravelState};
use rstest_bdd_macros::{given, then, when};

use crate::time_travel_orchestration_helpers::{
    ScriptedGitOps, StepResult, TimeTravelOrchestrationWorld, assert_eq_step, build_loaded_state,
    build_snapshot, navigate, read_load_result, read_navigation_state,
};
use frankie::time_travel::load_time_travel_state;

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
    navigate(state, TimeTravelNavigationDirection::Previous)
}

#[when("the state is navigated to the next commit")]
fn when_state_navigated_next(state: &TimeTravelOrchestrationWorld) -> StepResult {
    navigate(state, TimeTravelNavigationDirection::Next)
}

#[then("the loaded state snapshot SHA is {expected}")]
fn then_loaded_state_snapshot_sha(
    state: &TimeTravelOrchestrationWorld,
    expected: String,
) -> StepResult {
    let actual = read_load_result(state, |s| s.snapshot().sha().to_owned())?;
    assert_eq_step(
        &actual,
        &expected,
        "loaded state snapshot SHA should match the expected SHA",
    )
}

#[then("the loaded state index is {expected}")]
fn then_loaded_state_index(state: &TimeTravelOrchestrationWorld, expected: usize) -> StepResult {
    let actual = read_load_result(state, TimeTravelState::current_index)?;
    assert_eq_step(
        &actual,
        &expected,
        "loaded state index should match the expected index",
    )
}

#[then("the loaded history count is {expected}")]
fn then_loaded_history_count(state: &TimeTravelOrchestrationWorld, expected: usize) -> StepResult {
    let actual = read_load_result(state, TimeTravelState::commit_count)?;
    assert_eq_step(
        &actual,
        &expected,
        "loaded history count should match the expected value",
    )
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
    let actual = read_navigation_state(state, |s| s.snapshot().sha().to_owned())?;
    assert_eq_step(
        &actual,
        &expected,
        "navigation snapshot SHA should match the expected value",
    )
}

#[then("navigation returns history index {expected}")]
fn then_navigation_returns_history_index(
    state: &TimeTravelOrchestrationWorld,
    expected: usize,
) -> StepResult {
    let actual = read_navigation_state(state, TimeTravelState::current_index)?;
    assert_eq_step(
        &actual,
        &expected,
        "navigation history index should match the expected value",
    )
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
