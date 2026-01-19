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

use crate::local::{CommitSha, GitOperationError, GitOperations, LineMappingRequest, RepoFilePath};
use crate::tui::messages::AppMsg;
use crate::tui::state::{TimeTravelInitParams, TimeTravelParams, TimeTravelState};

use super::ReviewApp;

/// Maximum number of commits to load in history.
const COMMIT_HISTORY_LIMIT: usize = 50;

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

impl CommitNavigationContext {
    /// Creates a new commit navigation context.
    #[expect(
        clippy::too_many_arguments,
        reason = "All parameters needed for navigation context initialisation"
    )]
    const fn new(
        sha: String,
        file_path: String,
        original_line: Option<u32>,
        head_sha: Option<String>,
        new_index: usize,
        commit_history: Vec<String>,
    ) -> Self {
        Self {
            sha,
            file_path,
            original_line,
            head_sha,
            new_index,
            commit_history,
        }
    }
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
        let context = {
            let state = self.time_travel_state.as_ref()?;

            if !state.can_go_next() {
                return None;
            }

            CommitNavigationContext::new(
                state.next_commit_sha()?.to_owned(),
                state.file_path().to_owned(),
                state.original_line(),
                self.head_sha.clone(),
                state.current_index().saturating_sub(1),
                state.commit_history().to_vec(),
            )
        };

        let git_ops = Arc::clone(self.git_ops.as_ref()?);

        // Set loading state
        if let Some(state) = self.time_travel_state.as_mut() {
            state.set_loading(true);
        }

        Some(spawn_commit_navigation(git_ops, context))
    }

    /// Handles the `PreviousCommit` message.
    pub(super) fn handle_previous_commit(&mut self) -> Option<Cmd> {
        let context = {
            let state = self.time_travel_state.as_ref()?;

            if !state.can_go_previous() {
                return None;
            }

            CommitNavigationContext::new(
                state.previous_commit_sha()?.to_owned(),
                state.file_path().to_owned(),
                state.original_line(),
                self.head_sha.clone(),
                state.current_index() + 1,
                state.commit_history().to_vec(),
            )
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
fn spawn_commit_navigation(
    git_ops: Arc<dyn GitOperations>,
    context: CommitNavigationContext,
) -> Cmd {
    Box::pin(async move {
        let result = load_commit_snapshot(&*git_ops, context);

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

    let mut state = TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: context.file_path,
        original_line: context.original_line,
        line_mapping,
        commit_history: context.commit_history,
    });

    // Update to the new index
    state.update_snapshot(
        state.snapshot().clone(),
        state.line_mapping().cloned(),
        context.new_index,
    );

    Ok(state)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests panic on failure")]
mod tests {
    use super::*;
    use crate::github::models::ReviewComment;
    use crate::github::models::test_support::minimal_review;
    use crate::local::{CommitMetadata, CommitSnapshot, LineMappingVerification};
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
            let metadata = CommitMetadata::new(
                "abc1234567890".to_owned(),
                "Test commit".to_owned(),
                "Test Author".to_owned(),
                timestamp,
            );
            let commit = CommitSnapshot::with_file_content(
                metadata,
                "src/main.rs".to_owned(),
                "fn main() {}".to_owned(),
            );

            Self {
                commits: Mutex::new(vec![("abc1234567890".to_owned(), commit)]),
                history: vec!["abc1234567890".to_owned(), "def5678901234".to_owned()],
                commit_exists: true,
            }
        }

        /// Creates a snapshot from commit data, optionally including file content.
        fn create_snapshot(commit: &CommitSnapshot, include_file: bool) -> CommitSnapshot {
            if include_file {
                commit.clone()
            } else {
                let metadata = CommitMetadata::new(
                    commit.sha().to_owned(),
                    commit.message().to_owned(),
                    commit.author().to_owned(),
                    *commit.timestamp(),
                );
                CommitSnapshot::new(metadata)
            }
        }
    }

    impl GitOperations for MockGitOps {
        fn get_commit_snapshot(
            &self,
            sha: &CommitSha,
            file_path: Option<&RepoFilePath>,
        ) -> Result<CommitSnapshot, GitOperationError> {
            let commits = self.commits.lock().unwrap();
            let sha_str = sha.as_str();
            let found = commits.iter().find(|(s, _)| s == sha_str);
            let snapshot = found.map(|(_, c)| Self::create_snapshot(c, file_path.is_some()));
            snapshot.ok_or_else(|| GitOperationError::CommitNotFound {
                sha: sha.to_string(),
            })
        }

        fn get_file_at_commit(
            &self,
            sha: &CommitSha,
            _file_path: &RepoFilePath,
        ) -> Result<String, GitOperationError> {
            let commits = self.commits.lock().unwrap();
            let sha_str = sha.as_str();
            commits
                .iter()
                .find(|(s, _)| s == sha_str)
                .and_then(|(_, c)| c.file_content().map(String::from))
                .ok_or_else(|| GitOperationError::CommitNotFound {
                    sha: sha.to_string(),
                })
        }

        fn verify_line_mapping(
            &self,
            request: &LineMappingRequest,
        ) -> Result<LineMappingVerification, GitOperationError> {
            Ok(LineMappingVerification::exact(request.line))
        }

        fn get_parent_commits(
            &self,
            _sha: &CommitSha,
            limit: usize,
        ) -> Result<Vec<CommitSha>, GitOperationError> {
            Ok(self
                .history
                .iter()
                .take(limit)
                .cloned()
                .map(CommitSha::new)
                .collect())
        }

        fn commit_exists(&self, _sha: &CommitSha) -> bool {
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
