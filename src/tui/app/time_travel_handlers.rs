//! Time-travel navigation handlers.
//!
//! This module provides message handlers for time-travel navigation, allowing
//! users to view the code state at the time a comment was made and verify
//! line mapping correctness.

// Shadow reuse is acceptable for cloning references
#![expect(clippy::shadow_reuse, reason = "Clone of reference for async move")]

use std::any::Any;
use std::sync::Arc;

use bubbletea_rs::Cmd;

use crate::local::{GitOperationError, GitOperations};
use crate::tui::messages::AppMsg;
use crate::tui::state::{TimeTravelParams, TimeTravelState};

use super::ReviewApp;

/// Maximum number of commits to load in history.
const COMMIT_HISTORY_LIMIT: usize = 50;

impl ReviewApp {
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
            self.error = Some("No local repository available".to_owned());
            return None;
        };

        // Check if commit exists
        if !git_ops.commit_exists(&params.commit_sha) {
            let short_sha: String = params.commit_sha.chars().take(7).collect();
            self.error = Some(format!("Commit {short_sha} not found in local repository"));
            return None;
        }

        // Set loading state and enter time-travel mode
        self.time_travel_state = Some(TimeTravelState::loading(
            params.file_path.clone(),
            params.line_number,
        ));
        self.view_mode = super::ViewMode::TimeTravel;

        // Spawn async task to load time-travel data
        let git_ops = Arc::clone(git_ops);
        let head_sha = self.head_sha.clone();

        Some(spawn_time_travel_load(git_ops, params, head_sha))
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
        // Extract values needed for spawning the navigation task
        let (sha, file_path, original_line, new_index, commit_history) = {
            let state = self.time_travel_state.as_ref()?;

            if !state.can_go_next() {
                return None;
            }

            let sha = state.next_commit_sha()?.to_owned();
            let file_path = state.file_path().to_owned();
            let original_line = state.original_line();
            let new_index = state.current_index().saturating_sub(1);
            let commit_history = state.commit_history().to_vec();

            (sha, file_path, original_line, new_index, commit_history)
        };

        let git_ops = Arc::clone(self.git_ops.as_ref()?);
        let head_sha = self.head_sha.clone();

        // Set loading state
        if let Some(state) = self.time_travel_state.as_mut() {
            state.set_loading(true);
        }

        Some(spawn_commit_navigation(
            git_ops,
            sha,
            file_path,
            original_line,
            head_sha,
            new_index,
            commit_history,
        ))
    }

    /// Handles the `PreviousCommit` message.
    pub(super) fn handle_previous_commit(&mut self) -> Option<Cmd> {
        // Extract values needed for spawning the navigation task
        let (sha, file_path, original_line, new_index, commit_history) = {
            let state = self.time_travel_state.as_ref()?;

            if !state.can_go_previous() {
                return None;
            }

            let sha = state.previous_commit_sha()?.to_owned();
            let file_path = state.file_path().to_owned();
            let original_line = state.original_line();
            let new_index = state.current_index() + 1;
            let commit_history = state.commit_history().to_vec();

            (sha, file_path, original_line, new_index, commit_history)
        };

        let git_ops = Arc::clone(self.git_ops.as_ref()?);
        let head_sha = self.head_sha.clone();

        // Set loading state
        if let Some(state) = self.time_travel_state.as_mut() {
            state.set_loading(true);
        }

        Some(spawn_commit_navigation(
            git_ops,
            sha,
            file_path,
            original_line,
            head_sha,
            new_index,
            commit_history,
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

/// Spawns an async task to load time-travel data.
fn spawn_time_travel_load(
    git_ops: Arc<dyn GitOperations>,
    params: TimeTravelParams,
    head_sha: Option<String>,
) -> Cmd {
    Box::pin(async move {
        let result = load_time_travel_state(&*git_ops, &params, head_sha.as_deref());

        match result {
            Ok(state) => {
                Some(Box::new(AppMsg::TimeTravelLoaded(Box::new(state))) as Box<dyn Any + Send>)
            }
            Err(e) => {
                Some(Box::new(AppMsg::TimeTravelFailed(e.to_string())) as Box<dyn Any + Send>)
            }
        }
    })
}

/// Spawns an async task to navigate to a different commit.
#[expect(
    clippy::too_many_arguments,
    reason = "All parameters needed for navigation context"
)]
fn spawn_commit_navigation(
    git_ops: Arc<dyn GitOperations>,
    sha: String,
    file_path: String,
    original_line: Option<u32>,
    head_sha: Option<String>,
    new_index: usize,
    commit_history: Vec<String>,
) -> Cmd {
    Box::pin(async move {
        let result = load_commit_snapshot(
            &*git_ops,
            &sha,
            &file_path,
            original_line,
            head_sha.as_deref(),
            new_index,
            commit_history,
        );

        match result {
            Ok(state) => {
                Some(Box::new(AppMsg::CommitNavigated(Box::new(state))) as Box<dyn Any + Send>)
            }
            Err(e) => {
                Some(Box::new(AppMsg::TimeTravelFailed(e.to_string())) as Box<dyn Any + Send>)
            }
        }
    })
}

/// Loads the initial time-travel state for a comment.
fn load_time_travel_state(
    git_ops: &dyn GitOperations,
    params: &TimeTravelParams,
    head_sha: Option<&str>,
) -> Result<TimeTravelState, GitOperationError> {
    // Get commit snapshot with file content
    let snapshot = git_ops.get_commit_snapshot(&params.commit_sha, Some(&params.file_path))?;

    // Get commit history
    let commit_history = git_ops.get_parent_commits(&params.commit_sha, COMMIT_HISTORY_LIMIT)?;

    // Verify line mapping if we have a line number and HEAD
    let line_mapping = if let (Some(line), Some(head)) = (params.line_number, head_sha) {
        git_ops
            .verify_line_mapping(&params.commit_sha, head, &params.file_path, line)
            .ok()
    } else {
        None
    };

    Ok(TimeTravelState::new(
        snapshot,
        params.file_path.clone(),
        params.line_number,
        line_mapping,
        commit_history,
    ))
}

/// Loads a commit snapshot for navigation.
#[expect(
    clippy::too_many_arguments,
    reason = "All parameters needed for navigation context"
)]
fn load_commit_snapshot(
    git_ops: &dyn GitOperations,
    sha: &str,
    file_path: &str,
    original_line: Option<u32>,
    head_sha: Option<&str>,
    new_index: usize,
    commit_history: Vec<String>,
) -> Result<TimeTravelState, GitOperationError> {
    // Get commit snapshot with file content
    let snapshot = git_ops.get_commit_snapshot(sha, Some(file_path))?;

    // Verify line mapping if we have a line number and HEAD
    let line_mapping = if let (Some(line), Some(head)) = (original_line, head_sha) {
        git_ops.verify_line_mapping(sha, head, file_path, line).ok()
    } else {
        None
    };

    let mut state = TimeTravelState::new(
        snapshot,
        file_path.to_owned(),
        original_line,
        line_mapping,
        commit_history,
    );

    // Update to the new index
    state.update_snapshot(
        state.snapshot().clone(),
        state.line_mapping().cloned(),
        new_index,
    );

    Ok(state)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests panic on failure")]
mod tests {
    use super::*;
    use crate::github::models::ReviewComment;
    use crate::github::models::test_support::minimal_review;
    use crate::local::{CommitSnapshot, LineMappingVerification};
    use chrono::Utc;
    use std::sync::Mutex;

    /// Mock implementation of `GitOperations` for testing.
    #[derive(Debug)]
    struct MockGitOps {
        commits: Mutex<Vec<(String, CommitSnapshot)>>,
        history: Vec<String>,
        commit_exists: bool,
    }

    impl MockGitOps {
        fn new() -> Self {
            let timestamp = Utc::now();
            let commit = CommitSnapshot::with_file_content(
                "abc1234567890".to_owned(),
                "Test commit".to_owned(),
                "Test Author".to_owned(),
                timestamp,
                "src/main.rs".to_owned(),
                "fn main() {}".to_owned(),
            );

            Self {
                commits: Mutex::new(vec![("abc1234567890".to_owned(), commit)]),
                history: vec!["abc1234567890".to_owned(), "def5678901234".to_owned()],
                commit_exists: true,
            }
        }
    }

    impl GitOperations for MockGitOps {
        fn get_commit_snapshot(
            &self,
            sha: &str,
            file_path: Option<&str>,
        ) -> Result<CommitSnapshot, GitOperationError> {
            let commits = self.commits.lock().unwrap();
            let found = commits.iter().find(|(s, _)| s == sha);
            let snapshot = found.map(|(_, c)| match file_path {
                Some(_) => c.clone(),
                None => CommitSnapshot::new(
                    c.sha().to_owned(),
                    c.message().to_owned(),
                    c.author().to_owned(),
                    *c.timestamp(),
                ),
            });
            snapshot.ok_or_else(|| GitOperationError::CommitNotFound {
                sha: sha.to_owned(),
            })
        }

        fn get_file_at_commit(
            &self,
            sha: &str,
            _file_path: &str,
        ) -> Result<String, GitOperationError> {
            let commits = self.commits.lock().unwrap();
            commits
                .iter()
                .find(|(s, _)| s == sha)
                .and_then(|(_, c)| c.file_content().map(String::from))
                .ok_or_else(|| GitOperationError::CommitNotFound {
                    sha: sha.to_owned(),
                })
        }

        fn verify_line_mapping(
            &self,
            _old_sha: &str,
            _new_sha: &str,
            _file_path: &str,
            line: u32,
        ) -> Result<LineMappingVerification, GitOperationError> {
            Ok(LineMappingVerification::exact(line))
        }

        fn get_parent_commits(
            &self,
            _sha: &str,
            limit: usize,
        ) -> Result<Vec<String>, GitOperationError> {
            Ok(self.history.iter().take(limit).cloned().collect())
        }

        fn commit_exists(&self, _sha: &str) -> bool {
            self.commit_exists
        }
    }

    #[test]
    fn time_travel_params_from_comment() {
        let comment = ReviewComment {
            commit_sha: Some("abc123".to_owned()),
            file_path: Some("src/main.rs".to_owned()),
            line_number: Some(42),
            ..minimal_review(1, "Test", "alice")
        };

        let params = TimeTravelParams::from_comment(&comment).unwrap();
        assert_eq!(params.commit_sha, "abc123");
        assert_eq!(params.file_path, "src/main.rs");
        assert_eq!(params.line_number, Some(42));
    }

    #[test]
    fn time_travel_params_missing_sha() {
        let comment = ReviewComment {
            commit_sha: None,
            file_path: Some("src/main.rs".to_owned()),
            ..minimal_review(1, "Test", "alice")
        };

        assert!(TimeTravelParams::from_comment(&comment).is_none());
    }

    #[test]
    fn load_time_travel_state_success() {
        let git_ops = MockGitOps::new();
        let params = TimeTravelParams {
            commit_sha: "abc1234567890".to_owned(),
            file_path: "src/main.rs".to_owned(),
            line_number: Some(10),
        };

        let state = load_time_travel_state(&git_ops, &params, Some("HEAD")).unwrap();

        assert_eq!(state.snapshot().message(), "Test commit");
        assert_eq!(state.file_path(), "src/main.rs");
        assert_eq!(state.original_line(), Some(10));
        assert_eq!(state.commit_count(), 2);
    }

    #[test]
    fn load_time_travel_state_commit_not_found() {
        let git_ops = MockGitOps::new();
        let params = TimeTravelParams {
            commit_sha: "nonexistent".to_owned(),
            file_path: "src/main.rs".to_owned(),
            line_number: None,
        };

        let result = load_time_travel_state(&git_ops, &params, None);
        assert!(matches!(
            result,
            Err(GitOperationError::CommitNotFound { .. })
        ));
    }
}
