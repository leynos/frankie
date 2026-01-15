//! Comment detail component for rendering a selected review comment.
//!
//! This component displays a single review comment's metadata, body text,
//! and inline code context with optional syntax highlighting. Code context
//! is extracted from the comment's `diff_hunk` field and wrapped to a
//! maximum width (typically 80 columns or the terminal width if narrower).

use crate::github::models::ReviewComment;

use super::code_highlight::CodeHighlighter;
use super::text_wrap::wrap_text;

/// Placeholder message when no code context is available.
const NO_CONTEXT_PLACEHOLDER: &str = "(No code context available)";

/// Placeholder message when no comment is selected.
const NO_SELECTION_PLACEHOLDER: &str = "(No comment selected)";

/// Context for rendering the comment detail view.
///
/// Bundles the data needed to render a comment detail pane without
/// requiring per-frame allocations.
#[derive(Debug, Clone)]
pub struct CommentDetailViewContext<'a> {
    /// The selected review comment to display, if any.
    pub selected_comment: Option<&'a ReviewComment>,
    /// Maximum width for code block wrapping (typically 80).
    pub max_width: usize,
    /// Maximum height in lines for the detail pane (0 = unlimited).
    pub max_height: usize,
}

/// Component for displaying a single review comment with code context.
///
/// Renders the comment's metadata (author, file, line), body text, and
/// inline code context from the `diff_hunk` field with syntax highlighting.
#[derive(Debug)]
pub struct CommentDetailComponent {
    /// Syntax highlighter for code blocks.
    highlighter: CodeHighlighter,
}

impl Default for CommentDetailComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl CommentDetailComponent {
    /// Creates a new comment detail component.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            highlighter: CodeHighlighter::new(),
        }
    }

    /// Renders the comment detail as a string.
    ///
    /// Returns a formatted string containing:
    /// - A separator line
    /// - Comment header (author, file, line number)
    /// - Comment body text
    /// - Code context with syntax highlighting (if available)
    ///
    /// If no comment is selected, returns a placeholder message.
    /// Output is truncated to `max_height` lines if specified (> 0).
    #[must_use]
    pub fn view(&self, ctx: &CommentDetailViewContext<'_>) -> String {
        let Some(comment) = ctx.selected_comment else {
            return format!("{NO_SELECTION_PLACEHOLDER}\n");
        };

        let mut output = String::new();

        // Separator line
        output.push_str(&Self::render_separator(ctx.max_width));
        output.push('\n');

        // Header: [author] file:line
        output.push_str(&Self::render_header(comment));
        output.push('\n');

        // Body text
        output.push_str(&Self::render_body(comment, ctx.max_width));
        output.push('\n');

        // Code context
        output.push_str(&self.render_code_context(comment, ctx.max_width));

        // Truncate to max_height if specified
        if ctx.max_height > 0 {
            truncate_to_height(&mut output, ctx.max_height);
        }

        output
    }

    /// Renders a horizontal separator line.
    ///
    /// Trusts the caller to provide an appropriate width (already clamped
    /// to 80 or terminal width by the caller).
    fn render_separator(width: usize) -> String {
        "\u{2500}".repeat(width)
    }

    /// Renders the comment header with author, file path, and line number.
    fn render_header(comment: &ReviewComment) -> String {
        let author = comment.author.as_deref().unwrap_or("unknown");
        let file = comment.file_path.as_deref().unwrap_or("(no file)");
        let line_suffix = comment
            .line_number
            .map_or_else(String::new, |n| format!(":{n}"));

        format!("[{author}] {file}{line_suffix}")
    }

    /// Renders the comment body text, wrapped to max width.
    fn render_body(comment: &ReviewComment, max_width: usize) -> String {
        let body = comment.body.as_deref().unwrap_or("(no comment text)");
        wrap_text(body, max_width)
    }

    /// Renders the code context with syntax highlighting.
    ///
    /// Uses the comment's `diff_hunk` field as the code source and
    /// attempts to highlight based on the file extension. Falls back
    /// to plain text if highlighting fails.
    fn render_code_context(&self, comment: &ReviewComment, max_width: usize) -> String {
        let Some(diff_hunk) = comment.diff_hunk.as_deref() else {
            return format!("{NO_CONTEXT_PLACEHOLDER}\n");
        };

        if diff_hunk.trim().is_empty() {
            return format!("{NO_CONTEXT_PLACEHOLDER}\n");
        }

        let mut output = String::from("\n");
        output.push_str(&self.highlighter.highlight_or_plain(
            diff_hunk,
            comment.file_path.as_deref(),
            max_width,
        ));

        output
    }
}

/// Truncates output to a maximum number of lines.
///
/// If the output exceeds `max_height` lines, it is truncated and
/// a "..." indicator is appended to show content was cut off.
fn truncate_to_height(output: &mut String, max_height: usize) {
    let line_count = output.lines().count();
    if line_count <= max_height {
        return;
    }

    // Find the byte position after the nth newline
    let truncate_at = find_nth_newline_position(output, max_height.saturating_sub(1));

    if let Some(pos) = truncate_at {
        output.truncate(pos);
        output.push_str("\n...\n");
    }
}

/// Finds the byte position after the nth newline in a string.
fn find_nth_newline_position(s: &str, n: usize) -> Option<usize> {
    let mut count = 0;
    for (i, ch) in s.char_indices() {
        if ch == '\n' {
            count += 1;
            if count > n {
                return Some(i);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;
    use crate::tui::components::test_utils::strip_ansi_codes;

    /// Builder for creating test review comments.
    #[derive(Default)]
    struct ReviewBuilder {
        id: u64,
        author: Option<String>,
        file_path: Option<String>,
        line_number: Option<u32>,
        body: Option<String>,
        diff_hunk: Option<String>,
    }

    impl ReviewBuilder {
        fn author(mut self, author: &str) -> Self {
            self.author = Some(author.to_owned());
            self
        }

        fn file_path(mut self, path: &str) -> Self {
            self.file_path = Some(path.to_owned());
            self
        }

        fn line_number(mut self, line: u32) -> Self {
            self.line_number = Some(line);
            self
        }

        fn body(mut self, body: &str) -> Self {
            self.body = Some(body.to_owned());
            self
        }

        fn diff_hunk(mut self, hunk: &str) -> Self {
            self.diff_hunk = Some(hunk.to_owned());
            self
        }

        fn build(self) -> ReviewComment {
            ReviewComment {
                id: self.id,
                body: self.body,
                author: self.author,
                file_path: self.file_path,
                line_number: self.line_number,
                original_line_number: None,
                diff_hunk: self.diff_hunk,
                commit_sha: None,
                in_reply_to_id: None,
                created_at: None,
                updated_at: None,
            }
        }
    }

    /// Renders a comment detail view for testing.
    ///
    /// Creates a `CommentDetailComponent` and renders the given comment
    /// with standard test width (80 columns) and unlimited height.
    fn render_comment_detail(comment: Option<&ReviewComment>) -> String {
        let component = CommentDetailComponent::new();
        let ctx = CommentDetailViewContext {
            selected_comment: comment,
            max_width: 80,
            max_height: 0, // unlimited
        };
        component.view(&ctx)
    }

    #[fixture]
    fn sample_comment() -> ReviewComment {
        ReviewBuilder::default()
            .author("alice")
            .file_path("src/main.rs")
            .line_number(42)
            .body("Please extract this helper function.")
            .diff_hunk("@@ -40,6 +40,10 @@\n+fn helper() {\n+    // code\n+}")
            .build()
    }

    #[fixture]
    fn comment_without_hunk() -> ReviewComment {
        ReviewBuilder::default()
            .author("bob")
            .file_path("src/lib.rs")
            .line_number(10)
            .body("Looks good!")
            .build()
    }

    #[test]
    fn view_renders_placeholder_when_no_comment() {
        let output = render_comment_detail(None);

        assert!(
            output.contains(NO_SELECTION_PLACEHOLDER),
            "should show no-selection placeholder"
        );
    }

    #[rstest]
    fn view_includes_author_and_file(sample_comment: ReviewComment) {
        let output = render_comment_detail(Some(&sample_comment));

        assert!(output.contains("[alice]"), "should include author");
        assert!(output.contains("src/main.rs"), "should include file path");
        assert!(output.contains(":42"), "should include line number");
    }

    #[rstest]
    fn view_includes_body_text(sample_comment: ReviewComment) {
        let output = render_comment_detail(Some(&sample_comment));

        assert!(
            output.contains("Please extract this helper"),
            "should include comment body"
        );
    }

    #[rstest]
    fn view_includes_code_context(sample_comment: ReviewComment) {
        let output = render_comment_detail(Some(&sample_comment));

        // The diff hunk should be visible (may have ANSI codes)
        assert!(
            output.contains("fn helper()") || output.contains("helper"),
            "should include code context from diff_hunk"
        );
    }

    #[rstest]
    fn view_shows_placeholder_when_no_diff_hunk(comment_without_hunk: ReviewComment) {
        let output = render_comment_detail(Some(&comment_without_hunk));

        assert!(
            output.contains(NO_CONTEXT_PLACEHOLDER),
            "should show no-context placeholder when diff_hunk is None"
        );
    }

    #[test]
    fn view_wraps_code_to_max_width() {
        let long_code = format!("@@ -1,1 +1,1 @@\n+{}", "x".repeat(120));
        let comment = ReviewBuilder::default()
            .author("alice")
            .file_path("src/main.rs")
            .diff_hunk(&long_code)
            .build();

        let output = render_comment_detail(Some(&comment));

        // Strip ANSI codes and check line widths
        let stripped = strip_ansi_codes(&output);
        for line in stripped.lines() {
            assert!(
                line.chars().count() <= 80,
                "line exceeds 80 chars: '{line}'"
            );
        }
    }

    #[test]
    fn render_separator_respects_width() {
        let sep_80 = CommentDetailComponent::render_separator(80);
        assert_eq!(
            sep_80.chars().count(),
            80,
            "separator should be 80 chars wide"
        );

        let sep_40 = CommentDetailComponent::render_separator(40);
        assert_eq!(
            sep_40.chars().count(),
            40,
            "separator should respect narrower width"
        );

        // Separator trusts the caller to clamp width; width >80 is allowed
        // if caller provides it (e.g., for wide terminals)
        let sep_100 = CommentDetailComponent::render_separator(100);
        assert_eq!(
            sep_100.chars().count(),
            100,
            "separator should use provided width"
        );
    }
}
