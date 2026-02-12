//! Time-travel navigation handlers.
//!
//! This module provides message handlers for time-travel navigation, allowing
//! users to view the code state at the time a comment was made and verify
//! line mapping correctness.

mod error_messages;

use std::any::Any;
use std::sync::Arc;

use bubbletea_rs::Cmd;

use crate::local::{CommitSha, GitOperationError, GitOperations, LineMappingRequest, RepoFilePath};
use crate::tui::messages::AppMsg;
use crate::tui::state::{TimeTravelInitParams, TimeTravelParams, TimeTravelState};

use super::ReviewApp;

/// Maximum number of commits to load in history.
const COMMIT_HISTORY_LIMIT: usize = 50;

/// Context for verifying line mapping between commits.
#[derive(Debug, Clone)]
struct LineMappingContext<'a> {
    /// SHA of the commit where the line was originally referenced.
    commit_sha: &'a CommitSha,
    /// Path to the file being verified.
    file_path: &'a RepoFilePath,
    /// Original line number from the comment, if available.
    original_line: Option<u32>,
    /// SHA of the HEAD commit for comparison, if available.
    head_sha: Option<&'a CommitSha>,
}

/// Direction for commit navigation in time-travel mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavigationDirection {
    /// Navigate to the next (more recent) commit.
    Next,
    /// Navigate to the previous (older) commit.
    Previous,
}

impl NavigationDirection {
    /// Returns whether navigation in this direction is possible.
    const fn can_navigate(self, state: &TimeTravelState) -> bool {
        match self {
            Self::Next => state.can_go_next(),
            Self::Previous => state.can_go_previous(),
        }
    }

    /// Returns the target commit SHA for this direction.
    fn target_sha(self, state: &TimeTravelState) -> Option<&CommitSha> {
        match self {
            Self::Next => state.next_commit_sha(),
            Self::Previous => state.previous_commit_sha(),
        }
    }

    /// Calculates the new index after navigating in this direction.
    const fn calculate_index(self, current: usize) -> usize {
        match self {
            Self::Next => current.saturating_sub(1),
            Self::Previous => current + 1,
        }
    }
}

/// Context for navigating to a specific commit in time-travel mode.
#[derive(Debug, Clone)]
struct CommitNavigationContext {
    /// SHA of the commit to navigate to.
    sha: CommitSha,
    /// Path to the file being viewed.
    file_path: RepoFilePath,
    /// Original line number from the comment.
    original_line: Option<u32>,
    /// SHA of the HEAD commit for line mapping verification.
    head_sha: Option<CommitSha>,
    /// New index in the commit history.
    new_index: usize,
    /// List of commit SHAs in the history.
    commit_history: Vec<CommitSha>,
}

impl ReviewApp {
    /// Converts the stored HEAD SHA string to a `CommitSha` newtype.
    fn head_commit_sha(&self) -> Option<CommitSha> {
        self.head_sha.as_ref().map(|s| CommitSha::new(s.clone()))
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

        let Some(params) = TimeTravelParams::from_comment(comment) else {
            self.error = Some("Comment lacks commit SHA or file path".to_owned());
            return None;
        };

        let Some(ref git_ops) = self.git_ops else {
            self.error = Some(build_no_repo_error());
            return None;
        };

        // Set loading state and enter time-travel mode
        // Note: commit existence is verified asynchronously in load_time_travel_state,
        // which returns CommitNotFound error via TimeTravelFailed message if needed.
        self.time_travel_state = Some(TimeTravelState::loading(
            params.file_path.clone(),
            params.line_number,
        ));
        self.view_mode = super::ViewMode::TimeTravel;

        // Spawn async task to load time-travel data
        let git_ops_clone = Arc::clone(git_ops);
        let head_sha = self.head_commit_sha();

        Some(spawn_time_travel_load(git_ops_clone, params, head_sha))
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
        self.handle_commit_navigation(NavigationDirection::Next)
    }

    /// Handles the `PreviousCommit` message.
    pub(super) fn handle_previous_commit(&mut self) -> Option<Cmd> {
        self.handle_commit_navigation(NavigationDirection::Previous)
    }

    /// Handles commit navigation in the given direction.
    fn handle_commit_navigation(&mut self, direction: NavigationDirection) -> Option<Cmd> {
        let context = {
            let state = self.time_travel_state.as_ref()?;

            if !direction.can_navigate(state) {
                return None;
            }

            CommitNavigationContext {
                sha: direction.target_sha(state)?.clone(),
                file_path: state.file_path().clone(),
                original_line: state.original_line(),
                head_sha: self.head_commit_sha(),
                new_index: direction.calculate_index(state.current_index()),
                commit_history: state.commit_history().to_vec(),
            }
        };

        let git_ops = Arc::clone(self.git_ops.as_ref()?);

        // Set loading state
        if let Some(state) = self.time_travel_state.as_mut() {
            state.set_loading(true);
        }

        Some(spawn_commit_navigation(git_ops, context))
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
) -> Cmd {
    spawn_load_task(
        git_ops,
        move |ops| load_time_travel_state(ops, &params, head_sha.as_ref()),
        |state| AppMsg::TimeTravelLoaded(Box::new(state)),
    )
}

/// Spawns an async task to navigate to a different commit.
fn spawn_commit_navigation(
    git_ops: Arc<dyn GitOperations>,
    context: CommitNavigationContext,
) -> Cmd {
    spawn_load_task(
        git_ops,
        move |ops| load_commit_snapshot(ops, context),
        |state| AppMsg::CommitNavigated(Box::new(state)),
    )
}

/// Verifies line mapping between a commit and HEAD.
///
/// Returns `None` if either `original_line` or `head_sha` is `None`, or if
/// the verification fails.
fn verify_line_mapping_optional(
    git_ops: &dyn GitOperations,
    context: &LineMappingContext<'_>,
) -> Option<crate::local::LineMappingVerification> {
    let (line, head) = context.original_line.zip(context.head_sha)?;
    let request = LineMappingRequest::new(
        context.commit_sha.as_str().to_owned(),
        head.as_str().to_owned(),
        context.file_path.as_str().to_owned(),
        line,
    );
    git_ops.verify_line_mapping(&request).ok()
}

/// Loads the initial time-travel state for a comment.
fn load_time_travel_state(
    git_ops: &dyn GitOperations,
    params: &TimeTravelParams,
    head_sha: Option<&CommitSha>,
) -> Result<TimeTravelState, GitOperationError> {
    // Get commit snapshot with file content
    let snapshot = git_ops.get_commit_snapshot(&params.commit_sha, Some(&params.file_path))?;

    // Get commit history
    let commit_history = git_ops.get_parent_commits(&params.commit_sha, COMMIT_HISTORY_LIMIT)?;

    // Verify line mapping if we have a line number and HEAD
    let line_mapping = verify_line_mapping_optional(
        git_ops,
        &LineMappingContext {
            commit_sha: &params.commit_sha,
            file_path: &params.file_path,
            original_line: params.line_number,
            head_sha,
        },
    );

    Ok(TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: params.file_path.clone(),
        original_line: params.line_number,
        line_mapping,
        commit_history,
        current_index: 0,
    }))
}

/// Loads a commit snapshot for navigation.
fn load_commit_snapshot(
    git_ops: &dyn GitOperations,
    context: CommitNavigationContext,
) -> Result<TimeTravelState, GitOperationError> {
    // Get commit snapshot with file content
    let snapshot = git_ops.get_commit_snapshot(&context.sha, Some(&context.file_path))?;

    // Verify line mapping if we have a line number and HEAD
    let line_mapping = verify_line_mapping_optional(
        git_ops,
        &LineMappingContext {
            commit_sha: &context.sha,
            file_path: &context.file_path,
            original_line: context.original_line,
            head_sha: context.head_sha.as_ref(),
        },
    );

    Ok(TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: context.file_path,
        original_line: context.original_line,
        line_mapping,
        commit_history: context.commit_history,
        current_index: context.new_index,
    }))
}

#[cfg(test)]
#[expect(
    clippy::ref_option_ref,
    reason = "Generated by mockall macro for Option<&T> parameters"
)]
mod tests;
