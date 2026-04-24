//! Time-travel navigation handlers.
//!
//! This module provides message handlers for time-travel navigation, allowing
//! users to view the code state at the time a comment was made and verify
//! line mapping correctness.

mod error_messages;

use std::any::Any;
use std::sync::Arc;

use bubbletea_rs::Cmd;

use crate::local::{CommitSha, GitOperationError, GitOperations};
use crate::time_travel::{self, TimeTravelNavigationDirection, TimeTravelParams, TimeTravelState};
use crate::tui::messages::AppMsg;

use super::ReviewApp;

impl ReviewApp {
    /// Converts the stored HEAD SHA string to a `CommitSha` newtype.
    fn head_commit_sha(&self) -> Option<CommitSha> {
        self.head_sha.as_ref().map(|s| CommitSha::new(s.clone()))
    }

    /// Dispatches time-travel messages to their handlers.
    pub(super) fn handle_time_travel_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::EnterTimeTravel => self.handle_enter_time_travel(),
            AppMsg::ExitTimeTravel => self.handle_exit_time_travel(),
            AppMsg::TimeTravelLoaded(state) => self.handle_time_travel_loaded(state.clone()),
            AppMsg::TimeTravelFailed(error) => self.handle_time_travel_failed(error),
            AppMsg::NextCommit => self.handle_next_commit(),
            AppMsg::PreviousCommit => self.handle_previous_commit(),
            AppMsg::CommitNavigated(state) => self.handle_commit_navigated(state.clone()),
            _ => None,
        }
    }

    /// Handles the `EnterTimeTravel` message.
    ///
    /// Initiates loading of the time-travel state for the currently selected
    /// comment. If no comment is selected or the comment lacks required fields,
    /// shows an appropriate error.
    pub(super) fn handle_enter_time_travel(&mut self) -> Option<Cmd> {
        let Some(comment) = self.selected_comment() else {
            self.error = Some("No comment selected".to_owned());
            return None;
        };

        let params = match TimeTravelParams::from_comment(comment) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(e.to_string());
                return None;
            }
        };

        let Some(ref git_ops) = self.git_ops else {
            self.error = Some(build_no_repo_error());
            return None;
        };

        // Set loading state and enter time-travel mode
        // Note: commit existence is verified asynchronously in load_time_travel_state,
        // which returns CommitNotFound error via TimeTravelFailed message if needed.
        self.time_travel_state = Some(TimeTravelState::loading(
            params.file_path().clone(),
            params.line_number(),
        ));
        self.view_mode = super::ViewMode::TimeTravel;

        // Spawn async task to load time-travel data
        let git_ops_clone = Arc::clone(git_ops);
        let head_sha = self.head_commit_sha();
        let commit_history_limit = self.commit_history_limit;

        Some(spawn_time_travel_load(
            git_ops_clone,
            params,
            head_sha,
            commit_history_limit,
        ))
    }

    /// Handles the `ExitTimeTravel` message.
    pub(super) fn handle_exit_time_travel(&mut self) -> Option<Cmd> {
        self.time_travel_state = None;
        self.view_mode = super::ViewMode::ReviewList;
        None
    }

    /// Handles the `TimeTravelLoaded` message.
    pub(super) fn handle_time_travel_loaded(&mut self, state: Box<TimeTravelState>) -> Option<Cmd> {
        if self.view_mode == super::ViewMode::TimeTravel {
            self.time_travel_state = Some(*state);
        }
        None
    }

    /// Handles the `TimeTravelFailed` message.
    pub(super) fn handle_time_travel_failed(&mut self, error: &str) -> Option<Cmd> {
        if let Some(ref mut state) = self.time_travel_state {
            state.set_error(error.to_owned());
        } else {
            self.error = Some(error.to_owned());
            self.view_mode = super::ViewMode::ReviewList;
        }
        None
    }

    /// Handles the `NextCommit` message.
    pub(super) fn handle_next_commit(&mut self) -> Option<Cmd> {
        self.handle_commit_navigation(TimeTravelNavigationDirection::Next)
    }

    /// Handles the `PreviousCommit` message.
    pub(super) fn handle_previous_commit(&mut self) -> Option<Cmd> {
        self.handle_commit_navigation(TimeTravelNavigationDirection::Previous)
    }

    /// Handles commit navigation in the given direction.
    fn handle_commit_navigation(
        &mut self,
        direction: TimeTravelNavigationDirection,
    ) -> Option<Cmd> {
        let current_state = self.time_travel_state.as_ref()?;
        if !direction.can_navigate(current_state) {
            return None;
        }

        let navigation_state = current_state.clone();
        let git_ops = Arc::clone(self.git_ops.as_ref()?);
        let head_sha = self.head_commit_sha();

        // Set loading state
        if let Some(time_travel_state) = self.time_travel_state.as_mut() {
            time_travel_state.set_loading(true);
        }

        Some(spawn_commit_navigation(
            git_ops,
            navigation_state,
            direction,
            head_sha,
        ))
    }

    /// Handles the `CommitNavigated` message.
    pub(super) fn handle_commit_navigated(&mut self, state: Box<TimeTravelState>) -> Option<Cmd> {
        if self.view_mode == super::ViewMode::TimeTravel {
            self.time_travel_state = Some(*state);
        }
        None
    }
}

/// Builds the error message for missing repository, using stored context.
fn build_no_repo_error() -> String {
    crate::tui::get_time_travel_context()
        .map_or_else(error_messages::build_fallback_time_travel_error, |ctx| {
            error_messages::build_time_travel_error(&ctx)
        })
}

/// Spawns an async task that loads data and maps the result to a message.
///
/// Uses `tokio::task::spawn_blocking` to offload synchronous git2 operations
/// to a blocking thread pool, preventing the async executor from being stalled.
fn spawn_load_task<T, F, L>(git_ops: Arc<dyn GitOperations>, loader: L, success_msg: F) -> Cmd
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
                Some(Box::new(AppMsg::TimeTravelFailed(e.to_string())) as Box<dyn Any + Send>)
            }
            Err(e) => Some(
                Box::new(AppMsg::TimeTravelFailed(format!("Task join error: {e}")))
                    as Box<dyn Any + Send>,
            ),
        }
    })
}

/// Spawns an async task to load time-travel data.
fn spawn_time_travel_load(
    git_ops: Arc<dyn GitOperations>,
    params: TimeTravelParams,
    head_sha: Option<CommitSha>,
    commit_history_limit: usize,
) -> Cmd {
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
        |state| AppMsg::TimeTravelLoaded(Box::new(state)),
    )
}

/// Spawns an async task to navigate to a different commit.
fn spawn_commit_navigation(
    git_ops: Arc<dyn GitOperations>,
    state: TimeTravelState,
    direction: TimeTravelNavigationDirection,
    head_sha: Option<CommitSha>,
) -> Cmd {
    Box::pin(async move {
        let result = tokio::task::spawn_blocking(move || {
            time_travel::navigate_time_travel_state(&*git_ops, &state, direction, head_sha.as_ref())
        })
        .await;
        match result {
            Ok(Ok(Some(navigated_state))) => {
                Some(Box::new(AppMsg::CommitNavigated(Box::new(navigated_state)))
                    as Box<dyn Any + Send>)
            }
            Ok(Ok(None)) => None,
            Ok(Err(e)) => {
                Some(Box::new(AppMsg::TimeTravelFailed(e.to_string())) as Box<dyn Any + Send>)
            }
            Err(e) => Some(
                Box::new(AppMsg::TimeTravelFailed(format!("Task join error: {e}")))
                    as Box<dyn Any + Send>,
            ),
        }
    })
}

#[cfg(test)]
mod tests;
