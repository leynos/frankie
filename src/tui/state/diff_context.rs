//! Diff context state and helpers for full-screen diff navigation.
//!
//! This module provides data structures for collecting diff hunks from review
//! comments and tracking the current hunk in a full-screen diff view.

use std::collections::HashSet;

use crate::github::models::ReviewComment;

/// A single diff hunk extracted from a review comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiffHunk {
    /// File path associated with the hunk, if known.
    pub(crate) file_path: Option<String>,
    /// Line number associated with the hunk, if available.
    pub(crate) line_number: Option<u32>,
    /// Raw diff hunk text.
    pub(crate) text: String,
}

/// A rendered diff hunk with pre-formatted output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedDiffHunk {
    /// The raw diff hunk metadata.
    pub(crate) hunk: DiffHunk,
    /// Pre-rendered diff body string.
    pub(crate) rendered: String,
}

/// Index into the diff hunk list.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HunkIndex(usize);

impl HunkIndex {
    /// Creates a new index from a zero-based value.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::state::HunkIndex;
    ///
    /// let index = HunkIndex::new(2);
    /// assert_eq!(index.value(), 2);
    /// ```
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the underlying zero-based index.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::state::HunkIndex;
    ///
    /// let index = HunkIndex::new(1);
    /// assert_eq!(index.value(), 1);
    /// ```
    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }

    /// Clamps the index to the valid range for a given length.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::state::HunkIndex;
    ///
    /// let index = HunkIndex::new(10).clamp(3);
    /// assert_eq!(index.value(), 2);
    /// ```
    #[must_use]
    pub const fn clamp(self, len: usize) -> Self {
        if len == 0 {
            return Self(0);
        }
        if self.0 >= len {
            return Self(len.saturating_sub(1));
        }
        Self(self.0)
    }
}

/// State container for the full-screen diff context view.
#[derive(Debug, Default)]
pub(crate) struct DiffContextState {
    hunks: Vec<RenderedDiffHunk>,
    current_index: HunkIndex,
    cached_width: usize,
}

impl DiffContextState {
    /// Replaces the current hunk list and selection.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::state::{DiffContextState, HunkIndex, RenderedDiffHunk};
    ///
    /// let mut state = DiffContextState::default();
    /// state.rebuild(Vec::<RenderedDiffHunk>::new(), 80, HunkIndex::new(0));
    /// assert!(state.hunks().is_empty());
    /// ```
    pub(crate) fn rebuild(
        &mut self,
        hunks: Vec<RenderedDiffHunk>,
        cached_width: usize,
        preferred_index: HunkIndex,
    ) {
        self.hunks = hunks;
        self.cached_width = cached_width;
        self.current_index = preferred_index.clamp(self.hunks.len());
    }

    /// Returns the rendered hunks.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::state::DiffContextState;
    ///
    /// let state = DiffContextState::default();
    /// assert!(state.hunks().is_empty());
    /// ```
    #[must_use]
    pub(crate) fn hunks(&self) -> &[RenderedDiffHunk] {
        &self.hunks
    }

    /// Returns the current index.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::state::DiffContextState;
    ///
    /// let state = DiffContextState::default();
    /// assert_eq!(state.current_index().value(), 0);
    /// ```
    #[must_use]
    pub(crate) const fn current_index(&self) -> HunkIndex {
        self.current_index
    }

    /// Returns the cached width used for rendering.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::state::DiffContextState;
    ///
    /// let state = DiffContextState::default();
    /// assert_eq!(state.cached_width(), 0);
    /// ```
    #[must_use]
    pub(crate) const fn cached_width(&self) -> usize {
        self.cached_width
    }

    /// Moves to the next hunk, clamping at the last hunk.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::state::DiffContextState;
    ///
    /// let mut state = DiffContextState::default();
    /// state.move_next();
    /// ```
    pub(crate) fn move_next(&mut self) {
        if self.hunks.is_empty() {
            return;
        }
        let max_index = self.hunks.len().saturating_sub(1);
        let next = self.current_index.value().saturating_add(1).min(max_index);
        self.current_index = HunkIndex::new(next);
    }

    /// Moves to the previous hunk, clamping at the first hunk.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use frankie::tui::state::DiffContextState;
    ///
    /// let mut state = DiffContextState::default();
    /// state.move_previous();
    /// ```
    pub(crate) const fn move_previous(&mut self) {
        if self.hunks.is_empty() {
            return;
        }
        let prev = self.current_index.value().saturating_sub(1);
        self.current_index = HunkIndex::new(prev);
    }
}

/// Collects diff hunks from the filtered review comments.
///
/// Hunks are de-duplicated by `(file_path, hunk text)` identity and ordered by
/// file path (ascending) and line number (ascending). Comments without a
/// `diff_hunk` are ignored.
///
/// # Examples
///
/// ```rust,ignore
/// use frankie::tui::state::collect_diff_hunks;
/// use frankie::github::models::ReviewComment;
///
/// let reviews: Vec<ReviewComment> = Vec::new();
/// let indices: Vec<usize> = Vec::new();
/// let hunks = collect_diff_hunks(&reviews, &indices);
/// assert!(hunks.is_empty());
/// ```
#[must_use]
pub(crate) fn collect_diff_hunks(
    reviews: &[ReviewComment],
    filtered_indices: &[usize],
) -> Vec<DiffHunk> {
    let mut seen = HashSet::new();
    let mut hunks = Vec::new();

    for &index in filtered_indices {
        let Some(comment) = reviews.get(index) else {
            continue;
        };
        let Some(diff_hunk) = comment.diff_hunk.as_deref() else {
            continue;
        };
        if diff_hunk.trim().is_empty() {
            continue;
        }

        let key = DiffHunkKey {
            file_path: comment.file_path.clone(),
            text: diff_hunk.to_owned(),
        };

        if seen.insert(key.clone()) {
            hunks.push(DiffHunk {
                file_path: key.file_path,
                line_number: comment.line_number,
                text: key.text,
            });
        }
    }

    hunks.sort_by(|left, right| {
        let left_path = left.file_path.as_deref().unwrap_or("");
        let right_path = right.file_path.as_deref().unwrap_or("");
        left_path
            .cmp(right_path)
            .then_with(|| left.line_number.cmp(&right.line_number))
    });

    hunks
}

/// Finds the best starting index for a selected comment.
///
/// # Examples
///
/// ```rust,ignore
/// use frankie::tui::state::{find_hunk_index, DiffHunk, HunkIndex};
/// use frankie::github::models::ReviewComment;
///
/// let hunks: Vec<DiffHunk> = Vec::new();
/// let index = find_hunk_index(&hunks, None);
/// assert_eq!(index, HunkIndex::new(0));
/// ```
#[must_use]
pub(crate) fn find_hunk_index(
    hunks: &[DiffHunk],
    selected_comment: Option<&ReviewComment>,
) -> HunkIndex {
    let Some(comment) = selected_comment else {
        return HunkIndex::new(0);
    };

    let Some(diff_hunk) = comment.diff_hunk.as_deref() else {
        return HunkIndex::new(0);
    };

    let position = hunks.iter().position(|hunk| {
        hunk.text == diff_hunk && hunk.file_path.as_deref() == comment.file_path.as_deref()
    });

    HunkIndex::new(position.unwrap_or(0))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DiffHunkKey {
    file_path: Option<String>,
    text: String,
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;
    use crate::github::models::test_support::minimal_review;

    #[fixture]
    fn reviews() -> Vec<ReviewComment> {
        vec![
            ReviewComment {
                file_path: Some("src/lib.rs".to_owned()),
                line_number: Some(10),
                diff_hunk: Some("@@ -1 +1 @@\n+fn a() {}".to_owned()),
                ..minimal_review(1, "First", "alice")
            },
            ReviewComment {
                file_path: Some("src/lib.rs".to_owned()),
                line_number: Some(20),
                diff_hunk: Some("@@ -1 +1 @@\n+fn a() {}".to_owned()),
                ..minimal_review(2, "Second", "bob")
            },
            ReviewComment {
                file_path: Some("src/main.rs".to_owned()),
                line_number: Some(5),
                diff_hunk: Some("@@ -1 +1 @@\n+fn b() {}".to_owned()),
                ..minimal_review(3, "Third", "cara")
            },
            ReviewComment {
                file_path: None,
                line_number: None,
                diff_hunk: None,
                ..minimal_review(4, "Fourth", "drew")
            },
        ]
    }

    #[rstest]
    fn collect_diff_hunks_deduplicates(reviews: Vec<ReviewComment>) {
        let indices = vec![0, 1, 2, 3];
        let hunks = collect_diff_hunks(&reviews, &indices);

        assert_eq!(hunks.len(), 2, "expected deduplicated hunks");
    }

    #[rstest]
    fn collect_diff_hunks_sorts_by_path_then_line(reviews: Vec<ReviewComment>) {
        let indices = vec![2, 0];
        let hunks = collect_diff_hunks(&reviews, &indices);

        let first = hunks.first().expect("expected at least one hunk");
        let second = hunks.get(1).expect("expected two hunks");
        assert_eq!(first.file_path.as_deref(), Some("src/lib.rs"));
        assert_eq!(second.file_path.as_deref(), Some("src/main.rs"));
    }

    #[rstest]
    fn find_hunk_index_uses_selected_comment(reviews: Vec<ReviewComment>) {
        let indices = vec![0, 2];
        let hunks = collect_diff_hunks(&reviews, &indices);
        let selected = reviews.get(2);

        let index = find_hunk_index(&hunks, selected);

        assert_eq!(index.value(), 1);
    }

    #[test]
    fn hunk_index_clamps_to_length() {
        let index = HunkIndex::new(5).clamp(2);
        assert_eq!(index.value(), 1);
    }
}
