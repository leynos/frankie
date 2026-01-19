//! Time-travel state for navigating PR history.
//!
//! This module provides state management for the time-travel feature, which
//! allows users to view the exact code state when a comment was made and
//! verify line mapping correctness against git2 diffs.

use crate::github::models::ReviewComment;
use crate::local::{CommitSnapshot, LineMappingVerification};

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
    /// Creates a new time-travel state.
    #[must_use]
    #[doc(hidden)]
    #[expect(
        clippy::too_many_arguments,
        reason = "All parameters required for complete state"
    )]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Option<T> destructuring is not const-stable"
    )]
    pub fn new(
        snapshot: CommitSnapshot,
        file_path: String,
        original_line: Option<u32>,
        line_mapping: Option<LineMappingVerification>,
        commit_history: Vec<String>,
    ) -> Self {
        Self {
            snapshot,
            file_path,
            original_line,
            line_mapping,
            commit_history,
            current_index: 0,
            loading: false,
            error_message: None,
        }
    }

    /// Creates a loading placeholder state.
    #[must_use]
    pub(crate) fn loading(file_path: String, original_line: Option<u32>) -> Self {
        Self {
            snapshot: CommitSnapshot::new(
                String::new(),
                "Loading...".to_owned(),
                String::new(),
                chrono::Utc::now(),
            ),
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
        Self {
            snapshot: CommitSnapshot::new(
                String::new(),
                String::new(),
                String::new(),
                chrono::Utc::now(),
            ),
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
#[expect(clippy::unwrap_used, reason = "Tests panic on failure")]
mod tests {
    use chrono::Utc;
    use rstest::{fixture, rstest};

    use super::*;
    use crate::github::models::test_support::minimal_review;
    use crate::local::LineMappingStatus;

    #[fixture]
    fn sample_snapshot() -> CommitSnapshot {
        CommitSnapshot::with_file_content(
            "abc1234567890".to_owned(),
            "Fix login bug".to_owned(),
            "Alice".to_owned(),
            Utc::now(),
            "src/auth.rs".to_owned(),
            "fn login() {}".to_owned(),
        )
    }

    #[fixture]
    fn sample_history() -> Vec<String> {
        vec![
            "abc1234567890".to_owned(),
            "def5678901234".to_owned(),
            "ghi9012345678".to_owned(),
        ]
    }

    #[rstest]
    fn new_state_initialised(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
        let state = TimeTravelState::new(
            sample_snapshot.clone(),
            "src/auth.rs".to_owned(),
            Some(42),
            None,
            sample_history.clone(),
        );

        assert_eq!(state.snapshot().sha(), sample_snapshot.sha());
        assert_eq!(state.file_path(), "src/auth.rs");
        assert_eq!(state.original_line(), Some(42));
        assert!(state.line_mapping().is_none());
        assert_eq!(state.commit_count(), 3);
        assert_eq!(state.current_index(), 0);
        assert!(!state.is_loading());
        assert!(state.error_message().is_none());
    }

    #[rstest]
    fn loading_state() {
        let state = TimeTravelState::loading("src/main.rs".to_owned(), Some(10));

        assert!(state.is_loading());
        assert_eq!(state.snapshot().message(), "Loading...");
        assert_eq!(state.file_path(), "src/main.rs");
        assert_eq!(state.original_line(), Some(10));
    }

    #[rstest]
    fn error_state() {
        let state = TimeTravelState::error("Commit not found".to_owned(), "src/lib.rs".to_owned());

        assert!(!state.is_loading());
        assert_eq!(state.error_message(), Some("Commit not found"));
    }

    #[rstest]
    fn navigation_available(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
        let state = TimeTravelState::new(
            sample_snapshot,
            "src/auth.rs".to_owned(),
            None,
            None,
            sample_history,
        );

        // At index 0 (most recent): can go previous, cannot go next
        assert!(state.can_go_previous());
        assert!(!state.can_go_next());

        assert_eq!(state.next_commit_sha(), None);
        assert_eq!(state.previous_commit_sha(), Some("def5678901234"));
    }

    #[rstest]
    fn navigation_at_middle(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
        let mut state = TimeTravelState::new(
            sample_snapshot.clone(),
            "src/auth.rs".to_owned(),
            None,
            None,
            sample_history,
        );

        state.update_snapshot(sample_snapshot, None, 1);

        // At index 1 (middle): can go both ways
        assert!(state.can_go_previous());
        assert!(state.can_go_next());

        assert_eq!(state.next_commit_sha(), Some("abc1234567890"));
        assert_eq!(state.previous_commit_sha(), Some("ghi9012345678"));
    }

    #[rstest]
    fn navigation_at_oldest(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
        let mut state = TimeTravelState::new(
            sample_snapshot.clone(),
            "src/auth.rs".to_owned(),
            None,
            None,
            sample_history,
        );

        state.update_snapshot(sample_snapshot, None, 2);

        // At index 2 (oldest): cannot go previous, can go next
        assert!(!state.can_go_previous());
        assert!(state.can_go_next());
    }

    #[rstest]
    fn loading_blocks_navigation(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
        let mut state = TimeTravelState::new(
            sample_snapshot,
            "src/auth.rs".to_owned(),
            None,
            None,
            sample_history,
        );

        state.set_loading(true);

        assert!(!state.can_go_previous());
        assert!(!state.can_go_next());
    }

    #[rstest]
    fn update_snapshot_clamps_index(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
        let mut state = TimeTravelState::new(
            sample_snapshot.clone(),
            "src/auth.rs".to_owned(),
            None,
            None,
            sample_history,
        );

        // Try to update with an out-of-bounds index
        state.update_snapshot(sample_snapshot, None, 100);

        assert_eq!(state.current_index(), 2); // Clamped to last index
    }

    #[rstest]
    fn params_from_comment_full() {
        let comment = ReviewComment {
            commit_sha: Some("abc123".to_owned()),
            file_path: Some("src/main.rs".to_owned()),
            line_number: Some(42),
            original_line_number: Some(40),
            ..minimal_review(1, "Test comment", "alice")
        };

        let params = TimeTravelParams::from_comment(&comment).unwrap();

        assert_eq!(params.commit_sha, "abc123");
        assert_eq!(params.file_path, "src/main.rs");
        assert_eq!(params.line_number, Some(42)); // Prefers line_number
    }

    #[rstest]
    fn params_from_comment_original_line() {
        let comment = ReviewComment {
            commit_sha: Some("abc123".to_owned()),
            file_path: Some("src/main.rs".to_owned()),
            line_number: None,
            original_line_number: Some(40),
            ..minimal_review(1, "Test comment", "alice")
        };

        let params = TimeTravelParams::from_comment(&comment).unwrap();

        assert_eq!(params.line_number, Some(40)); // Falls back to original_line_number
    }

    #[rstest]
    fn params_from_comment_missing_sha() {
        let comment = ReviewComment {
            commit_sha: None,
            file_path: Some("src/main.rs".to_owned()),
            ..minimal_review(1, "Test comment", "alice")
        };

        assert!(TimeTravelParams::from_comment(&comment).is_none());
    }

    #[rstest]
    fn params_from_comment_missing_path() {
        let comment = ReviewComment {
            commit_sha: Some("abc123".to_owned()),
            file_path: None,
            ..minimal_review(1, "Test comment", "alice")
        };

        assert!(TimeTravelParams::from_comment(&comment).is_none());
    }

    #[rstest]
    fn line_mapping_stored(sample_snapshot: CommitSnapshot, sample_history: Vec<String>) {
        let mapping = LineMappingVerification::moved(42, 50);
        let state = TimeTravelState::new(
            sample_snapshot,
            "src/auth.rs".to_owned(),
            Some(42),
            Some(mapping.clone()),
            sample_history,
        );

        let stored = state.line_mapping().unwrap();
        assert_eq!(stored.status(), LineMappingStatus::Moved);
        assert_eq!(stored.original_line(), 42);
        assert_eq!(stored.current_line(), Some(50));
    }

    #[test]
    fn clamp_index_empty() {
        assert_eq!(clamp_index(0, 0), 0);
        assert_eq!(clamp_index(5, 0), 0);
    }

    #[test]
    fn clamp_index_normal() {
        assert_eq!(clamp_index(0, 3), 0);
        assert_eq!(clamp_index(2, 3), 2);
        assert_eq!(clamp_index(5, 3), 2);
    }
}
