//! Review list component for displaying filtered review comments.
//!
//! This component renders a scrollable list of review comments with cursor
//! highlighting and displays relevant metadata for each comment.

use crate::github::models::ReviewComment;

/// Default visible height for the review list component.
const DEFAULT_VISIBLE_HEIGHT: usize = 20;

/// Context for rendering the review list view.
///
/// Bundles the data needed to render a filtered list of reviews without
/// requiring per-frame allocations.
#[derive(Debug, Clone)]
pub struct ReviewListViewContext<'a> {
    /// Full slice of all review comments.
    pub reviews: &'a [ReviewComment],
    /// Indices of reviews matching the current filter.
    pub filtered_indices: &'a [usize],
    /// Current cursor position (0-indexed).
    pub cursor_position: usize,
    /// Number of lines scrolled from top.
    pub scroll_offset: usize,
    /// Maximum visible height in lines (for layout calculations).
    pub visible_height: usize,
}

/// Component for displaying a list of review comments.
#[derive(Debug, Clone)]
pub struct ReviewListComponent {
    /// Visible height in lines (for scrolling calculations).
    visible_height: usize,
}

impl Default for ReviewListComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewListComponent {
    /// Creates a new review list component.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            visible_height: DEFAULT_VISIBLE_HEIGHT,
        }
    }

    /// Updates the visible height for scrolling calculations.
    pub const fn set_visible_height(&mut self, height: usize) {
        self.visible_height = height;
    }

    /// Returns the visible height.
    #[must_use]
    pub const fn visible_height(&self) -> usize {
        self.visible_height
    }

    /// Renders the review list as a string.
    ///
    /// Only renders reviews within the visible window (based on scroll offset
    /// and visible height) for performance with large lists.
    ///
    /// This method accepts a context containing the full reviews slice and
    /// filtered indices, avoiding per-frame allocation.
    #[must_use]
    pub fn view(&self, ctx: &ReviewListViewContext<'_>) -> String {
        if ctx.filtered_indices.is_empty() {
            return "  No review comments match the current filter.\n".to_owned();
        }

        let mut output = String::new();

        // Use context's visible_height, falling back to component's default
        let visible_height = if ctx.visible_height > 0 {
            ctx.visible_height
        } else {
            self.visible_height
        };

        // Calculate visible range based on scroll offset and visible height
        let start = ctx.scroll_offset;
        let end = (ctx.scroll_offset + visible_height).min(ctx.filtered_indices.len());

        for (display_index, &review_index) in ctx
            .filtered_indices
            .iter()
            .enumerate()
            .skip(start)
            .take(end - start)
        {
            let Some(review) = ctx.reviews.get(review_index) else {
                continue;
            };
            let is_selected = display_index == ctx.cursor_position;
            let prefix = if is_selected { ">" } else { " " };
            let line = Self::format_review_line(review, prefix);
            output.push_str(&line);
            output.push('\n');
        }

        output
    }

    /// Formats a single review line for display.
    fn format_review_line(review: &ReviewComment, prefix: &str) -> String {
        let author = review.author.as_deref().unwrap_or("unknown");
        let file = review.file_path.as_deref().unwrap_or("(no file)");
        let line_num = review
            .line_number
            .map_or_else(String::new, |n| format!(":{n}"));

        let body_preview = review
            .body
            .as_ref()
            .map(|b| truncate_body(b, 50))
            .unwrap_or_default();

        format!("{prefix} [{author}] {file}{line_num}: {body_preview}")
    }
}

/// Truncates body text to a maximum length, adding ellipsis if needed.
fn truncate_body(body: &str, max_len: usize) -> String {
    // Take first line only and truncate
    let first_line = body.lines().next().unwrap_or("");
    let trimmed = first_line.trim();

    if trimmed.len() <= max_len {
        trimmed.to_owned()
    } else if let Some(truncated) = trimmed.get(..max_len.saturating_sub(3)) {
        format!("{truncated}...")
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;

    /// Builder for creating test review comments with struct initialization syntax.
    #[derive(Default)]
    struct ReviewBuilder {
        id: u64,
        author: Option<String>,
        file_path: Option<String>,
        line_number: Option<u32>,
        body: Option<String>,
    }

    impl ReviewBuilder {
        fn build(self) -> ReviewComment {
            ReviewComment {
                id: self.id,
                body: self.body,
                author: self.author,
                file_path: self.file_path,
                line_number: self.line_number,
                original_line_number: None,
                diff_hunk: None,
                commit_sha: None,
                in_reply_to_id: None,
                created_at: None,
                updated_at: None,
            }
        }
    }

    #[fixture]
    fn two_reviews() -> Vec<ReviewComment> {
        vec![
            ReviewBuilder {
                id: 1,
                author: Some("alice".to_owned()),
                file_path: Some("src/main.rs".to_owned()),
                line_number: Some(10),
                body: Some("Fix this".to_owned()),
            }
            .build(),
            ReviewBuilder {
                id: 2,
                author: Some("bob".to_owned()),
                file_path: Some("src/lib.rs".to_owned()),
                line_number: Some(20),
                body: Some("Looks good".to_owned()),
            }
            .build(),
        ]
    }

    #[fixture]
    fn sample_review() -> ReviewComment {
        ReviewBuilder {
            id: 1,
            author: Some("alice".to_owned()),
            file_path: Some("src/main.rs".to_owned()),
            line_number: Some(42),
            body: Some("Consider refactoring".to_owned()),
        }
        .build()
    }

    #[test]
    fn view_shows_empty_message_when_no_reviews() {
        let component = ReviewListComponent::new();
        let reviews: Vec<ReviewComment> = vec![];
        let filtered_indices: Vec<usize> = vec![];
        let ctx = ReviewListViewContext {
            reviews: &reviews,
            filtered_indices: &filtered_indices,
            cursor_position: 0,
            scroll_offset: 0,
            visible_height: 10,
        };
        let output = component.view(&ctx);
        assert!(output.contains("No review comments"));
    }

    #[rstest]
    fn view_shows_cursor_indicator(two_reviews: Vec<ReviewComment>) {
        let filtered_indices = vec![0, 1];

        let component = ReviewListComponent::new();
        let ctx = ReviewListViewContext {
            reviews: &two_reviews,
            filtered_indices: &filtered_indices,
            cursor_position: 1,
            scroll_offset: 0,
            visible_height: 10,
        };
        let output = component.view(&ctx);

        // First line should not have cursor
        assert!(output.contains("  [alice]"));
        // Second line should have cursor
        assert!(output.contains("> [bob]"));
    }

    #[rstest]
    fn format_review_line_includes_all_fields(sample_review: ReviewComment) {
        let line = ReviewListComponent::format_review_line(&sample_review, " ");

        assert!(line.contains("[alice]"));
        assert!(line.contains("src/main.rs"));
        assert!(line.contains(":42"));
        assert!(line.contains("Consider refactoring"));
    }

    #[test]
    fn truncate_body_shortens_long_text() {
        let long_text = "This is a very long comment that should be truncated";
        let truncated = truncate_body(long_text, 20);
        assert_eq!(truncated.len(), 20);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn truncate_body_preserves_short_text() {
        let short_text = "Short";
        let result = truncate_body(short_text, 20);
        assert_eq!(result, "Short");
    }

    #[test]
    fn truncate_body_takes_first_line_only() {
        let multiline = "First line\nSecond line\nThird line";
        let result = truncate_body(multiline, 50);
        assert_eq!(result, "First line");
    }
}
