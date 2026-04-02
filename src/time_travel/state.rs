//! Public runtime state for time-travel navigation.
//!
//! This module exposes the state container used to inspect a historical file
//! snapshot and navigate through related commit history without depending on
//! `crate::tui`.

use crate::local::{
    CommitMetadata, CommitSha, CommitSnapshot, LineMappingVerification, RepoFilePath,
};

/// Parameters for initializing a time-travel state.
#[derive(Debug, Clone)]
pub struct TimeTravelInitParams {
    /// The commit snapshot to display.
    pub snapshot: CommitSnapshot,
    /// Path to the file being viewed.
    pub file_path: RepoFilePath,
    /// Line number from the original comment.
    pub original_line: Option<u32>,
    /// Verification of line mapping to HEAD.
    pub line_mapping: Option<LineMappingVerification>,
    /// List of commit SHAs in the history (most recent first).
    pub commit_history: Vec<CommitSha>,
    /// Current position in the commit history.
    pub current_index: usize,
}

/// State container for time-travel navigation.
///
/// This type captures the current historical snapshot, the commit history used
/// for navigation, and the line-mapping metadata needed by renderers.
///
/// # Example
///
/// ```
/// use chrono::Utc;
/// use frankie::local::{CommitMetadata, CommitSha, CommitSnapshot, RepoFilePath};
/// use frankie::time_travel::{TimeTravelInitParams, TimeTravelState};
///
/// let metadata = CommitMetadata::new(
///     "abc1234567890".to_owned(),
///     "Fix login validation".to_owned(),
///     "Alice".to_owned(),
///     Utc::now(),
/// );
/// let snapshot = CommitSnapshot::with_file_content(
///     metadata,
///     "src/auth.rs".to_owned(),
///     "fn login() {}".to_owned(),
/// );
///
/// let state = TimeTravelState::new(TimeTravelInitParams {
///     snapshot,
///     file_path: RepoFilePath::new("src/auth.rs".to_owned()),
///     original_line: Some(42),
///     line_mapping: None,
///     commit_history: vec![CommitSha::new("abc1234567890".to_owned())],
///     current_index: 0,
/// });
///
/// assert_eq!(state.file_path().as_str(), "src/auth.rs");
/// assert_eq!(state.original_line(), Some(42));
/// assert_eq!(state.commit_count(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct TimeTravelState {
    /// The commit snapshot being viewed.
    snapshot: CommitSnapshot,
    /// File path being viewed.
    file_path: RepoFilePath,
    /// Line number from the original comment.
    original_line: Option<u32>,
    /// Verification of line mapping to HEAD.
    line_mapping: Option<LineMappingVerification>,
    /// List of commit SHAs in the history (most recent first).
    commit_history: Vec<CommitSha>,
    /// Current index in the commit history.
    current_index: usize,
    /// Whether the state is currently loading.
    loading: bool,
    /// Error message if loading failed.
    error_message: Option<String>,
}

impl TimeTravelState {
    /// Creates a new time-travel state from initialization parameters.
    #[must_use]
    pub fn new(params: TimeTravelInitParams) -> Self {
        let commit_history = params.commit_history;
        let (current_index, error_message) = synchronize_snapshot_index(
            params.snapshot.sha(),
            &commit_history,
            params.current_index,
        );
        Self {
            snapshot: params.snapshot,
            file_path: params.file_path,
            original_line: params.original_line,
            line_mapping: params.line_mapping,
            commit_history,
            current_index,
            loading: false,
            error_message,
        }
    }

    /// Creates a loading placeholder state.
    #[must_use]
    pub(crate) fn loading(file_path: RepoFilePath, original_line: Option<u32>) -> Self {
        let metadata = CommitMetadata::new(
            String::new(),
            "Loading...".to_owned(),
            String::new(),
            chrono::Utc::now(),
        );
        Self {
            snapshot: CommitSnapshot::new(metadata),
            file_path,
            original_line,
            line_mapping: None,
            commit_history: Vec::new(),
            current_index: 0,
            loading: true,
            error_message: None,
        }
    }

    /// Creates an error state for in-crate tests.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn error(message: String, file_path: RepoFilePath) -> Self {
        let metadata = CommitMetadata::new(
            String::new(),
            String::new(),
            String::new(),
            chrono::Utc::now(),
        );
        Self {
            snapshot: CommitSnapshot::new(metadata),
            file_path,
            original_line: None,
            line_mapping: None,
            commit_history: Vec::new(),
            current_index: 0,
            loading: false,
            error_message: Some(message),
        }
    }

    /// Returns the current commit snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &CommitSnapshot {
        &self.snapshot
    }

    /// Returns the file path being viewed.
    #[must_use]
    pub const fn file_path(&self) -> &RepoFilePath {
        &self.file_path
    }

    /// Returns the original line number from the comment.
    #[must_use]
    pub const fn original_line(&self) -> Option<u32> {
        self.original_line
    }

    /// Returns the line mapping verification, if available.
    #[must_use]
    pub const fn line_mapping(&self) -> Option<&LineMappingVerification> {
        self.line_mapping.as_ref()
    }

    /// Returns the commit history ordered from most recent to oldest.
    #[must_use]
    pub fn commit_history(&self) -> &[CommitSha] {
        &self.commit_history
    }

    /// Returns the current index in the commit history.
    #[must_use]
    pub const fn current_index(&self) -> usize {
        self.current_index
    }

    /// Returns whether the state is currently loading.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Returns the error message, if any.
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Returns the total number of commits in history.
    #[must_use]
    pub const fn commit_count(&self) -> usize {
        self.commit_history.len()
    }

    /// Returns whether navigation to the previous commit is possible.
    #[must_use]
    pub const fn can_go_previous(&self) -> bool {
        !self.loading && self.current_index + 1 < self.commit_history.len()
    }

    /// Returns whether navigation to the next (more recent) commit is possible.
    #[must_use]
    pub const fn can_go_next(&self) -> bool {
        !self.loading && self.current_index > 0
    }

    /// Updates the state with a new snapshot after navigation.
    pub fn update_snapshot(
        &mut self,
        snapshot: CommitSnapshot,
        line_mapping: Option<LineMappingVerification>,
        new_index: usize,
    ) {
        let (current_index, error_message) =
            synchronize_snapshot_index(snapshot.sha(), &self.commit_history, new_index);
        self.snapshot = snapshot;
        self.line_mapping = line_mapping;
        self.current_index = current_index;
        self.loading = false;
        self.error_message = error_message;
    }

    /// Sets the loading state during in-crate orchestration.
    pub(crate) const fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// Sets an error message during in-crate orchestration.
    pub(crate) fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.loading = false;
    }

    /// Returns the SHA of the next (more recent) commit, if available.
    #[must_use]
    pub fn next_commit_sha(&self) -> Option<&CommitSha> {
        if self.current_index > 0 {
            self.commit_history.get(self.current_index - 1)
        } else {
            None
        }
    }

    /// Returns the SHA of the previous (older) commit, if available.
    #[must_use]
    pub fn previous_commit_sha(&self) -> Option<&CommitSha> {
        self.commit_history.get(self.current_index + 1)
    }
}

/// Clamps an index to valid bounds.
#[must_use]
fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        index.min(len.saturating_sub(1))
    }
}

/// Keeps the snapshot SHA and history index aligned for public callers.
#[must_use]
fn synchronize_snapshot_index(
    snapshot_sha: &str,
    commit_history: &[CommitSha],
    requested_index: usize,
) -> (usize, Option<String>) {
    commit_history
        .iter()
        .position(|commit_sha| commit_sha.as_str() == snapshot_sha)
        .map_or_else(
            || {
                let current_index = clamp_index(requested_index, commit_history.len());
                let error_message = commit_history.get(current_index).map(|expected_sha| {
                    format!(
                        "snapshot SHA {snapshot_sha} does not match commit history entry {} at index {current_index}",
                        expected_sha.as_str()
                    )
                });
                (current_index, error_message)
            },
            |index| (index, None),
        )
}

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;
