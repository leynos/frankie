//! Syntax highlighting adapter using syntect.
//!
//! Provides code block highlighting with graceful fallback to plain text
//! when highlighting is unavailable or fails. Lines are wrapped to a maximum
//! width before highlighting to avoid ANSI escape code complexity in width
//! calculations.

use std::path::Path;
use std::sync::LazyLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;
use thiserror::Error;

use super::text_wrap::wrap_code_block;

/// Lazily-loaded syntax definitions shared across all highlighters.
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

/// Lazily-loaded theme set shared across all highlighters.
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Errors that can occur during syntax highlighting.
#[derive(Debug, Error)]
enum HighlightError {
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

/// Code highlighter with shared syntax definitions.
///
/// Uses syntect to provide syntax highlighting for code blocks based on
/// file extension detection. Falls back gracefully to plain text when
/// highlighting is not available.
///
/// Syntax and theme definitions are lazily loaded once and shared across
/// all instances via static references.
#[derive(Debug, Default)]
pub struct CodeHighlighter;

impl CodeHighlighter {
    /// Creates a new highlighter.
    ///
    /// This is a lightweight operation as syntax definitions are
    /// loaded lazily on first use and shared across all instances.
    #[must_use]
    pub const fn new() -> Self {
        Self
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
        highlight_code_block(code, file_path, max_width)
            .unwrap_or_else(|_| wrap_code_block(code, max_width))
    }
}

/// Attempts to highlight a code block with syntax colouring.
///
/// Lines are wrapped to `max_width` before highlighting to ensure
/// consistent width regardless of ANSI escape codes.
fn highlight_code_block(
    code: &str,
    file_path: Option<&str>,
    max_width: usize,
) -> Result<String, HighlightError> {
    let extension = file_path
        .and_then(|p| Path::new(p).extension())
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let syntax = SYNTAX_SET
        .find_syntax_by_extension(extension)
        .ok_or_else(|| HighlightError::NoSyntaxFound {
            extension: extension.to_owned(),
        })?;

    let theme = THEME_SET
        .themes
        .get("base16-ocean.dark")
        .or_else(|| THEME_SET.themes.values().next())
        .ok_or_else(|| HighlightError::HighlightFailed {
            message: "no theme available".to_owned(),
        })?;

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut output = String::new();

    // Wrap lines first, then highlight each wrapped line
    let wrapped = wrap_code_block(code, max_width);

    for line in wrapped.lines() {
        let ranges = highlighter.highlight_line(line, &SYNTAX_SET).map_err(|e| {
            HighlightError::HighlightFailed {
                message: e.to_string(),
            }
        })?;

        let escaped = as_24_bit_terminal_escaped(&ranges, false);
        output.push_str(&escaped);
        output.push('\n');
    }

    // Add reset code at the end to clear any lingering styles
    output.push_str("\x1b[0m");

    Ok(output)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::tui::components::test_utils::strip_ansi_codes;

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
        let code = "let x = 1;";

        let result = highlight_code_block(code, Some(file_path), 80);

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
        let code = "fn main() { println!(\"hello\"); }";

        let result =
            highlight_code_block(code, Some("test.rs"), 80).expect("should highlight Rust code");

        // ANSI escape codes start with \x1b[
        assert!(
            result.contains("\x1b["),
            "highlighted output should contain ANSI codes"
        );
    }

    #[test]
    fn highlighted_output_respects_max_width() {
        let long_line = format!("let x = \"{}\";", "a".repeat(100));

        let result =
            highlight_code_block(&long_line, Some("test.rs"), 80).expect("should highlight");

        // Strip ANSI codes for width check
        let stripped = strip_ansi_codes(&result);
        for line in stripped.lines() {
            assert!(
                line.chars().count() <= 80,
                "line exceeds 80 chars after stripping ANSI: '{line}'"
            );
        }
    }
}
