//! Syntax highlighting adapter using syntect.
//!
//! Provides code block highlighting with graceful fallback to plain text
//! when highlighting is unavailable or fails. Lines are wrapped to a maximum
//! width before highlighting to avoid ANSI escape code complexity in width
//! calculations.

use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;
use thiserror::Error;

/// Errors that can occur during syntax highlighting.
#[derive(Debug, Error)]
pub enum HighlightError {
    /// No syntax definition found for the file extension.
    #[error("no syntax found for extension: {extension}")]
    NoSyntaxFound {
        /// The file extension that could not be matched.
        extension: String,
    },
    /// Syntect internal error during highlighting.
    #[error("highlighting failed: {message}")]
    HighlightFailed {
        /// Description of the failure.
        message: String,
    },
}

/// Code highlighter with lazy-loaded syntax definitions.
///
/// Uses syntect to provide syntax highlighting for code blocks based on
/// file extension detection. Falls back gracefully to plain text when
/// highlighting is not available.
#[derive(Debug)]
pub struct CodeHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl Default for CodeHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeHighlighter {
    /// Creates a new highlighter with default syntax and theme sets.
    #[must_use]
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Highlights a code block, falling back to plain text on error.
    ///
    /// Returns highlighted text with ANSI escape codes on success,
    /// or the original text (wrapped) on failure.
    ///
    /// # Arguments
    ///
    /// * `code` - The code text to highlight
    /// * `file_path` - Optional file path for syntax detection via extension
    /// * `max_width` - Maximum line width for wrapping (typically 80)
    #[must_use]
    pub fn highlight_or_plain(
        &self,
        code: &str,
        file_path: Option<&str>,
        max_width: usize,
    ) -> String {
        self.highlight_code_block(code, file_path, max_width)
            .unwrap_or_else(|_| wrap_plain_text(code, max_width))
    }

    /// Attempts to highlight a code block with syntax colouring.
    ///
    /// Lines are wrapped to `max_width` before highlighting to ensure
    /// consistent width regardless of ANSI escape codes.
    ///
    /// # Arguments
    ///
    /// * `code` - The code text to highlight
    /// * `file_path` - Optional file path for syntax detection via extension
    /// * `max_width` - Maximum line width for wrapping
    ///
    /// # Errors
    ///
    /// Returns [`HighlightError::NoSyntaxFound`] if the file extension cannot
    /// be matched to a syntax definition, or [`HighlightError::HighlightFailed`]
    /// if syntect encounters an internal error.
    pub fn highlight_code_block(
        &self,
        code: &str,
        file_path: Option<&str>,
        max_width: usize,
    ) -> Result<String, HighlightError> {
        let extension = file_path
            .and_then(|p| Path::new(p).extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let syntax = self
            .syntax_set
            .find_syntax_by_extension(extension)
            .ok_or_else(|| HighlightError::NoSyntaxFound {
                extension: extension.to_owned(),
            })?;

        let theme = self
            .theme_set
            .themes
            .get("base16-ocean.dark")
            .or_else(|| self.theme_set.themes.values().next())
            .ok_or_else(|| HighlightError::HighlightFailed {
                message: "no theme available".to_owned(),
            })?;

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut output = String::new();

        // Wrap lines first, then highlight each wrapped line
        let wrapped = wrap_plain_text(code, max_width);

        for line in wrapped.lines() {
            let ranges = highlighter
                .highlight_line(line, &self.syntax_set)
                .map_err(|e| HighlightError::HighlightFailed {
                    message: e.to_string(),
                })?;

            let escaped = as_24_bit_terminal_escaped(&ranges, false);
            output.push_str(&escaped);
            output.push('\n');
        }

        // Add reset code at the end to clear any lingering styles
        output.push_str("\x1b[0m");

        Ok(output)
    }
}

/// Wraps a single line to a maximum width using character count.
///
/// Uses character count rather than byte count to correctly handle
/// Unicode characters including emoji and CJK text.
///
/// # Arguments
///
/// * `line` - The line to wrap
/// * `max_width` - Maximum characters per line
///
/// # Examples
///
/// ```
/// use frankie::tui::components::code_highlight::wrap_to_width;
///
/// let short = "hello";
/// assert_eq!(wrap_to_width(short, 80), "hello");
///
/// let long = "a".repeat(100);
/// let wrapped = wrap_to_width(&long, 80);
/// assert!(wrapped.lines().all(|l| l.chars().count() <= 80));
/// ```
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

/// Wraps all lines of a multi-line code block to a maximum width.
///
/// Applies [`wrap_to_width`] to each line and joins the results.
///
/// # Arguments
///
/// * `code` - The multi-line code text
/// * `max_width` - Maximum characters per line
#[must_use]
pub fn wrap_plain_text(code: &str, max_width: usize) -> String {
    code.lines()
        .map(|line| wrap_to_width(line, max_width))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

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
    fn wrap_plain_text_handles_multiline() {
        let code = "short line\na]".to_owned() + &"b".repeat(100);
        let result = wrap_plain_text(&code, 80);

        for line in result.lines() {
            assert!(line.chars().count() <= 80, "line '{line}' exceeds 80 chars");
        }
    }

    #[test]
    fn wrap_plain_text_preserves_empty_lines() {
        let code = "line1\n\nline3";
        let result = wrap_plain_text(code, 80);
        let lines: Vec<&str> = result.lines().collect();

        assert_eq!(lines.len(), 3, "should preserve line count");
        assert_eq!(lines.get(1), Some(&""), "middle line should be empty");
    }

    #[rstest]
    #[case("test.rs", true)]
    #[case("test.py", true)]
    #[case("test.js", true)]
    #[case("test.unknown_ext_xyz", false)]
    #[case("", false)]
    fn highlighter_finds_syntax_for_known_extensions(
        #[case] file_path: &str,
        #[case] should_succeed: bool,
    ) {
        let highlighter = CodeHighlighter::new();
        let code = "let x = 1;";

        let result = highlighter.highlight_code_block(code, Some(file_path), 80);

        if should_succeed {
            assert!(
                result.is_ok(),
                "should find syntax for {file_path}: {result:?}"
            );
        } else {
            assert!(result.is_err(), "should not find syntax for {file_path}");
        }
    }

    #[test]
    fn highlight_or_plain_falls_back_gracefully() {
        let highlighter = CodeHighlighter::new();
        let code = "some code here";

        // Unknown extension should fall back to plain text
        let result = highlighter.highlight_or_plain(code, Some("test.xyz"), 80);

        assert!(
            !result.is_empty(),
            "should return non-empty result on fallback"
        );
        assert!(
            result.contains("some code here"),
            "should contain original text"
        );
    }

    #[test]
    fn highlighted_output_contains_ansi_codes() {
        let highlighter = CodeHighlighter::new();
        let code = "fn main() { println!(\"hello\"); }";

        let result = highlighter
            .highlight_code_block(code, Some("test.rs"), 80)
            .expect("should highlight Rust code");

        // ANSI escape codes start with \x1b[
        assert!(
            result.contains("\x1b["),
            "highlighted output should contain ANSI codes"
        );
    }

    #[test]
    fn highlighted_output_respects_max_width() {
        let highlighter = CodeHighlighter::new();
        let long_line = format!("let x = \"{}\";", "a".repeat(100));

        let result = highlighter
            .highlight_code_block(&long_line, Some("test.rs"), 80)
            .expect("should highlight");

        // Strip ANSI codes for width check
        let stripped = strip_ansi_codes(&result);
        for line in stripped.lines() {
            assert!(
                line.chars().count() <= 80,
                "line exceeds 80 chars after stripping ANSI: '{line}'"
            );
        }
    }

    /// Helper to strip ANSI escape codes for testing width.
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
