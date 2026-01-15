//! Test utilities for TUI component tests.
//!
//! Provides common helpers used across multiple test modules.

use crate::github::models::ReviewComment;

/// Builder for creating test `ReviewComment` instances.
///
/// Provides a fluent API for constructing review comments with only
/// the fields relevant to each test case.
///
/// # Example
///
/// ```
/// use frankie::tui::components::test_utils::ReviewCommentBuilder;
///
/// let comment = ReviewCommentBuilder::new(1)
///     .author("alice")
///     .file_path("src/main.rs")
///     .line_number(42)
///     .body("Please refactor this")
///     .build();
/// ```
#[derive(Default)]
pub struct ReviewCommentBuilder {
    id: u64,
    author: Option<String>,
    file_path: Option<String>,
    line_number: Option<u32>,
    body: Option<String>,
    diff_hunk: Option<String>,
}

impl ReviewCommentBuilder {
    /// Creates a new builder with the specified comment ID.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self {
            id,
            author: None,
            file_path: None,
            line_number: None,
            body: None,
            diff_hunk: None,
        }
    }

    /// Sets the comment author.
    #[must_use]
    pub fn author(mut self, author: &str) -> Self {
        self.author = Some(author.to_owned());
        self
    }

    /// Sets the file path for the comment.
    #[must_use]
    pub fn file_path(mut self, path: &str) -> Self {
        self.file_path = Some(path.to_owned());
        self
    }

    /// Sets the line number for the comment.
    #[must_use]
    pub const fn line_number(mut self, line: u32) -> Self {
        self.line_number = Some(line);
        self
    }

    /// Sets the comment body text.
    #[must_use]
    pub fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_owned());
        self
    }

    /// Sets the diff hunk for inline code context.
    #[must_use]
    pub fn diff_hunk(mut self, hunk: &str) -> Self {
        self.diff_hunk = Some(hunk.to_owned());
        self
    }

    /// Builds the `ReviewComment` instance.
    #[must_use]
    pub fn build(self) -> ReviewComment {
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

/// Strips ANSI escape codes from a string.
///
/// Used in tests to verify text content without interference from
/// syntax highlighting or terminal formatting codes.
///
/// # Example
///
/// ```
/// use frankie::tui::components::test_utils::strip_ansi_codes;
///
/// let colored = "\x1b[31mred text\x1b[0m";
/// assert_eq!(strip_ansi_codes(colored), "red text");
/// ```
#[must_use]
pub fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;

    for ch in s.chars() {
        in_escape = process_char_for_ansi_strip(ch, in_escape, &mut result);
    }

    result
}

/// Processes a single character for ANSI escape code stripping.
///
/// Returns the new escape state after processing the character.
fn process_char_for_ansi_strip(ch: char, in_escape: bool, result: &mut String) -> bool {
    // Start of escape sequence
    if ch == '\x1b' {
        return true;
    }

    // Inside escape sequence - wait for terminator
    if in_escape {
        // Alphabetic character ends the escape sequence
        return !ch.is_ascii_alphabetic();
    }

    // Normal character - add to result
    result.push(ch);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_simple_color_codes() {
        let input = "\x1b[31mred\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "red");
    }

    #[test]
    fn strips_multiple_codes() {
        let input = "\x1b[1m\x1b[31mbold red\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "bold red");
    }

    #[test]
    fn preserves_plain_text() {
        let input = "plain text without codes";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn handles_empty_string() {
        assert_eq!(strip_ansi_codes(""), "");
    }

    #[test]
    fn handles_codes_at_boundaries() {
        let input = "\x1b[31m\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "");
    }
}
