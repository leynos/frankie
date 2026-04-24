//! Shared orchestration services for time-travel loading and navigation.
//!
//! These functions keep Git-backed time-travel orchestration in the library
//! layer so hosts can materialize and navigate historical snapshots without
//! depending on Bubble Tea, Tokio, or TUI-only storage.

use crate::local::{
    CommitSha, GitOperationError, GitOperations, LineMappingRequest, LineMappingVerification,
};

use super::{TimeTravelInitParams, TimeTravelParams, TimeTravelState};

#[derive(Debug, Clone, Copy)]
struct LineMappingContext<'a> {
    git_ops: &'a dyn GitOperations,
    commit_sha: &'a CommitSha,
    file_path: &'a crate::local::RepoFilePath,
    original_line: Option<u32>,
    head_sha: Option<&'a CommitSha>,
}

/// Direction for navigating within a loaded time-travel history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeTravelNavigationDirection {
    /// Navigate to the next, more recent commit in the loaded history.
    Next,
    /// Navigate to the previous, older commit in the loaded history.
    Previous,
}

impl TimeTravelNavigationDirection {
    /// Returns whether navigation in this direction is currently possible.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::Utc;
    /// use frankie::local::{CommitMetadata, CommitSha, CommitSnapshot, RepoFilePath};
    /// use frankie::time_travel::{
    ///     TimeTravelInitParams, TimeTravelNavigationDirection, TimeTravelState,
    /// };
    ///
    /// let snapshot = CommitSnapshot::with_file_content(
    ///     CommitMetadata::new(
    ///         "abc123".to_owned(),
    ///         "Newest commit".to_owned(),
    ///         "Alice".to_owned(),
    ///         Utc::now(),
    ///     ),
    ///     "src/main.rs".to_owned(),
    ///     "fn main() {}".to_owned(),
    /// );
    /// let state = TimeTravelState::new(TimeTravelInitParams {
    ///     snapshot,
    ///     file_path: RepoFilePath::new("src/main.rs".to_owned()),
    ///     original_line: Some(7),
    ///     line_mapping: None,
    ///     commit_history: vec![
    ///         CommitSha::new("abc123".to_owned()),
    ///         CommitSha::new("def456".to_owned()),
    ///     ],
    ///     current_index: 0,
    /// });
    ///
    /// assert!(TimeTravelNavigationDirection::Previous.can_navigate(&state));
    /// assert!(!TimeTravelNavigationDirection::Next.can_navigate(&state));
    /// ```
    #[must_use]
    pub const fn can_navigate(self, state: &TimeTravelState) -> bool {
        match self {
            Self::Next => state.can_go_next(),
            Self::Previous => state.can_go_previous(),
        }
    }

    fn target_sha(self, state: &TimeTravelState) -> Option<&CommitSha> {
        match self {
            Self::Next => state.next_commit_sha(),
            Self::Previous => state.previous_commit_sha(),
        }
    }

    const fn target_index(self, current_index: usize) -> usize {
        match self {
            Self::Next => current_index.saturating_sub(1),
            Self::Previous => current_index + 1,
        }
    }
}

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
    let snapshot = git_ops.get_commit_snapshot(params.commit_sha(), Some(params.file_path()))?;
    let commit_history =
        git_ops.get_parent_commits(params.commit_sha(), commit_history_limit.max(1))?;
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
}

/// Navigates a loaded time-travel state to an adjacent commit.
///
/// Returns `Ok(None)` when the requested navigation is unavailable at the
/// current history boundary or while the state is marked as loading.
///
/// # Errors
///
/// Returns a [`GitOperationError`] when loading the target snapshot fails.
///
/// # Example
///
/// ```no_run
/// use frankie::local::CommitSha;
/// use frankie::local::GitOperations;
/// use frankie::time_travel::{
///     TimeTravelNavigationDirection, TimeTravelParams, load_time_travel_state,
///     navigate_time_travel_state,
/// };
///
/// # fn example(git_ops: &dyn GitOperations) -> Result<(), Box<dyn std::error::Error>> {
/// let params = TimeTravelParams::new(
///     CommitSha::new("abc123".to_owned()),
///     "src/main.rs".into(),
///     Some(42),
/// );
/// let state = load_time_travel_state(git_ops, &params, None, 50)?;
/// let older_state = navigate_time_travel_state(
///     git_ops,
///     &state,
///     TimeTravelNavigationDirection::Previous,
///     None,
/// )?;
///
/// if let Some(older_state) = older_state {
///     assert!(older_state.current_index() > state.current_index());
/// }
/// # Ok(())
/// # }
/// ```
pub fn navigate_time_travel_state(
    git_ops: &dyn GitOperations,
    state: &TimeTravelState,
    direction: TimeTravelNavigationDirection,
    head_sha: Option<&CommitSha>,
) -> Result<Option<TimeTravelState>, GitOperationError> {
    if !direction.can_navigate(state) {
        return Ok(None);
    }

    let Some(target_sha) = direction.target_sha(state) else {
        return Ok(None);
    };
    let snapshot = git_ops.get_commit_snapshot(target_sha, Some(state.file_path()))?;
    let line_mapping = verify_line_mapping(&LineMappingContext {
        git_ops,
        commit_sha: target_sha,
        file_path: state.file_path(),
        original_line: state.original_line(),
        head_sha,
    });

    Ok(Some(TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: state.file_path().clone(),
        original_line: state.original_line(),
        line_mapping,
        commit_history: state.commit_history().to_vec(),
        current_index: direction.target_index(state.current_index()),
    })))
}

fn verify_line_mapping(context: &LineMappingContext<'_>) -> Option<LineMappingVerification> {
    let (line, head) = context.original_line.zip(context.head_sha)?;
    let request = LineMappingRequest::new(
        context.commit_sha.as_str().to_owned(),
        head.as_str().to_owned(),
        context.file_path.as_str().to_owned(),
        line,
    );
    context.git_ops.verify_line_mapping(&request).ok()
}

#[cfg(test)]
#[path = "service/tests.rs"]
mod tests;
