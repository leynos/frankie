//! Text truncation helpers for fixed-height terminal views.
//!
//! The helpers in this module trim rendered strings to a maximum number of
//! lines while preserving a clear "cut-off" indicator.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Truncates output to a maximum number of lines.
///
/// When `max_height` is non-zero and the output exceeds that number of lines,
/// the content is truncated and an ellipsis line is appended. The final output
/// contains at most `max_height` lines.
///
/// # Examples
///
/// ```rust,ignore
/// use frankie::tui::components::text_truncate::truncate_to_height;
///
/// let mut output = String::from("one\ntwo\nthree\n");
/// truncate_to_height(&mut output, 2);
///
/// assert_eq!(output, "one\n...\n");
/// ```
pub(crate) fn truncate_to_height(output: &mut String, max_height: usize) {
    if max_height == 0 {
        return;
    }

    let line_count = output.lines().count();
    if line_count <= max_height {
        return;
    }

    let lines_to_keep = max_height.saturating_sub(1);
    let truncate_at = if lines_to_keep == 0 {
        Some(0)
    } else {
        find_nth_newline_position(output, lines_to_keep - 1).map(|pos| pos + 1)
    };

    if let Some(pos) = truncate_at {
        output.truncate(pos);
        if contains_ansi_escape(output) {
            output.push_str(ANSI_RESET);
        }
        output.push_str("...\n");
    }
}

/// Finds the byte index of the nth newline character in a string (0-indexed).
///
/// Callers should add 1 to the returned index to obtain the byte position
/// immediately after the newline.
///
/// # Examples
///
/// ```rust,ignore
/// use frankie::tui::components::text_truncate::find_nth_newline_position;
///
/// let text = "a\nb\nc\n";
/// assert_eq!(find_nth_newline_position(text, 1), Some(3));
/// ```
pub(crate) fn find_nth_newline_position(s: &str, n: usize) -> Option<usize> {
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

const ANSI_RESET: &str = "\x1b[0m";

enum WidthTruncationDecision {
    Empty,
    Unchanged,
    DotFallback,
    Ellipsis,
}

const fn is_zero_width(max_width: usize) -> bool {
    max_width == 0
}

fn fits_display_width(text: &str, max_width: usize) -> bool {
    text.width() <= max_width
}

const fn should_use_dot_fallback(max_width: usize) -> bool {
    max_width <= 3
}

fn width_truncation_decision(text: &str, max_width: usize) -> WidthTruncationDecision {
    if is_zero_width(max_width) {
        WidthTruncationDecision::Empty
    } else if fits_display_width(text, max_width) {
        WidthTruncationDecision::Unchanged
    } else if should_use_dot_fallback(max_width) {
        WidthTruncationDecision::DotFallback
    } else {
        WidthTruncationDecision::Ellipsis
    }
}

/// Truncates text to the provided display width and appends an ellipsis.
///
/// This helper measures width in terminal columns, not Unicode scalar count.
pub(crate) fn truncate_to_display_width_with_ellipsis(text: &str, max_width: usize) -> String {
    match width_truncation_decision(text, max_width) {
        WidthTruncationDecision::Empty => String::new(),
        WidthTruncationDecision::Unchanged => text.to_owned(),
        WidthTruncationDecision::DotFallback => ".".repeat(max_width),
        WidthTruncationDecision::Ellipsis => {
            let target_width = max_width.saturating_sub(3);
            let mut truncated = String::new();
            let mut current_width = 0;
            for ch in text.chars() {
                let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
                if current_width + char_width > target_width {
                    break;
                }
                truncated.push(ch);
                current_width += char_width;
            }
            format!("{truncated}...")
        }
    }
}

fn contains_ansi_escape(text: &str) -> bool {
    text.contains("\x1b[")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_to_height_keeps_short_output() {
        let mut output = String::from("one\ntwo\n");
        truncate_to_height(&mut output, 3);
        assert_eq!(output, "one\ntwo\n");
    }

    #[test]
    fn truncate_to_height_adds_ellipsis() {
        let mut output = String::from("one\ntwo\nthree\n");
        truncate_to_height(&mut output, 2);
        assert_eq!(output, "one\n...\n");
    }

    #[test]
    fn truncate_to_height_skips_zero_height() {
        let mut output = String::from("one\ntwo\n");
        truncate_to_height(&mut output, 0);
        assert_eq!(output, "one\ntwo\n");
    }

    #[test]
    fn truncate_to_height_adds_reset_for_ansi_output() {
        let mut output = "\u{1b}[31mred\nline2\nline3\n".to_owned();
        truncate_to_height(&mut output, 2);
        assert_eq!(output, "\u{1b}[31mred\n\u{1b}[0m...\n");
    }

    #[test]
    fn truncate_to_display_width_with_ellipsis_keeps_short_text() {
        assert_eq!(
            truncate_to_display_width_with_ellipsis("hello", 10),
            "hello"
        );
    }

    #[test]
    fn truncate_to_display_width_with_ellipsis_handles_small_widths() {
        assert_eq!(truncate_to_display_width_with_ellipsis("abcdef", 0), "");
        assert_eq!(truncate_to_display_width_with_ellipsis("abcdef", 2), "..");
        assert_eq!(truncate_to_display_width_with_ellipsis("abcdef", 3), "...");
    }

    #[test]
    fn truncate_to_display_width_with_ellipsis_respects_wide_characters() {
        assert_eq!(
            truncate_to_display_width_with_ellipsis("你好世界", 5),
            "你..."
        );
    }
}
