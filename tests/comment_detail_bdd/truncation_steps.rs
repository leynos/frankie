//! Truncation-related BDD step definitions for comment detail tests.

use super::state::{DetailState, ReviewCommentBuilder};
use frankie::tui::components::test_utils::strip_ansi_codes;
use frankie::tui::components::{CommentDetailComponent, CommentDetailViewContext};
use rstest_bdd_macros::{given, then, when};

/// Error type for BDD test step failures.
type StepError = &'static str;

/// Result type for BDD test steps.
type StepResult = Result<(), StepError>;

// Given steps for truncation tests

#[given("a TUI with a comment producing more lines than visible height")]
pub fn given_comment_producing_many_lines(detail_state: &DetailState) {
    // Create a comment with a long diff hunk that will produce many lines
    let diff_hunk = concat!(
        "@@ -1,10 +1,15 @@\n",
        "+line1\n",
        "+line2\n",
        "+line3\n",
        "+line4\n",
        "+line5\n",
        "+line6\n",
        "+line7\n",
        "+line8\n",
        "+line9\n",
        "+line10"
    );
    let builder = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/main.rs")
        .line_number(1)
        .body("Many lines of code")
        .diff_hunk(diff_hunk);
    let comment = builder.build();
    detail_state.standalone_comment.set(comment);
}

#[given("a TUI with a comment containing consecutive blank lines")]
pub fn given_comment_with_blank_lines(detail_state: &DetailState) {
    // Create a comment with blank lines in the diff hunk.
    // The diff hunk includes diff markers at the start of lines, so
    // "+ " prefix followed by nothing creates a blank line in the output.
    let diff_hunk = concat!(
        "@@ -1,5 +1,8 @@\n",
        "+line1\n",
        "+\n",
        "+\n",
        "+line4\n",
        "+line5"
    );
    let builder = ReviewCommentBuilder::new(1)
        .author("bob")
        .file_path("src/lib.rs")
        .line_number(1)
        .body("Code with blank lines")
        .diff_hunk(diff_hunk);
    let comment = builder.build();
    detail_state.standalone_comment.set(comment);
}

// When steps for truncation tests

#[when("the view is rendered with max height {height:usize}")]
pub fn when_view_rendered_with_max_height(detail_state: &DetailState, height: usize) -> StepResult {
    let comment = detail_state
        .standalone_comment
        .with_ref(Clone::clone)
        .ok_or("standalone comment should be set before rendering")?;
    let component = CommentDetailComponent::new();
    let ctx = CommentDetailViewContext {
        selected_comment: Some(&comment),
        max_width: 80,
        max_height: height,
        reply_draft: None,
    };
    let view = component.view(&ctx);
    detail_state.rendered_view.set(view);
    Ok(())
}

// Then steps for truncation tests

#[then("the output has at most {max:usize} lines")]
pub fn then_output_has_max_lines(detail_state: &DetailState, max: usize) -> StepResult {
    let view = detail_state.get_rendered_view()?;
    let line_count = view.lines().count();
    assert!(
        line_count <= max,
        "expected at most {max} lines, got {line_count}:\n{view}"
    );
    Ok(())
}

#[then("the last line is an ellipsis indicator")]
pub fn then_last_line_is_ellipsis(detail_state: &DetailState) -> StepResult {
    let view = detail_state.get_rendered_view()?;
    let stripped = strip_ansi_codes(&view);
    let last_line = stripped
        .lines()
        .last()
        .ok_or("rendered view should have at least one line")?;
    assert_eq!(
        last_line, "...",
        "expected last line to be '...', got '{last_line}'"
    );
    Ok(())
}

#[then("blank lines are preserved in the truncated output")]
pub fn then_blank_lines_preserved(detail_state: &DetailState) -> StepResult {
    let view = detail_state.get_rendered_view()?;
    let stripped = strip_ansi_codes(&view);
    // The output should contain at least one line that is just a diff marker "+"
    // (representing a blank line in the diff context) within the code section.
    // Note: The diff hunk lines like "+" represent blank added lines.
    let has_diff_blank = stripped.lines().any(|line| line == "+");
    assert!(
        has_diff_blank,
        "expected diff blank line '+' to be preserved in output:\n{stripped}"
    );
    Ok(())
}
