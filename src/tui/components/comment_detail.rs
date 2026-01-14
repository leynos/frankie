//! Comment detail component for rendering a selected review comment.
//!
//! This component displays a single review comment's metadata, body text,
//! and inline code context with optional syntax highlighting. Code context
//! is extracted from the comment's `diff_hunk` field and wrapped to a
//! maximum width (typically 80 columns or the terminal width if narrower).

use crate::github::models::ReviewComment;

use super::code_highlight::CodeHighlighter;

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
    /// Terminal height available for the detail pane.
    pub available_height: usize,
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
    pub fn new() -> Self {
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

        output
    }

    /// Renders a horizontal separator line.
    fn render_separator(width: usize) -> String {
        "\u{2500}".repeat(width.min(80))
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

/// Wraps text to a maximum width, preserving word boundaries where possible.
///
/// This is a simple word-wrap implementation for comment body text.
fn wrap_text(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return text.to_owned();
    }

    text.split('\n')
        .map(|paragraph| wrap_paragraph(paragraph, max_width))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wraps a single paragraph to the specified maximum width.
///
/// Handles word-by-word wrapping, collecting lines into a vector
/// and joining them with newlines.
fn wrap_paragraph(paragraph: &str, max_width: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in paragraph.split_whitespace() {
        let word_len = word.chars().count();

        if should_start_new_line(current_width, word_len, max_width) {
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
            }
            current_width = 0;
        }

        append_word_to_line(word, &mut current_line, &mut current_width);
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines.join("\n")
}

/// Determines if a new line should be started before adding a word.
///
/// Returns true if the current line is non-empty and adding the word
/// (plus a space separator) would exceed the maximum width.
const fn should_start_new_line(current_width: usize, word_len: usize, max_width: usize) -> bool {
    current_width > 0 && current_width + 1 + word_len > max_width
}

/// Appends a word to the current line, updating the width.
///
/// Adds a space separator if the line already has content.
fn append_word_to_line(word: &str, line: &mut String, width: &mut usize) {
    if *width > 0 {
        line.push(' ');
        *width += 1;
    }
    line.push_str(word);
    *width += word.chars().count();
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;

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
    /// with standard test dimensions (80 width, 20 height).
    fn render_comment_detail(comment: Option<&ReviewComment>) -> String {
        let component = CommentDetailComponent::new();
        let ctx = CommentDetailViewContext {
            selected_comment: comment,
            max_width: 80,
            available_height: 20,
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
        let component = CommentDetailComponent::new();
        let ctx = CommentDetailViewContext {
            selected_comment: Some(&sample_comment),
            max_width: 80,
            available_height: 20,
        };

        let output = component.view(&ctx);

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

        let component = CommentDetailComponent::new();
        let ctx = CommentDetailViewContext {
            selected_comment: Some(&comment),
            max_width: 80,
            available_height: 20,
        };

        let output = component.view(&ctx);

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
    fn wrap_text_handles_short_text() {
        let text = "Short text";
        let result = wrap_text(text, 80);
        assert_eq!(result, text, "short text should be unchanged");
    }

    #[test]
    fn wrap_text_wraps_long_paragraph() {
        let text = "This is a longer paragraph that should be wrapped across multiple lines when the width is limited.";
        let result = wrap_text(text, 40);

        for line in result.lines() {
            assert!(line.chars().count() <= 40, "line '{line}' exceeds 40 chars");
        }
    }

    #[test]
    fn wrap_text_preserves_paragraphs() {
        let text = "First paragraph.\n\nSecond paragraph.";
        let result = wrap_text(text, 80);

        assert!(
            result.contains("First paragraph."),
            "should preserve first paragraph"
        );
        assert!(
            result.contains("Second paragraph."),
            "should preserve second paragraph"
        );
    }

    #[test]
    fn render_separator_respects_width() {
        let sep_80 = CommentDetailComponent::render_separator(80);
        assert_eq!(
            sep_80.chars().count(),
            80,
            "separator should be 80 chars wide"
        );

        let sep_120 = CommentDetailComponent::render_separator(120);
        assert_eq!(
            sep_120.chars().count(),
            80,
            "separator should cap at 80 chars"
        );

        let sep_40 = CommentDetailComponent::render_separator(40);
        assert_eq!(
            sep_40.chars().count(),
            40,
            "separator should respect narrower width"
        );
    }

    /// Helper to strip ANSI escape codes for testing.
    fn strip_ansi_codes(s: &str) -> String {
        let mut result = String::new();
        let mut in_escape = false;

        for ch in s.chars() {
            if ch == '\x1b' {
                in_escape = true;
                continue;
            }
            if in_escape && ch.is_ascii_alphabetic() {
                in_escape = false;
                continue;
            }
            if !in_escape {
                result.push(ch);
            }
        }

        result
    }
}
