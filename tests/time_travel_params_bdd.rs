//! Behavioural tests for the public time-travel parameter extraction API.
//!
//! These tests import `frankie::time_travel::TimeTravelParams` and
//! `frankie::ReviewComment` — not `frankie::tui` — to prove that the
//! public library surface is usable by an external caller.

use frankie::ReviewComment;
use frankie::time_travel::{TimeTravelParams, TimeTravelParamsError};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

/// Error type for BDD test step failures.
type StepError = &'static str;

/// Result type for BDD test steps.
type StepResult = Result<(), StepError>;

/// State shared across steps in a time-travel params extraction scenario.
#[derive(ScenarioState, Default)]
struct ParamsState {
    /// The review comment under test.
    comment: Slot<ReviewComment>,
    /// The extraction result (success or failure).
    result: Slot<Result<TimeTravelParams, TimeTravelParamsError>>,
}

#[fixture]
fn state() -> ParamsState {
    ParamsState::default()
}

// -- Given steps --

#[given("a review comment with commit SHA {sha} and file path {path}")]
fn given_comment_with_sha_and_path(state: &ParamsState, sha: String, path: String) {
    state.comment.set(ReviewComment {
        commit_sha: Some(sha),
        file_path: Some(path),
        ..ReviewComment::default()
    });
}

#[given("the comment has line number {line} and original line number {original}")]
fn given_comment_lines(state: &ParamsState, line: u32, original: u32) {
    state.comment.with_mut(|c| {
        c.line_number = Some(line);
        c.original_line_number = Some(original);
    });
}

#[given("the comment has no current line number but original line number {original}")]
fn given_comment_original_only(state: &ParamsState, original: u32) {
    state.comment.with_mut(|c| {
        c.line_number = None;
        c.original_line_number = Some(original);
    });
}

#[given("a review comment without a commit SHA")]
fn given_comment_no_sha(state: &ParamsState) {
    state.comment.set(ReviewComment {
        commit_sha: None,
        file_path: Some("src/lib.rs".to_owned()),
        ..ReviewComment::default()
    });
}

#[given("a review comment with commit SHA {sha} but no file path")]
fn given_comment_no_path(state: &ParamsState, sha: String) {
    state.comment.set(ReviewComment {
        commit_sha: Some(sha),
        file_path: None,
        ..ReviewComment::default()
    });
}

// -- When steps --

#[when("time-travel parameters are extracted")]
fn when_extract(state: &ParamsState) -> StepResult {
    let comment = state
        .comment
        .with_ref(Clone::clone)
        .ok_or("comment should be set before extraction")?;
    state.result.set(TimeTravelParams::from_comment(&comment));
    Ok(())
}

// -- Then steps --

#[then("extraction succeeds")]
fn then_succeeds(state: &ParamsState) -> StepResult {
    state
        .result
        .with_ref(Result::is_ok)
        .filter(|ok| *ok)
        .ok_or("extraction should succeed")?;
    Ok(())
}

#[then("the commit SHA is {expected}")]
fn then_commit_sha(state: &ParamsState, expected: String) -> StepResult {
    let actual = state
        .result
        .with_ref(|r| r.as_ref().ok().map(|p| p.commit_sha().as_str().to_owned()))
        .ok_or("result should be set")?
        .ok_or("extraction should have succeeded")?;
    if actual == expected {
        Ok(())
    } else {
        Err("commit SHA does not match expected value")
    }
}

#[then("the file path is {expected}")]
fn then_file_path(state: &ParamsState, expected: String) -> StepResult {
    let actual = state
        .result
        .with_ref(|r| r.as_ref().ok().map(|p| p.file_path().as_str().to_owned()))
        .ok_or("result should be set")?
        .ok_or("extraction should have succeeded")?;
    if actual == expected {
        Ok(())
    } else {
        Err("file path does not match expected value")
    }
}

#[then("the line number is {expected}")]
fn then_line_number(state: &ParamsState, expected: u32) -> StepResult {
    let actual = state
        .result
        .with_ref(|r| r.as_ref().ok().map(TimeTravelParams::line_number))
        .ok_or("result should be set")?
        .ok_or("extraction should have succeeded")?;
    if actual == Some(expected) {
        Ok(())
    } else {
        Err("line number does not match expected value")
    }
}

#[then("extraction fails with a missing commit SHA error")]
fn then_missing_sha(state: &ParamsState) -> StepResult {
    let err = state
        .result
        .with_ref(|r| r.as_ref().err().cloned())
        .flatten()
        .ok_or("extraction should have failed")?;
    if err == TimeTravelParamsError::MissingCommitSha {
        Ok(())
    } else {
        Err("expected MissingCommitSha error variant")
    }
}

#[then("extraction fails with a missing file path error")]
fn then_missing_path(state: &ParamsState) -> StepResult {
    let err = state
        .result
        .with_ref(|r| r.as_ref().err().cloned())
        .flatten()
        .ok_or("extraction should have failed")?;
    if err == TimeTravelParamsError::MissingFilePath {
        Ok(())
    } else {
        Err("expected MissingFilePath error variant")
    }
}

// -- Scenario bindings --

#[scenario(
    path = "tests/features/time_travel_params.feature",
    name = "Derive parameters from a complete review comment"
)]
fn derive_params_complete(state: ParamsState) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_params.feature",
    name = "Fall back to original line when current line is missing"
)]
fn derive_params_fallback(state: ParamsState) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_params.feature",
    name = "Fail when the commit SHA is missing"
)]
fn derive_params_missing_sha(state: ParamsState) {
    let _ = state;
}

#[scenario(
    path = "tests/features/time_travel_params.feature",
    name = "Fail when the file path is missing"
)]
fn derive_params_missing_path(state: ParamsState) {
    let _ = state;
}
