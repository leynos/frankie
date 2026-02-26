//! Side-by-side preview helpers for rewrite comparisons.

/// One row in a side-by-side rewrite preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideBySideLine {
    /// Original input line.
    pub original: String,
    /// Candidate rewritten line.
    pub candidate: String,
}

/// Side-by-side preview model consumed by CLI and TUI renderers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideBySideDiffPreview {
    /// Ordered preview rows.
    pub lines: Vec<SideBySideLine>,
    /// Whether the candidate text differs from the original.
    pub has_changes: bool,
}

/// Build a line-aligned side-by-side preview.
#[must_use]
pub fn build_side_by_side_diff_preview(original: &str, candidate: &str) -> SideBySideDiffPreview {
    let original_lines = split_lines_preserving_empty(original);
    let candidate_lines = split_lines_preserving_empty(candidate);

    let row_count = original_lines.len().max(candidate_lines.len());
    let lines = (0..row_count)
        .map(|index| SideBySideLine {
            original: original_lines.get(index).cloned().unwrap_or_default(),
            candidate: candidate_lines.get(index).cloned().unwrap_or_default(),
        })
        .collect();

    SideBySideDiffPreview {
        lines,
        has_changes: original != candidate,
    }
}

fn split_lines_preserving_empty(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut lines: Vec<String> = text.split('\n').map(ToOwned::to_owned).collect();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::build_side_by_side_diff_preview;

    #[test]
    fn preview_marks_changes_when_text_differs() {
        let preview = build_side_by_side_diff_preview("a", "b");

        assert!(preview.has_changes);
        assert_eq!(preview.lines.len(), 1);
        let first_line = preview.lines.first();
        assert!(first_line.is_some(), "preview should contain one line");
        if let Some(line) = first_line {
            assert_eq!(line.original, "a");
            assert_eq!(line.candidate, "b");
        }
    }

    #[test]
    fn preview_marks_no_change_for_identical_text() {
        let preview = build_side_by_side_diff_preview("same", "same");

        assert!(!preview.has_changes);
        assert_eq!(preview.lines.len(), 1);
    }

    #[rstest]
    #[case("line1\nline2", "line1")]
    #[case("line1", "line1\nline2")]
    fn preview_aligns_different_line_counts(#[case] original: &str, #[case] candidate: &str) {
        let preview = build_side_by_side_diff_preview(original, candidate);

        assert_eq!(preview.lines.len(), 2);
    }
}
