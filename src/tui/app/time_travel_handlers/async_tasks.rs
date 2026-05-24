//! Async command builders for time-travel TUI handlers.

use std::any::Any;
use std::sync::Arc;

use bubbletea_rs::Cmd;

use crate::local::{CommitSha, GitOperationError, GitOperations};
use crate::time_travel::{self, TimeTravelNavigationDirection, TimeTravelParams, TimeTravelState};
use crate::tui::messages::{AppMsg, TimeTravelFailurePhase};

pub(super) struct TimeTravelLoadTask {
    pub(super) git_ops: Arc<dyn GitOperations>,
    pub(super) params: TimeTravelParams,
    pub(super) head_sha: Option<CommitSha>,
    pub(super) commit_history_limit: usize,
    pub(super) session_id: u64,
}

pub(super) struct CommitNavigationTask {
    pub(super) git_ops: Arc<dyn GitOperations>,
    pub(super) state: TimeTravelState,
    pub(super) direction: TimeTravelNavigationDirection,
    pub(super) head_sha: Option<CommitSha>,
    pub(super) session_id: u64,
}

/// Spawns an async task to load time-travel data.
pub(super) fn spawn_time_travel_load(task: TimeTravelLoadTask) -> Cmd {
    let TimeTravelLoadTask {
        git_ops,
        params,
        head_sha,
        commit_history_limit,
        session_id,
    } = task;

    spawn_load_task(
        git_ops,
        move |ops| {
            time_travel::load_time_travel_state(
                ops,
                &params,
                head_sha.as_ref(),
                commit_history_limit,
            )
        },
        move |state| AppMsg::TimeTravelLoaded {
            session_id,
            state: Box::new(state),
        },
        session_id,
    )
}

/// Spawns an async task to navigate to a different commit.
pub(super) fn spawn_commit_navigation(task: CommitNavigationTask) -> Cmd {
    let CommitNavigationTask {
        git_ops,
        state,
        direction,
        head_sha,
        session_id,
    } = task;

    Box::pin(async move {
        let result = tokio::task::spawn_blocking(move || {
            time_travel::navigate_time_travel_state(&*git_ops, &state, direction, head_sha.as_ref())
        })
        .await;
        map_navigation_result(result, session_id)
    })
}

/// Spawns an async task that loads data and maps the result to a message.
///
/// Uses `tokio::task::spawn_blocking` to offload synchronous git2 operations
/// to a blocking thread pool, preventing the async executor from being stalled.
fn spawn_load_task<T, F, L>(
    git_ops: Arc<dyn GitOperations>,
    loader: L,
    success_msg: F,
    session_id: u64,
) -> Cmd
where
    T: Send + 'static,
    F: FnOnce(T) -> AppMsg + Send + 'static,
    L: FnOnce(&dyn GitOperations) -> Result<T, GitOperationError> + Send + 'static,
{
    Box::pin(async move {
        let result = tokio::task::spawn_blocking(move || loader(&*git_ops)).await;
        match result {
            Ok(Ok(value)) => Some(Box::new(success_msg(value)) as Box<dyn Any + Send>),
            Ok(Err(e)) => {
                tracing::debug!(?e, "time-travel blocking load failed");
                Some(time_travel_failed_msg(
                    session_id,
                    TimeTravelFailurePhase::Load,
                    e.to_string(),
                ))
            }
            Err(e) => {
                tracing::debug!(?e, "time-travel blocking load task failed to join");
                Some(time_travel_failed_msg(
                    session_id,
                    TimeTravelFailurePhase::Load,
                    format!("Task join error: {e}"),
                ))
            }
        }
    })
}

fn map_navigation_result(
    result: Result<Result<Option<TimeTravelState>, GitOperationError>, tokio::task::JoinError>,
    session_id: u64,
) -> Option<Box<dyn Any + Send>> {
    match result {
        Ok(operation_result) => map_navigation_operation_result(operation_result, session_id),
        Err(e) => Some(map_navigation_join_error(&e, session_id)),
    }
}

fn map_navigation_operation_result(
    result: Result<Option<TimeTravelState>, GitOperationError>,
    session_id: u64,
) -> Option<Box<dyn Any + Send>> {
    match result {
        Ok(Some(navigated_state)) => Some(Box::new(AppMsg::CommitNavigated {
            session_id,
            state: Box::new(navigated_state),
        }) as Box<dyn Any + Send>),
        Ok(None) => None,
        Err(e) => {
            tracing::debug!(?e, "time-travel navigation failed");
            Some(time_travel_failed_msg(
                session_id,
                TimeTravelFailurePhase::Navigate,
                e.to_string(),
            ))
        }
    }
}

fn map_navigation_join_error(
    error: &tokio::task::JoinError,
    session_id: u64,
) -> Box<dyn Any + Send> {
    tracing::debug!(?error, "time-travel navigation task failed to join");
    time_travel_failed_msg(
        session_id,
        TimeTravelFailurePhase::Navigate,
        format!("Task join error: {error}"),
    )
}

fn time_travel_failed_msg(
    session_id: u64,
    phase: TimeTravelFailurePhase,
    error: String,
) -> Box<dyn Any + Send> {
    Box::new(AppMsg::TimeTravelFailed {
        session_id,
        phase,
        error,
    })
}
