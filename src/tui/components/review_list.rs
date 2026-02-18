//! Review list component for displaying filtered review comments.
//!
//! This component renders a scrollable list of review comments with cursor
//! highlighting and displays relevant metadata for each comment.

use crate::github::models::ReviewComment;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Defensive fallback for visible height when layout has not yet been applied.
const FALLBACK_VISIBLE_HEIGHT: usize = 5;

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
    /// Maximum visible width in characters for row rendering.
    ///
    /// Rows longer than this width are truncated.
    pub max_width: usize,
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
            visible_height: FALLBACK_VISIBLE_HEIGHT,
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

        // Use context height when provided, otherwise fall back defensively.
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
            let line = Self::format_review_line(review, prefix, ctx.max_width);
            output.push_str(&line);
            output.push('\n');
        }

        output
    }

    /// Formats a single review line for display.
    fn format_review_line(review: &ReviewComment, prefix: &str, max_width: usize) -> String {
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

        let line = format!("{prefix} [{author}] {file}{line_num}: {body_preview}");
        truncate_to_width(&line, max_width)
    }
}

/// Truncates body text to a maximum length, adding ellipsis if needed.
fn truncate_body(body: &str, max_len: usize) -> String {
    // Take first line only and truncate
    let first_line = body.lines().next().unwrap_or("");
    let trimmed = first_line.trim();
    truncate_with_ellipsis_to_width(trimmed, max_len)
}

enum TruncationDecision {
    Empty,
    Unchanged,
    DotFallback,
    Ellipsis,
}

const fn is_zero_width(max_width: usize) -> bool {
    max_width == 0
}

fn fits_display_width(text: &str, max_width: usize) -> bool {
    text.width() <= max_width
}

const fn should_use_dot_fallback(max_width: usize) -> bool {
    max_width <= 3
}

fn truncation_decision(text: &str, max_width: usize) -> TruncationDecision {
    if is_zero_width(max_width) {
        TruncationDecision::Empty
    } else if fits_display_width(text, max_width) {
        TruncationDecision::Unchanged
    } else if should_use_dot_fallback(max_width) {
        TruncationDecision::DotFallback
    } else {
        TruncationDecision::Ellipsis
    }
}

/// Truncates text to the provided display width and appends an ellipsis.
fn truncate_with_ellipsis_to_width(text: &str, max_width: usize) -> String {
    match truncation_decision(text, max_width) {
        TruncationDecision::Empty => String::new(),
        TruncationDecision::Unchanged => text.to_owned(),
        TruncationDecision::DotFallback => {
            // Keep fallback dots deterministic for very small width budgets.
            ".".repeat(max_width)
        }
        TruncationDecision::Ellipsis => {
            let target_width = max_width.saturating_sub(3);
            let mut truncated = String::new();
            let mut current_width = 0;
            for ch in text.chars() {
                let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
                if current_width + char_width > target_width {
                    break;
                }
                truncated.push(ch);
                current_width += char_width;
            }
            format!("{truncated}...")
        }
    }
}

/// Truncates a row to a fixed width, preserving the caller's overflow budget.
fn truncate_to_width(text: &str, max_width: usize) -> String {
    truncate_with_ellipsis_to_width(text, max_width)
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use unicode_width::UnicodeWidthStr;

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
            max_width: 80,
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
            max_width: 80,
        };
        let output = component.view(&ctx);

        // First line should not have cursor
        assert!(output.contains("  [alice]"));
        // Second line should have cursor
        assert!(output.contains("> [bob]"));
    }

    #[rstest]
    fn format_review_line_includes_all_fields(sample_review: ReviewComment) {
        let line = ReviewListComponent::format_review_line(&sample_review, " ", 80);

        assert!(line.contains("[alice]"));
        assert!(line.contains("src/main.rs"));
        assert!(line.contains(":42"));
        assert!(line.contains("Consider refactoring"));
    }

    #[test]
    fn format_review_line_is_truncated_to_max_width() {
        let comment = ReviewComment {
            author: Some("averylongreviewername".to_owned()),
            body: Some("A long body that should not exceed the rendered row width".to_owned()),
            file_path: Some("a/very/long/path/that/will/exceed/the/width.rs".to_owned()),
            line_number: Some(123),
            id: 1,
            ..Default::default()
        };

        let line = ReviewListComponent::format_review_line(&comment, ">", 20);
        assert!(line.width() <= 20, "list rows should be clamped to width");
    }

    #[rstest]
    fn view_rows_respect_max_width(sample_review: ReviewComment) {
        let reviews = vec![sample_review];
        let filtered_indices = vec![0];
        let component = ReviewListComponent::new();
        let ctx = ReviewListViewContext {
            reviews: &reviews,
            filtered_indices: &filtered_indices,
            cursor_position: 0,
            scroll_offset: 0,
            visible_height: 10,
            max_width: 16,
        };
        let output = component.view(&ctx);
        let first_row = output.lines().next().unwrap_or("");

        assert_eq!(first_row.width(), 16);
    }

    #[test]
    fn truncate_body_shortens_long_text() {
        let long_text = "This is a very long comment that should be truncated";
        let truncated = truncate_body(long_text, 20);
        assert_eq!(truncated.width(), 20);
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

    #[test]
    fn truncate_body_uses_display_width_for_wide_characters() {
        let result = truncate_body("你好世界和平", 7);
        assert_eq!(result, "你好...");
        assert_eq!(result.width(), 7);
    }

    #[test]
    fn truncate_to_width_uses_display_width_for_wide_characters() {
        let result = truncate_to_width("你好世界和平", 7);
        assert_eq!(result, "你好...");
        assert_eq!(result.width(), 7);
    }
}
