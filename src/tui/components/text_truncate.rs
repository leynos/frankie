//! Text truncation helpers for fixed-height terminal views.
//!
//! The helpers in this module trim rendered strings to a maximum number of
//! lines while preserving a clear "cut-off" indicator.

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
}
