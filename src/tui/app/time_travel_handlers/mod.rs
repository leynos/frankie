//! Time-travel navigation handlers.
//!
//! This module provides message handlers for time-travel navigation, allowing
//! users to view the code state at the time a comment was made and verify
//! line mapping correctness.

mod async_tasks;
mod error_messages;

use std::sync::Arc;

use bubbletea_rs::Cmd;
use metrics::counter;

use crate::local::CommitSha;
use crate::time_travel::{TimeTravelNavigationDirection, TimeTravelParams, TimeTravelState};
use crate::tui::messages::{AppMsg, TimeTravelFailurePhase};

use super::ReviewApp;
use async_tasks::{
    CommitNavigationTask, TimeTravelLoadTask, spawn_commit_navigation, spawn_time_travel_load,
};

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
            AppMsg::TimeTravelLoaded { session_id, state } => {
                self.handle_time_travel_loaded(*session_id, state.clone())
            }
            AppMsg::TimeTravelFailed {
                session_id,
                phase,
                error,
            } => self.handle_time_travel_failed(*session_id, *phase, error),
            AppMsg::NextCommit => self.handle_next_commit(),
            AppMsg::PreviousCommit => self.handle_previous_commit(),
            AppMsg::CommitNavigated { session_id, state } => {
                self.handle_commit_navigated(*session_id, state.clone())
            }
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

        let Some(git_ops_clone) = self.git_ops.as_ref().map(Arc::clone) else {
            self.error = Some(build_no_repo_error());
            return None;
        };
        let session_id = self.start_time_travel_session();

        // Set loading state and enter time-travel mode
        // Note: commit existence is verified asynchronously in load_time_travel_state,
        // which returns CommitNotFound error via TimeTravelFailed message if needed.
        self.time_travel_state = Some(TimeTravelState::loading(
            params.file_path().clone(),
            params.line_number(),
        ));
        self.view_mode = super::ViewMode::TimeTravel;
        counter!("time_travel_tui_state_transitions_total", "transition" => "enter").increment(1);
        tracing::info!(
            commit_sha = params.commit_sha().as_str(),
            file_path = params.file_path().as_str(),
            "entered time-travel mode"
        );

        // Spawn async task to load time-travel data
        let head_sha = self.head_commit_sha();
        let commit_history_limit = self.commit_history_limit;

        Some(spawn_time_travel_load(TimeTravelLoadTask {
            git_ops: git_ops_clone,
            params,
            head_sha,
            commit_history_limit,
            session_id,
        }))
    }

    /// Handles the `ExitTimeTravel` message.
    pub(super) fn handle_exit_time_travel(&mut self) -> Option<Cmd> {
        self.time_travel_state = None;
        self.view_mode = super::ViewMode::ReviewList;
        self.active_time_travel_session_id = None;
        counter!("time_travel_tui_state_transitions_total", "transition" => "exit").increment(1);
        tracing::info!("exited time-travel mode");
        None
    }

    /// Handles the `TimeTravelLoaded` message.
    pub(super) fn handle_time_travel_loaded(
        &mut self,
        session_id: u64,
        state: Box<TimeTravelState>,
    ) -> Option<Cmd> {
        self.accept_time_travel_state(
            TimeTravelCompletion { session_id, state },
            "load_success",
            |s| {
                tracing::info!(
                    commit_sha = s.snapshot().sha(),
                    commit_count = s.commit_count(),
                    "time-travel load succeeded"
                );
            },
        )
    }

    /// Handles the `TimeTravelFailed` message.
    pub(super) fn handle_time_travel_failed(
        &mut self,
        session_id: u64,
        phase: TimeTravelFailurePhase,
        error: &str,
    ) -> Option<Cmd> {
        if !self.is_active_time_travel_session(session_id) {
            return None;
        }
        counter!("time_travel_tui_state_transitions_total", "transition" => phase.transition())
            .increment(1);
        tracing::info!(error, "{}", phase.log_message());
        self.record_time_travel_error(error);
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
        if navigation_is_blocked(current_state, direction) {
            return None;
        }

        let navigation_state = current_state.clone();
        let git_ops = Arc::clone(self.git_ops.as_ref()?);
        let head_sha = self.head_commit_sha();
        let session_id = self.active_time_travel_session_id?;

        self.mark_time_travel_navigation_started(navigation_state.current_index(), direction);

        // Bubble Tea processes messages sequentially, so marking the state as
        // loading here is enough to block later navigation messages until the
        // command emits `CommitNavigated` or `TimeTravelFailed`.
        Some(spawn_commit_navigation(CommitNavigationTask {
            git_ops,
            state: navigation_state,
            direction,
            head_sha,
            session_id,
        }))
    }

    const fn start_time_travel_session(&mut self) -> u64 {
        let session_id = self.next_time_travel_session_id;
        self.next_time_travel_session_id = self.next_time_travel_session_id.saturating_add(1);
        self.active_time_travel_session_id = Some(session_id);
        session_id
    }

    fn is_active_time_travel_session(&self, session_id: u64) -> bool {
        self.active_time_travel_session_id == Some(session_id)
            && self.view_mode == super::ViewMode::TimeTravel
    }

    fn accept_time_travel_state(
        &mut self,
        completion: TimeTravelCompletion,
        transition: &'static str,
        log: impl FnOnce(&TimeTravelState),
    ) -> Option<Cmd> {
        if !self.is_active_time_travel_session(completion.session_id) {
            return None;
        }
        counter!(
            "time_travel_tui_state_transitions_total",
            "transition" => transition
        )
        .increment(1);
        log(&completion.state);
        self.time_travel_state = Some(*completion.state);
        None
    }

    fn mark_time_travel_navigation_started(
        &mut self,
        current_index: usize,
        direction: TimeTravelNavigationDirection,
    ) {
        if let Some(time_travel_state) = self.time_travel_state.as_mut() {
            time_travel_state.set_loading(true);
        }
        counter!("time_travel_tui_state_transitions_total", "transition" => "navigation_start")
            .increment(1);
        tracing::info!(
            direction = ?direction,
            current_index,
            "started time-travel navigation"
        );
    }

    fn record_time_travel_error(&mut self, error: &str) {
        if let Some(ref mut state) = self.time_travel_state {
            state.set_error(error.to_owned());
        } else {
            self.error = Some(error.to_owned());
            self.view_mode = super::ViewMode::ReviewList;
        }
    }

    /// Handles the `CommitNavigated` message.
    pub(super) fn handle_commit_navigated(
        &mut self,
        session_id: u64,
        state: Box<TimeTravelState>,
    ) -> Option<Cmd> {
        self.accept_time_travel_state(
            TimeTravelCompletion { session_id, state },
            "navigation_success",
            |s| {
                tracing::info!(
                    commit_sha = s.snapshot().sha(),
                    current_index = s.current_index(),
                    "time-travel navigation succeeded"
                );
            },
        )
    }
}

struct TimeTravelCompletion {
    session_id: u64,
    state: Box<TimeTravelState>,
}

/// Builds the error message for missing repository, using stored context.
fn build_no_repo_error() -> String {
    crate::tui::get_time_travel_context()
        .map_or_else(error_messages::build_fallback_time_travel_error, |ctx| {
            error_messages::build_time_travel_error(&ctx)
        })
}

fn navigation_is_blocked(
    current_state: &TimeTravelState,
    direction: TimeTravelNavigationDirection,
) -> bool {
    let is_blocked = !direction.can_navigate(current_state);
    if is_blocked {
        tracing::debug!(
            direction = ?direction,
            loading = current_state.is_loading(),
            current_index = current_state.current_index(),
            commit_count = current_state.commit_count(),
            "time-travel TUI navigation ignored"
        );
    }
    is_blocked
}

#[cfg(test)]
mod tests;
