//! Filter and cursor state for review listing.
//!
//! This module provides types for managing which reviews are displayed and
//! tracking the user's position within the filtered list. The design ensures
//! that cursor position is retained when filters change (clamped to valid range).

use crate::github::models::ReviewComment;

/// Filter criteria for the review listing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ReviewFilter {
    /// Show all review comments.
    #[default]
    All,
    /// Show only unresolved review comments (root comments without replies).
    ///
    /// A comment is considered unresolved if it meets both conditions:
    /// 1. It is a root comment (has no `in_reply_to_id`)
    /// 2. It has no replies (no other comment has `in_reply_to_id` pointing to it)
    ///
    /// This approximates "unresolved" as "unanswered thread starters". Comments
    /// that have received at least one reply are considered addressed.
    ///
    /// Note: This filter does not track GitHub's explicit "Resolve conversation"
    /// feature, which requires additional API data not currently fetched.
    Unresolved,
    /// Show only comments on a specific file path.
    ByFile(String),
    /// Show only comments from a specific reviewer.
    ByReviewer(String),
    /// Show only comments within a commit range.
    ByCommitRange {
        /// Starting commit SHA (exclusive).
        from: String,
        /// Ending commit SHA (inclusive).
        to: String,
    },
}

impl ReviewFilter {
    /// Returns a human-readable label for display in the UI.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::All => "All".to_owned(),
            Self::Unresolved => "Unresolved".to_owned(),
            Self::ByFile(path) => format!("File: {path}"),
            Self::ByReviewer(name) => format!("Reviewer: {name}"),
            Self::ByCommitRange { from, to } => {
                let from_short = truncate_sha(from);
                let to_short = truncate_sha(to);
                format!("Commits: {from_short}..{to_short}")
            }
        }
    }

    /// Returns true if this filter matches the given review comment.
    ///
    /// The `all_reviews` parameter is required for filters that need thread
    /// analysis (e.g., `Unresolved` checks if a comment has replies).
    #[must_use]
    pub fn matches(&self, review: &ReviewComment, all_reviews: &[ReviewComment]) -> bool {
        match self {
            Self::All => true,
            Self::Unresolved => {
                // A comment is unresolved if:
                // 1. It is a root comment (not a reply to another comment)
                // 2. It has no replies (no comment in the collection replies to it)
                let is_root = review.in_reply_to_id.is_none();
                if !is_root {
                    return false;
                }
                let has_replies = all_reviews
                    .iter()
                    .any(|r| r.in_reply_to_id == Some(review.id));
                !has_replies
            }
            Self::ByFile(path) => review.file_path.as_ref().is_some_and(|p| p == path),
            Self::ByReviewer(name) => review.author.as_ref().is_some_and(|a| a == name),
            Self::ByCommitRange { from, to } => {
                // For commit range filtering, we check if the commit_sha
                // matches either endpoint. Full range checking requires
                // commit ordering which is deferred to future implementation.
                review
                    .commit_sha
                    .as_ref()
                    .is_some_and(|sha| sha == from || sha == to)
            }
        }
    }
}

/// State managing the active filter and cursor position.
#[derive(Debug, Clone, Default)]
pub struct FilterState {
    /// Currently active filter.
    pub active_filter: ReviewFilter,
    /// Current cursor position (0-indexed) within the filtered list.
    pub cursor_position: usize,
    /// Scroll offset for virtual scrolling (lines scrolled from top).
    pub scroll_offset: usize,
}

impl FilterState {
    /// Creates a new filter state with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies the current filter to a list of reviews.
    ///
    /// Returns a vector of references to matching reviews.
    #[must_use]
    pub fn apply_filter<'a>(&self, reviews: &'a [ReviewComment]) -> Vec<&'a ReviewComment> {
        reviews
            .iter()
            .filter(|review| self.active_filter.matches(review, reviews))
            .collect()
    }

    /// Updates the filter and clamps the cursor to valid range.
    ///
    /// This method preserves the cursor position when possible, only adjusting
    /// it if the new filtered list is shorter than the current position.
    pub fn set_filter(&mut self, filter: ReviewFilter, new_count: usize) {
        self.active_filter = filter;
        self.clamp_cursor(new_count);
    }

    /// Clamps the cursor position to be within the valid range.
    ///
    /// If the list is empty, cursor is set to 0. If cursor exceeds the list
    /// length, it is set to the last valid index.
    pub const fn clamp_cursor(&mut self, count: usize) {
        if count == 0 {
            self.cursor_position = 0;
            self.scroll_offset = 0;
        } else if self.cursor_position >= count {
            self.cursor_position = count.saturating_sub(1);
        }
    }

    /// Moves the cursor up by one position if possible.
    pub const fn cursor_up(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1);
    }

    /// Moves the cursor down by one position if within bounds.
    pub const fn cursor_down(&mut self, max_index: usize) {
        if self.cursor_position < max_index {
            self.cursor_position = self.cursor_position.saturating_add(1);
        }
    }

    /// Moves the cursor up by a page (visible height).
    pub const fn page_up(&mut self, page_size: usize) {
        self.cursor_position = self.cursor_position.saturating_sub(page_size);
    }

    /// Moves the cursor down by a page (visible height).
    pub const fn page_down(&mut self, page_size: usize, max_index: usize) {
        let new_pos = self.cursor_position.saturating_add(page_size);
        self.cursor_position = if new_pos < max_index {
            new_pos
        } else {
            max_index
        };
    }

    /// Moves the cursor to the first item.
    pub const fn home(&mut self) {
        self.cursor_position = 0;
        self.scroll_offset = 0;
    }

    /// Moves the cursor to the last item.
    pub const fn end(&mut self, max_index: usize) {
        self.cursor_position = max_index;
    }
}

/// Truncates a SHA to first 7 characters for display.
///
/// SHA strings are ASCII hex digits, but we use `get()` for safety in case
/// the input contains unexpected characters.
fn truncate_sha(sha: &str) -> &str {
    const SHA_DISPLAY_LEN: usize = 7;
    sha.get(..SHA_DISPLAY_LEN).unwrap_or(sha)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_review(id: u64, author: Option<&str>, file: Option<&str>) -> ReviewComment {
        ReviewComment {
            id,
            body: Some("Test comment".to_owned()),
            author: author.map(ToOwned::to_owned),
            file_path: file.map(ToOwned::to_owned),
            line_number: Some(42),
            original_line_number: None,
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn filter_all_matches_everything() {
        let review = make_review(1, Some("alice"), Some("src/main.rs"));
        let reviews = [review.clone()];
        assert!(ReviewFilter::All.matches(&review, &reviews));
    }

    #[test]
    fn filter_by_file_matches_correct_path() {
        let review = make_review(1, Some("alice"), Some("src/main.rs"));
        let reviews = [review.clone()];
        assert!(ReviewFilter::ByFile("src/main.rs".to_owned()).matches(&review, &reviews));
        assert!(!ReviewFilter::ByFile("src/lib.rs".to_owned()).matches(&review, &reviews));
    }

    #[test]
    fn filter_by_reviewer_matches_correct_author() {
        let review = make_review(1, Some("alice"), Some("src/main.rs"));
        let reviews = [review.clone()];
        assert!(ReviewFilter::ByReviewer("alice".to_owned()).matches(&review, &reviews));
        assert!(!ReviewFilter::ByReviewer("bob".to_owned()).matches(&review, &reviews));
    }

    #[test]
    fn filter_unresolved_matches_unanswered_root_comments() {
        let root_unanswered = make_review(1, Some("alice"), Some("src/main.rs"));
        let root_answered = make_review(2, Some("alice"), Some("src/lib.rs"));
        let reply_to_root2 = ReviewComment {
            in_reply_to_id: Some(2),
            ..make_review(3, Some("bob"), Some("src/lib.rs"))
        };

        let reviews = [
            root_unanswered.clone(),
            root_answered.clone(),
            reply_to_root2.clone(),
        ];

        // Root comment with no replies is unresolved
        assert!(ReviewFilter::Unresolved.matches(&root_unanswered, &reviews));
        // Root comment with a reply is NOT unresolved (it has been answered)
        assert!(!ReviewFilter::Unresolved.matches(&root_answered, &reviews));
        // Reply comments are never considered unresolved
        assert!(!ReviewFilter::Unresolved.matches(&reply_to_root2, &reviews));
    }

    #[test]
    fn clamp_cursor_sets_to_zero_when_empty() {
        let mut state = FilterState {
            cursor_position: 5,
            ..FilterState::default()
        };
        state.clamp_cursor(0);
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn clamp_cursor_reduces_to_last_valid_index() {
        let mut state = FilterState {
            cursor_position: 10,
            ..FilterState::default()
        };
        state.clamp_cursor(5);
        assert_eq!(state.cursor_position, 4);
    }

    #[test]
    fn clamp_cursor_preserves_valid_position() {
        let mut state = FilterState {
            cursor_position: 3,
            ..FilterState::default()
        };
        state.clamp_cursor(10);
        assert_eq!(state.cursor_position, 3);
    }

    #[test]
    fn set_filter_changes_filter_and_clamps() {
        let mut state = FilterState {
            cursor_position: 10,
            active_filter: ReviewFilter::All,
            ..FilterState::default()
        };
        state.set_filter(ReviewFilter::Unresolved, 5);
        assert_eq!(state.active_filter, ReviewFilter::Unresolved);
        assert_eq!(state.cursor_position, 4);
    }

    #[test]
    fn apply_filter_returns_matching_reviews() {
        let reviews = vec![
            make_review(1, Some("alice"), Some("src/main.rs")),
            make_review(2, Some("bob"), Some("src/lib.rs")),
            make_review(3, Some("alice"), Some("src/lib.rs")),
        ];

        let state = FilterState {
            active_filter: ReviewFilter::ByReviewer("alice".to_owned()),
            ..FilterState::default()
        };

        let filtered = state.apply_filter(&reviews);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered.first().map(|r| r.id), Some(1));
        assert_eq!(filtered.get(1).map(|r| r.id), Some(3));
    }

    #[test]
    fn cursor_navigation_respects_bounds() {
        let mut state = FilterState {
            cursor_position: 5,
            ..FilterState::default()
        };

        state.cursor_up();
        assert_eq!(state.cursor_position, 4);

        state.cursor_position = 0;
        state.cursor_up();
        assert_eq!(state.cursor_position, 0); // Cannot go below 0

        state.cursor_down(10);
        assert_eq!(state.cursor_position, 1);

        state.cursor_position = 10;
        state.cursor_down(10);
        assert_eq!(state.cursor_position, 10); // Cannot exceed max
    }

    #[test]
    fn filter_label_is_human_readable() {
        assert_eq!(ReviewFilter::All.label(), "All");
        assert_eq!(ReviewFilter::Unresolved.label(), "Unresolved");
        assert_eq!(
            ReviewFilter::ByFile("src/main.rs".to_owned()).label(),
            "File: src/main.rs"
        );
        assert_eq!(
            ReviewFilter::ByReviewer("alice".to_owned()).label(),
            "Reviewer: alice"
        );
        assert_eq!(
            ReviewFilter::ByCommitRange {
                from: "abc123456789".to_owned(),
                to: "def987654321".to_owned()
            }
            .label(),
            "Commits: abc1234..def9876"
        );
    }
}
