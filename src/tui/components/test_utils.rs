//! Test utilities for TUI component tests.
//!
//! Provides common helpers used across multiple test modules.

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
