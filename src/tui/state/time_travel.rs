//! Time-travel state for navigating PR history.
//!
//! This module provides state management for the time-travel feature, which
//! allows users to view the exact code state when a comment was made and
//! verify line mapping correctness against git2 diffs.

use crate::github::models::ReviewComment;
use crate::local::{CommitMetadata, CommitSnapshot, LineMappingVerification};

/// Parameters for initialising a time-travel state.
#[derive(Debug, Clone)]
pub struct TimeTravelInitParams {
    /// The commit snapshot to display.
    pub snapshot: CommitSnapshot,
    /// Path to the file being viewed.
    pub file_path: String,
    /// Line number from the original comment.
    pub original_line: Option<u32>,
    /// Verification of line mapping to HEAD.
    pub line_mapping: Option<LineMappingVerification>,
    /// List of commit SHAs in the history (most recent first).
    pub commit_history: Vec<String>,
    /// Current position in the commit history.
    pub current_index: usize,
}

/// State container for time-travel navigation.
#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct TimeTravelState {
    /// The commit snapshot being viewed.
    snapshot: CommitSnapshot,
    /// File path being viewed.
    file_path: String,
    /// Line number from the original comment.
    original_line: Option<u32>,
    /// Verification of line mapping to HEAD.
    line_mapping: Option<LineMappingVerification>,
    /// List of commit SHAs in the history (most recent first).
    commit_history: Vec<String>,
    /// Current index in the commit history.
    current_index: usize,
    /// Whether the state is currently loading.
    loading: bool,
    /// Error message if loading failed.
    error_message: Option<String>,
}

impl TimeTravelState {
    /// Creates a new time-travel state from initialisation parameters.
    #[must_use]
    #[doc(hidden)]
    pub fn new(params: TimeTravelInitParams) -> Self {
        Self {
            snapshot: params.snapshot,
            file_path: params.file_path,
            original_line: params.original_line,
            line_mapping: params.line_mapping,
            commit_history: params.commit_history,
            current_index: params.current_index,
            loading: false,
            error_message: None,
        }
    }

    /// Creates a loading placeholder state.
    #[must_use]
    pub(crate) fn loading(file_path: String, original_line: Option<u32>) -> Self {
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

    /// Creates an error state.
    #[must_use]
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Used in tests and planned for error handling")
    )]
    pub(crate) fn error(message: String, file_path: String) -> Self {
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
    pub(crate) const fn snapshot(&self) -> &CommitSnapshot {
        &self.snapshot
    }

    /// Returns the file path being viewed.
    #[must_use]
    pub(crate) fn file_path(&self) -> &str {
        &self.file_path
    }

    /// Returns the original line number from the comment.
    #[must_use]
    pub(crate) const fn original_line(&self) -> Option<u32> {
        self.original_line
    }

    /// Returns the line mapping verification, if available.
    #[must_use]
    pub(crate) const fn line_mapping(&self) -> Option<&LineMappingVerification> {
        self.line_mapping.as_ref()
    }

    /// Returns the commit history.
    #[must_use]
    pub(crate) fn commit_history(&self) -> &[String] {
        &self.commit_history
    }

    /// Returns the current index in the commit history.
    #[must_use]
    pub(crate) const fn current_index(&self) -> usize {
        self.current_index
    }

    /// Returns whether the state is currently loading.
    #[must_use]
    pub(crate) const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Returns the error message, if any.
    #[must_use]
    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Returns the total number of commits in history.
    #[must_use]
    #[expect(clippy::missing_const_for_fn, reason = "Vec::len is not const-stable")]
    pub(crate) fn commit_count(&self) -> usize {
        self.commit_history.len()
    }

    /// Returns whether navigation to the previous commit is possible.
    #[must_use]
    #[expect(clippy::missing_const_for_fn, reason = "Vec::len is not const-stable")]
    pub(crate) fn can_go_previous(&self) -> bool {
        !self.loading && self.current_index + 1 < self.commit_history.len()
    }

    /// Returns whether navigation to the next (more recent) commit is possible.
    #[must_use]
    pub(crate) const fn can_go_next(&self) -> bool {
        !self.loading && self.current_index > 0
    }

    /// Updates the state with a new snapshot after navigation.
    #[doc(hidden)]
    pub fn update_snapshot(
        &mut self,
        snapshot: CommitSnapshot,
        line_mapping: Option<LineMappingVerification>,
        new_index: usize,
    ) {
        self.snapshot = snapshot;
        self.line_mapping = line_mapping;
        self.current_index = clamp_index(new_index, self.commit_history.len());
        self.loading = false;
        self.error_message = None;
    }

    /// Sets the loading state.
    pub(crate) const fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// Sets an error message.
    pub(crate) fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.loading = false;
    }

    /// Returns the SHA of the next (more recent) commit, if available.
    #[must_use]
    pub(crate) fn next_commit_sha(&self) -> Option<&str> {
        if self.current_index > 0 {
            self.commit_history
                .get(self.current_index - 1)
                .map(String::as_str)
        } else {
            None
        }
    }

    /// Returns the SHA of the previous (older) commit, if available.
    #[must_use]
    pub(crate) fn previous_commit_sha(&self) -> Option<&str> {
        self.commit_history
            .get(self.current_index + 1)
            .map(String::as_str)
    }
}

/// Parameters for creating a time-travel state from a review comment.
#[derive(Debug, Clone)]
pub(crate) struct TimeTravelParams {
    /// The commit SHA where the comment was made.
    pub(crate) commit_sha: String,
    /// Path to the file.
    pub(crate) file_path: String,
    /// Line number in the file.
    pub(crate) line_number: Option<u32>,
}

impl TimeTravelParams {
    /// Extracts time-travel parameters from a review comment.
    ///
    /// Returns `None` if the comment doesn't have the required fields.
    #[must_use]
    pub(crate) fn from_comment(comment: &ReviewComment) -> Option<Self> {
        let commit_sha = comment.commit_sha.as_ref()?;
        let file_path = comment.file_path.as_ref()?;

        Some(Self {
            commit_sha: commit_sha.clone(),
            file_path: file_path.clone(),
            line_number: comment.line_number.or(comment.original_line_number),
        })
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

#[cfg(test)]
#[path = "time_travel/tests.rs"]
mod tests;
