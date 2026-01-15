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

/// Wraps text to a maximum width, preserving existing whitespace.
///
/// Unlike simple word-wrap, this preserves:
/// - Leading indentation on each line
/// - Multiple spaces between words
/// - Empty lines (paragraph breaks)
///
/// Lines are only wrapped when they exceed `max_width`. Wrapped
/// continuation lines preserve the original line's indentation.
fn wrap_text(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return text.to_owned();
    }

    text.lines()
        .map(|line| wrap_line_preserving_indent(line, max_width))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wraps a single line while preserving its leading indentation.
///
/// If the line fits within `max_width`, returns it unchanged.
/// Otherwise, wraps at word boundaries, using the original
/// indentation for continuation lines.
fn wrap_line_preserving_indent(line: &str, max_width: usize) -> String {
    let line_width = line.chars().count();

    // Short lines pass through unchanged
    if line_width <= max_width {
        return line.to_owned();
    }

    // Extract leading whitespace
    let (indent, content) = split_at_content_start(line);
    let indent_width = indent.chars().count();

    // If indent alone exceeds max_width, just hard-wrap
    if indent_width >= max_width {
        return hard_wrap_line(line, max_width);
    }

    let available_width = max_width.saturating_sub(indent_width);
    wrap_content_with_indent(content, indent, available_width)
}

/// Splits a line into leading whitespace and content.
///
/// Returns a tuple of (indent, content) where indent is all leading
/// whitespace characters and content is the rest of the line.
fn split_at_content_start(line: &str) -> (&str, &str) {
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    line.split_at(indent_len)
}

/// Context for wrapping content with indentation.
struct WrapContext<'a> {
    indent: &'a str,
    available_width: usize,
    lines: Vec<String>,
    current_line: String,
    content_width: usize,
}

impl<'a> WrapContext<'a> {
    fn new(indent: &'a str, available_width: usize) -> Self {
        Self {
            indent,
            available_width,
            lines: Vec::new(),
            current_line: String::from(indent),
            content_width: 0,
        }
    }

    const fn is_line_empty(&self) -> bool {
        self.content_width == 0
    }

    fn start_new_line(&mut self) {
        let old_line = std::mem::replace(&mut self.current_line, String::from(self.indent));
        self.lines.push(old_line);
        self.content_width = 0;
    }

    fn push_word(&mut self, word: &str) {
        self.current_line.push_str(word);
        self.content_width += word.chars().count();
    }

    fn push_space(&mut self, space: &str) {
        self.current_line.push_str(space);
        self.content_width += space.chars().count();
    }

    fn process_word(&mut self, word: &str) {
        let word_len = word.chars().count();

        // Check if we need to wrap before this word
        if !self.is_line_empty() && self.content_width + word_len > self.available_width {
            self.start_new_line();
        }

        // Handle words longer than available width (hard wrap)
        if word_len > self.available_width && self.is_line_empty() {
            self.process_long_word(word);
        } else {
            self.push_word(word);
        }
    }

    fn process_long_word(&mut self, word: &str) {
        let wrapped = hard_wrap_line(word, self.available_width);
        let parts: Vec<&str> = wrapped.lines().collect();
        let last_idx = parts.len().saturating_sub(1);

        for (i, part) in parts.into_iter().enumerate() {
            self.push_word(part);
            if i < last_idx {
                self.start_new_line();
            }
        }
    }

    fn process_space(&mut self, space: &str) {
        if self.is_line_empty() {
            return;
        }

        let space_len = space.chars().count();
        if self.content_width + space_len <= self.available_width {
            self.push_space(space);
        } else {
            self.start_new_line();
        }
    }

    fn finish(mut self) -> String {
        self.lines.push(self.current_line);
        self.lines.join("\n")
    }
}

/// Wraps content with a given indent prefix.
///
/// Words are wrapped to fit within `available_width`, and each
/// wrapped line is prefixed with `indent`.
fn wrap_content_with_indent(content: &str, indent: &str, available_width: usize) -> String {
    let mut ctx = WrapContext::new(indent, available_width);

    for segment in split_preserving_spaces(content) {
        match segment {
            Segment::Word(word) => ctx.process_word(&word),
            Segment::Space(space) => ctx.process_space(&space),
        }
    }

    ctx.finish()
}

/// Hard-wraps a line at exactly `max_width` characters.
///
/// Used when soft wrapping is not possible (e.g., very long words
/// or lines where indentation exceeds the available width).
fn hard_wrap_line(line: &str, max_width: usize) -> String {
    if max_width == 0 {
        return line.to_owned();
    }

    let mut result = String::new();
    let mut current_width = 0;

    for ch in line.chars() {
        if current_width >= max_width {
            result.push('\n');
            current_width = 0;
        }
        result.push(ch);
        current_width += 1;
    }

    result
}

/// A segment of text: either a word or whitespace.
enum Segment {
    Word(String),
    Space(String),
}

/// Splits content into alternating word and space segments.
///
/// Preserves the exact spacing between words, allowing the wrapper
/// to maintain multiple spaces where they appear in the original.
fn split_preserving_spaces(content: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_whitespace: Option<bool> = None;

    for ch in content.chars() {
        let is_space = ch.is_whitespace();

        match in_whitespace {
            None => {
                // First character
                current.push(ch);
                in_whitespace = Some(is_space);
            }
            Some(was_space) if was_space == is_space => {
                // Same type, continue accumulating
                current.push(ch);
            }
            Some(was_space) => {
                // Type changed, push current segment and start new one
                let segment = if was_space {
                    Segment::Space(std::mem::take(&mut current))
                } else {
                    Segment::Word(std::mem::take(&mut current))
                };
                segments.push(segment);
                current.push(ch);
                in_whitespace = Some(is_space);
            }
        }
    }

    // Push final segment if any
    push_final_segment(&mut segments, current, in_whitespace);

    segments
}

/// Pushes the final accumulated segment if non-empty.
fn push_final_segment(segments: &mut Vec<Segment>, current: String, in_whitespace: Option<bool>) {
    let Some(is_space) = in_whitespace else {
        return;
    };

    if current.is_empty() {
        return;
    }

    let segment = if is_space {
        Segment::Space(current)
    } else {
        Segment::Word(current)
    };
    segments.push(segment);
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
    fn wrap_text_preserves_indentation() {
        let text = "    indented line";
        let result = wrap_text(text, 80);
        assert_eq!(result, text, "should preserve leading spaces");
    }

    #[test]
    fn wrap_text_preserves_code_block_indentation() {
        let text = "Here is code:\n    fn example() {\n        let x = 1;\n    }";
        let result = wrap_text(text, 80);

        assert!(
            result.contains("    fn example()"),
            "should preserve 4-space indent: {result}"
        );
        assert!(
            result.contains("        let x = 1;"),
            "should preserve 8-space indent: {result}"
        );
    }

    #[test]
    fn wrap_text_preserves_multiple_spaces() {
        let text = "column1  column2  column3";
        let result = wrap_text(text, 80);
        assert_eq!(
            result, text,
            "should preserve double spaces between columns"
        );
    }

    #[test]
    fn wrap_text_wraps_indented_long_line() {
        let text =
            "    This is an indented line that is quite long and should wrap to the next line.";
        let result = wrap_text(text, 40);

        // All lines should have the same indentation
        for line in result.lines() {
            assert!(
                line.starts_with("    ") || line.is_empty(),
                "wrapped line should preserve indent: '{line}'"
            );
            assert!(
                line.chars().count() <= 40,
                "line should not exceed max width: '{line}'"
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
