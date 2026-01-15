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
/// a "..." indicator is appended on the final line to show content was cut off.
/// The result will have exactly `max_height` lines (or fewer if input is shorter).
fn truncate_to_height(output: &mut String, max_height: usize) {
    let line_count = output.lines().count();
    if line_count <= max_height {
        return;
    }

    // Reserve one line for the ellipsis indicator
    let lines_to_keep = max_height.saturating_sub(1);

    // Find the byte position of the newline after the lines we want to keep
    // We want to find the (lines_to_keep)th newline (0-indexed), so we look
    // for the position where count exceeds (lines_to_keep - 1)
    let truncate_at = if lines_to_keep == 0 {
        // Special case: keep 0 lines, just show ellipsis
        Some(0)
    } else {
        find_nth_newline_position(output, lines_to_keep - 1).map(|pos| pos + 1)
    };

    if let Some(pos) = truncate_at {
        output.truncate(pos);
        output.push_str("...\n");
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
#[path = "comment_detail_tests.rs"]
mod tests;
