//! Tests for the comment detail component.

use rstest::{fixture, rstest};

use super::*;
use crate::tui::components::test_utils::{ReviewCommentBuilder, strip_ansi_codes};
use crate::tui::components::text_truncate::find_nth_newline_position;

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
        reply_draft: None,
        reply_draft_ai_preview: None,
    };
    component.view(&ctx)
}

/// Helper to render a comment detail view with a reply draft for testing.
fn render_comment_with_reply_draft(
    text: &str,
    char_count: usize,
    ready_to_send: bool,
    origin_label: Option<&str>,
) -> String {
    let component = CommentDetailComponent::new();
    let comment = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/main.rs")
        .line_number(42)
        .body("nit")
        .build();
    let ctx = CommentDetailViewContext {
        selected_comment: Some(&comment),
        max_width: 80,
        max_height: 0,
        reply_draft: Some(ReplyDraftRenderContext {
            text,
            char_count,
            max_length: 120,
            ready_to_send,
            origin_label,
        }),
        reply_draft_ai_preview: None,
    };
    component.view(&ctx)
}

#[fixture]
fn sample_comment() -> ReviewComment {
    ReviewCommentBuilder::new(0)
        .author("alice")
        .file_path("src/main.rs")
        .line_number(42)
        .body("Please extract this helper function.")
        .diff_hunk("@@ -40,6 +40,10 @@\n+fn helper() {\n+    // code\n+}")
        .build()
}

#[fixture]
fn comment_without_hunk() -> ReviewComment {
    ReviewCommentBuilder::new(0)
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
    let stripped = strip_ansi_codes(&output);

    // The diff hunk from sample_comment contains "+fn helper() {\n+    // code\n+}"
    // After rendering via render_comment_detail (which uses CodeHighlighter),
    // the exact diff symbol "fn helper()" must appear in the output.
    assert!(
        stripped.contains("fn helper()"),
        "should include exact diff symbol 'fn helper()' from diff_hunk; got: {stripped}"
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
    let comment = ReviewCommentBuilder::new(0)
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

// Tests for find_nth_newline_position

#[test]
fn find_nth_newline_position_returns_none_for_empty_string() {
    let result = find_nth_newline_position("", 0);
    assert_eq!(result, None, "empty string has no newlines");
}

#[test]
fn find_nth_newline_position_returns_none_when_no_newlines() {
    let result = find_nth_newline_position("no newlines here", 0);
    assert_eq!(result, None, "string without newlines should return None");
}

#[test]
fn find_nth_newline_position_finds_first_newline() {
    // "line1\nline2\n" - first newline at index 5, looking for n=0 returns None
    // because we want position AFTER the nth newline
    let input = "line1\nline2\n";
    let result = find_nth_newline_position(input, 0);
    // n=0 means we want to find position after 0th newline occurrence is exceeded
    // The function returns Some(i) when count > n, so for n=0, after first newline
    assert_eq!(result, Some(5), "should find byte index of first newline");
}

#[test]
fn find_nth_newline_position_finds_second_newline() {
    let input = "line1\nline2\nline3";
    let result = find_nth_newline_position(input, 1);
    // First newline at 5, second at 11
    assert_eq!(result, Some(11), "should find byte index of second newline");
}

#[test]
fn find_nth_newline_position_returns_none_when_not_enough_newlines() {
    let input = "line1\nline2";
    let result = find_nth_newline_position(input, 5);
    assert_eq!(
        result, None,
        "should return None when n exceeds newline count"
    );
}

// Tests for truncate_to_height
//
// truncate_to_height keeps (max_height - 1) lines of content plus
// a "...\n" indicator, resulting in exactly max_height lines when truncated.

#[test]
fn truncate_to_height_preserves_short_content() {
    let mut output = "line1\nline2\nline3".to_owned();
    truncate_to_height(&mut output, 5);
    assert_eq!(
        output, "line1\nline2\nline3",
        "content within max_height should be unchanged"
    );
}

#[test]
fn truncate_to_height_truncates_long_content() {
    let mut output = "line1\nline2\nline3\nline4\nline5".to_owned();
    truncate_to_height(&mut output, 3);
    // Keeps max_height-1=2 lines, then appends "...\n" for exactly 3 lines
    assert_eq!(
        output, "line1\nline2\n...\n",
        "should truncate to max_height lines with ellipsis"
    );
}

#[test]
fn truncate_to_height_handles_trailing_newline() {
    let mut output = "line1\nline2\nline3\n".to_owned();
    truncate_to_height(&mut output, 2);
    // Input has 4 lines via lines(): "line1", "line2", "line3", ""
    // Keeps max_height-1=1 line, then appends "...\n"
    assert_eq!(
        output, "line1\n...\n",
        "should handle input ending with newline"
    );
}

#[test]
fn truncate_to_height_handles_no_trailing_newline() {
    let mut output = "line1\nline2\nline3".to_owned();
    truncate_to_height(&mut output, 2);
    // Input has 3 lines via lines(): "line1", "line2", "line3"
    // Keeps max_height-1=1 line, appends "...\n" for exactly 2 lines
    assert_eq!(
        output, "line1\n...\n",
        "should handle input without trailing newline"
    );
}

#[test]
fn truncate_to_height_handles_consecutive_blank_lines() {
    let mut output = "line1\n\n\nline4".to_owned();
    truncate_to_height(&mut output, 3);
    // Input has 4 lines: "line1", "", "", "line4"
    // Keeps max_height-1=2 lines (including blank), appends "...\n"
    assert_eq!(
        output, "line1\n\n...\n",
        "should preserve blank lines in truncated output"
    );
}

#[test]
fn truncate_to_height_handles_single_line() {
    let mut output = "only one line".to_owned();
    truncate_to_height(&mut output, 1);
    assert_eq!(
        output, "only one line",
        "single line within max_height should be unchanged"
    );
}

#[test]
fn truncate_to_height_handles_max_height_one() {
    let mut output = "line1\nline2".to_owned();
    truncate_to_height(&mut output, 1);
    // max_height=1 with 2 lines: keeps 0 lines of content, just "...\n"
    // find_nth_newline_position(_, 0) finds first newline at pos 5
    // truncates to empty string before first newline, then appends "...\n"
    assert_eq!(
        output, "...\n",
        "max_height=1 with multiple lines shows only ellipsis"
    );
}

#[test]
fn truncate_to_height_exact_boundary() {
    let mut output = "line1\nline2\nline3".to_owned();
    truncate_to_height(&mut output, 3);
    assert_eq!(
        output, "line1\nline2\nline3",
        "content exactly at max_height should be unchanged"
    );
}

#[test]
fn view_renders_inline_reply_draft_when_present() {
    let output = render_comment_with_reply_draft("Thanks for the review.", 22, true, None);

    assert!(output.contains("Reply draft:"));
    assert!(output.contains("Thanks for the review."));
    assert!(output.contains("Length: 22/120 (ready to send)"));
}

#[test]
fn view_renders_inline_reply_draft_empty_placeholder() {
    let output = render_comment_with_reply_draft("", 0, true, None);

    assert!(output.contains("(empty)"));
    assert!(output.contains("(ready to send)"));
}

#[test]
fn view_renders_inline_reply_draft_without_ready_suffix() {
    let output = render_comment_with_reply_draft("Work in progress", 16, false, None);

    assert!(output.contains("Work in progress"));
    assert!(!output.contains("(ready to send)"));
}

#[test]
fn view_renders_ai_origin_label_for_reply_draft() {
    let output = render_comment_with_reply_draft("AI suggestion", 13, false, Some("AI-originated"));

    assert!(output.contains("Origin: AI-originated"));
}
