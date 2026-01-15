//! Tests for text wrapping utilities.

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
    let text = "    This is an indented line that is quite long and should wrap to the next line.";
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
