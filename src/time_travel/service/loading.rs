//! Loading helpers for shared time-travel service orchestration.

use std::time::Instant;

use metrics::{counter, histogram};

use crate::local::{CommitSha, CommitSnapshot, GitOperationError, GitOperations};

use super::git_error_type;
use super::line_mapping::{LineMappingContext, verify_line_mapping};
use super::navigation::TimeTravelNavigationDirection;
use crate::time_travel::{TimeTravelInitParams, TimeTravelParams, TimeTravelState};

/// Loads the initial time-travel state for a comment.
///
/// This function provides the core loading logic for time-travel mode,
/// fetching the commit snapshot, parent commit history, and optionally
/// verifying line mappings when both a line number and HEAD SHA are provided.
///
/// The `commit_history_limit` parameter is defensively clamped to a minimum
/// of `1` to ensure at least one commit is loaded.
///
/// # Errors
///
/// Returns a [`GitOperationError`] if:
/// - The commit snapshot cannot be retrieved
/// - The parent commit history cannot be fetched
///
/// # Example
///
/// ```no_run
/// use frankie::local::{CommitSha, RepoFilePath};
/// use frankie::time_travel::{TimeTravelParams, load_time_travel_state};
/// # use frankie::local::GitOperations;
///
/// # fn example(git_ops: &dyn GitOperations) -> Result<(), Box<dyn std::error::Error>> {
/// let params = TimeTravelParams::new(
///     CommitSha::new("abc123".to_owned()),
///     RepoFilePath::new("src/main.rs".to_owned()),
///     Some(42),
/// );
///
/// let state = load_time_travel_state(git_ops, &params, None, 50)?;
/// assert_eq!(state.current_index(), 0);
/// # Ok(())
/// # }
/// ```
pub fn load_time_travel_state(
    git_ops: &dyn GitOperations,
    params: &TimeTravelParams,
    head_sha: Option<&CommitSha>,
    commit_history_limit: usize,
) -> Result<TimeTravelState, GitOperationError> {
    let started_at = Instant::now();
    counter!("time_travel_service_operations_total", "operation" => "load").increment(1);
    let result = (|| {
        let snapshot = load_initial_snapshot(git_ops, params)?;
        let effective_limit = commit_history_limit.max(1);
        let commit_history = load_commit_history(git_ops, params.commit_sha(), effective_limit)?;
        let line_mapping = verify_line_mapping(&LineMappingContext {
            git_ops,
            commit_sha: params.commit_sha(),
            file_path: params.file_path(),
            original_line: params.line_number(),
            head_sha,
        });

        Ok(TimeTravelState::new(TimeTravelInitParams {
            snapshot,
            file_path: params.file_path().clone(),
            original_line: params.line_number(),
            line_mapping,
            commit_history,
            current_index: 0,
        }))
    })();
    histogram!("time_travel_service_operation_duration_seconds", "operation" => "load")
        .record(started_at.elapsed().as_secs_f64());
    if let Ok(state) = &result {
        counter!("time_travel_service_operations_success_total", "operation" => "load")
            .increment(1);
        tracing::info!(
            commit_sha = params.commit_sha().as_str(),
            file_path = params.file_path().as_str(),
            commit_count = state.commit_count(),
            has_line_mapping = state.line_mapping().is_some(),
            "loaded time-travel state"
        );
    }
    result
}

pub(super) fn load_initial_snapshot(
    git_ops: &dyn GitOperations,
    params: &TimeTravelParams,
) -> Result<CommitSnapshot, GitOperationError> {
    tracing::debug!(
        commit_sha = params.commit_sha().as_str(),
        file_path = params.file_path().as_str(),
        "loading time-travel snapshot"
    );
    git_ops
        .get_commit_snapshot(params.commit_sha(), Some(params.file_path()))
        .map_err(|error| {
            counter!(
                "time_travel_service_operation_errors_total",
                "operation" => "load",
                "error_type" => git_error_type(&error)
            )
            .increment(1);
            tracing::debug!(
                commit_sha = params.commit_sha().as_str(),
                file_path = params.file_path().as_str(),
                ?error,
                "time-travel snapshot load failed"
            );
            error
        })
}

fn load_commit_history(
    git_ops: &dyn GitOperations,
    commit_sha: &CommitSha,
    effective_limit: usize,
) -> Result<Vec<CommitSha>, GitOperationError> {
    tracing::debug!(
        commit_sha = commit_sha.as_str(),
        limit = effective_limit,
        "loading time-travel commit history"
    );
    git_ops
        .get_parent_commits(commit_sha, effective_limit)
        .map_err(|error| {
            counter!(
                "time_travel_service_operation_errors_total",
                "operation" => "load",
                "error_type" => git_error_type(&error)
            )
            .increment(1);
            tracing::debug!(
                commit_sha = commit_sha.as_str(),
                limit = effective_limit,
                ?error,
                "time-travel commit history load failed"
            );
            error
        })
}

pub(super) fn load_navigation_snapshot(
    git_ops: &dyn GitOperations,
    state: &TimeTravelState,
    direction: TimeTravelNavigationDirection,
    target_sha: &CommitSha,
) -> Result<CommitSnapshot, GitOperationError> {
    tracing::debug!(
        direction = ?direction,
        current_index = state.current_index(),
        target_sha = target_sha.as_str(),
        file_path = state.file_path().as_str(),
        "loading time-travel navigation snapshot"
    );
    git_ops
        .get_commit_snapshot(target_sha, Some(state.file_path()))
        .map_err(|error| {
            counter!(
                "time_travel_service_operation_errors_total",
                "operation" => "navigate",
                "error_type" => git_error_type(&error)
            )
            .increment(1);
            tracing::debug!(
                direction = ?direction,
                target_sha = target_sha.as_str(),
                file_path = state.file_path().as_str(),
                ?error,
                "time-travel navigation snapshot load failed"
            );
            error
        })
}
