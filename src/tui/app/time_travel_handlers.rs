//! Time-travel navigation handlers.
//!
//! This module provides message handlers for time-travel navigation, allowing
//! users to view the code state at the time a comment was made and verify
//! line mapping correctness.

use std::any::Any;
use std::sync::Arc;

use bubbletea_rs::Cmd;

use crate::local::{CommitSha, GitOperationError, GitOperations, LineMappingRequest, RepoFilePath};
use crate::tui::messages::AppMsg;
use crate::tui::state::{TimeTravelInitParams, TimeTravelParams, TimeTravelState};

use super::ReviewApp;

/// Maximum number of commits to load in history.
const COMMIT_HISTORY_LIMIT: usize = 50;

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
    fn can_navigate(self, state: &TimeTravelState) -> bool {
        match self {
            Self::Next => state.can_go_next(),
            Self::Previous => state.can_go_previous(),
        }
    }

    /// Returns the target commit SHA for this direction.
    fn target_sha(self, state: &TimeTravelState) -> Option<&str> {
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
    sha: String,
    /// Path to the file being viewed.
    file_path: String,
    /// Original line number from the comment.
    original_line: Option<u32>,
    /// SHA of the HEAD commit for line mapping verification.
    head_sha: Option<String>,
    /// New index in the commit history.
    new_index: usize,
    /// List of commit SHAs in the history.
    commit_history: Vec<String>,
}

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
        let commit_sha = CommitSha::new(params.commit_sha.clone());
        if !git_ops.commit_exists(&commit_sha) {
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
        let git_ops_clone = Arc::clone(git_ops);
        let head_sha = self.head_sha.clone();

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
                sha: direction.target_sha(state)?.to_owned(),
                file_path: state.file_path().to_owned(),
                original_line: state.original_line(),
                head_sha: self.head_sha.clone(),
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

/// Spawns an async task that loads data and maps the result to a message.
fn spawn_load_task<T, F, L>(git_ops: Arc<dyn GitOperations>, loader: L, success_msg: F) -> Cmd
where
    T: Send + 'static,
    F: FnOnce(T) -> AppMsg + Send + 'static,
    L: FnOnce(&dyn GitOperations) -> Result<T, GitOperationError> + Send + 'static,
{
    Box::pin(async move {
        match loader(&*git_ops) {
            Ok(value) => Some(Box::new(success_msg(value)) as Box<dyn Any + Send>),
            Err(e) => {
                Some(Box::new(AppMsg::TimeTravelFailed(e.to_string())) as Box<dyn Any + Send>)
            }
        }
    })
}

/// Spawns an async task to load time-travel data.
fn spawn_time_travel_load(
    git_ops: Arc<dyn GitOperations>,
    params: TimeTravelParams,
    head_sha: Option<String>,
) -> Cmd {
    spawn_load_task(
        git_ops,
        move |ops| load_time_travel_state(ops, &params, head_sha.as_deref()),
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

/// Loads the initial time-travel state for a comment.
fn load_time_travel_state(
    git_ops: &dyn GitOperations,
    params: &TimeTravelParams,
    head_sha: Option<&str>,
) -> Result<TimeTravelState, GitOperationError> {
    let commit_sha = CommitSha::new(params.commit_sha.clone());
    let file_path = RepoFilePath::new(params.file_path.clone());

    // Get commit snapshot with file content
    let snapshot = git_ops.get_commit_snapshot(&commit_sha, Some(&file_path))?;

    // Get commit history
    let commit_history_shas = git_ops.get_parent_commits(&commit_sha, COMMIT_HISTORY_LIMIT)?;
    let commit_history: Vec<String> = commit_history_shas
        .into_iter()
        .map(|sha| sha.to_string())
        .collect();

    // Verify line mapping if we have a line number and HEAD
    let line_mapping = if let (Some(line), Some(head)) = (params.line_number, head_sha) {
        let request = LineMappingRequest::new(
            params.commit_sha.clone(),
            head.to_owned(),
            params.file_path.clone(),
            line,
        );
        git_ops.verify_line_mapping(&request).ok()
    } else {
        None
    };

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
    let commit_sha = CommitSha::new(context.sha.clone());
    let repo_file_path = RepoFilePath::new(context.file_path.clone());

    // Get commit snapshot with file content
    let snapshot = git_ops.get_commit_snapshot(&commit_sha, Some(&repo_file_path))?;

    // Verify line mapping if we have a line number and HEAD
    let line_mapping = if let (Some(line), Some(head)) = (context.original_line, &context.head_sha)
    {
        let request = LineMappingRequest::new(
            context.sha.clone(),
            head.clone(),
            context.file_path.clone(),
            line,
        );
        git_ops.verify_line_mapping(&request).ok()
    } else {
        None
    };

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
#[expect(clippy::unwrap_used, reason = "Tests panic on failure")]
#[expect(
    clippy::ref_option_ref,
    reason = "Generated by mockall macro for Option<&T> parameters"
)]
mod tests {
    use super::*;
    use crate::github::models::ReviewComment;
    use crate::github::models::test_support::minimal_review;
    use crate::local::{CommitMetadata, CommitSnapshot, LineMappingVerification};
    use chrono::Utc;
    use mockall::mock;

    // Mock GitOperations using mockall
    mock! {
        pub GitOps {}

        impl std::fmt::Debug for GitOps {
            fn fmt<'a>(&self, f: &mut std::fmt::Formatter<'a>) -> std::fmt::Result;
        }

        impl GitOperations for GitOps {
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

    /// Helper to create a test commit snapshot.
    fn create_test_snapshot() -> CommitSnapshot {
        let timestamp = Utc::now();
        let metadata = CommitMetadata::new(
            "abc1234567890".to_owned(),
            "Test commit".to_owned(),
            "Test Author".to_owned(),
            timestamp,
        );
        CommitSnapshot::with_file_content(
            metadata,
            "src/main.rs".to_owned(),
            "fn main() {}".to_owned(),
        )
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
        let mut git_ops = MockGitOps::new();
        let test_snapshot = create_test_snapshot();
        let snapshot_clone = test_snapshot.clone();

        // Expect get_commit_snapshot to be called with the commit SHA
        git_ops
            .expect_get_commit_snapshot()
            .times(1)
            .returning(move |_sha, _file_path| Ok(snapshot_clone.clone()));

        // Expect get_parent_commits to be called
        git_ops
            .expect_get_parent_commits()
            .times(1)
            .returning(|_sha, _limit| {
                Ok(vec![
                    CommitSha::new("abc1234567890".to_owned()),
                    CommitSha::new("def5678901234".to_owned()),
                ])
            });

        // Expect verify_line_mapping to be called with line number
        git_ops
            .expect_verify_line_mapping()
            .times(1)
            .returning(|request| Ok(LineMappingVerification::exact(request.line)));

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
        let mut git_ops = MockGitOps::new();

        // Expect get_commit_snapshot to be called and return CommitNotFound error
        git_ops
            .expect_get_commit_snapshot()
            .times(1)
            .returning(|sha, _file_path| {
                Err(GitOperationError::CommitNotFound {
                    sha: sha.to_string(),
                })
            });

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
