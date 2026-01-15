//! Text wrapping utilities for terminal display.
//!
//! Provides two wrapping strategies:
//! - **Character-based wrapping** (`wrap_to_width`): Hard-wraps at exactly N
//!   characters, used for code blocks where preserving exact layout matters.
//! - **Word-based wrapping** (`wrap_text`): Wraps at word boundaries while
//!   preserving indentation and multiple spaces, used for prose text.

/// Wraps a single line to a maximum width using character count.
///
/// Uses character count rather than byte count to correctly handle
/// Unicode characters including emoji and CJK text.
///
/// # Arguments
///
/// * `line` - The line to wrap
/// * `max_width` - Maximum characters per line
#[must_use]
pub fn wrap_to_width(line: &str, max_width: usize) -> String {
    if max_width == 0 {
        return line.to_owned();
    }

    let char_count = line.chars().count();
    if char_count <= max_width {
        return line.to_owned();
    }

    // Estimate extra capacity for newlines: one per max_width chars
    let extra_newlines = char_count
        .saturating_sub(1)
        .checked_div(max_width)
        .unwrap_or(0);
    let mut result = String::with_capacity(line.len() + extra_newlines);
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

/// Wraps all lines of a multi-line text block to a maximum width.
///
/// Applies [`wrap_to_width`] to each line and joins the results.
/// This is the primary entry point for wrapping code blocks.
///
/// # Arguments
///
/// * `text` - The multi-line text
/// * `max_width` - Maximum characters per line
#[must_use]
pub fn wrap_code_block(text: &str, max_width: usize) -> String {
    text.lines()
        .map(|line| wrap_to_width(line, max_width))
        .collect::<Vec<_>>()
        .join("\n")
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
///
/// # Arguments
///
/// * `text` - The text to wrap
/// * `max_width` - Maximum characters per line
#[must_use]
pub fn wrap_text(text: &str, max_width: usize) -> String {
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
    use super::*;

    // Character-based wrapping tests

    #[test]
    fn wrap_to_width_preserves_short_lines() {
        let short = "hello world";
        let result = wrap_to_width(short, 80);
        assert_eq!(result, short, "short line should be unchanged");
    }

    #[test]
    fn wrap_to_width_wraps_long_lines() {
        let long = "a".repeat(120);
        let result = wrap_to_width(&long, 80);

        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2, "should wrap into 2 lines");
        assert_eq!(
            lines.first().map(|l| l.chars().count()),
            Some(80),
            "first line should be 80 chars"
        );
        assert_eq!(
            lines.get(1).map(|l| l.chars().count()),
            Some(40),
            "second line should be 40 chars"
        );
    }

    #[test]
    fn wrap_to_width_handles_exact_width() {
        let exact = "a".repeat(80);
        let result = wrap_to_width(&exact, 80);
        assert_eq!(result, exact, "exact width line should be unchanged");
    }

    #[test]
    fn wrap_to_width_handles_zero_width() {
        let line = "hello";
        let result = wrap_to_width(line, 0);
        assert_eq!(result, line, "zero width should return original");
    }

    #[test]
    fn wrap_to_width_handles_unicode() {
        // Each emoji is one character but multiple bytes
        let emojis = "🎉".repeat(100);
        let result = wrap_to_width(&emojis, 80);

        for line in result.lines() {
            assert!(
                line.chars().count() <= 80,
                "each line should have at most 80 characters"
            );
        }
    }

    #[test]
    fn wrap_code_block_handles_multiline() {
        let code = "short line\na]".to_owned() + &"b".repeat(100);
        let result = wrap_code_block(&code, 80);

        for line in result.lines() {
            assert!(line.chars().count() <= 80, "line '{line}' exceeds 80 chars");
        }
    }

    #[test]
    fn wrap_code_block_preserves_empty_lines() {
        let code = "line1\n\nline3";
        let result = wrap_code_block(code, 80);
        let lines: Vec<&str> = result.lines().collect();

        assert_eq!(lines.len(), 3, "should preserve line count");
        assert_eq!(lines.get(1), Some(&""), "middle line should be empty");
    }

    // Word-based wrapping tests

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
}
