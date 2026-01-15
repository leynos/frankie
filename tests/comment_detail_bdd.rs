//! Behavioural tests for comment detail view with inline code context.

#[path = "comment_detail_bdd/mod.rs"]
mod comment_detail_bdd_support;

use bubbletea_rs::Model;
use comment_detail_bdd_support::DetailState;
use comment_detail_bdd_support::ReviewCommentBuilder;
use frankie::tui::app::ReviewApp;
use frankie::tui::components::test_utils::strip_ansi_codes;
use frankie::tui::components::{CommentDetailComponent, CommentDetailViewContext};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn detail_state() -> DetailState {
    DetailState::default()
}

// Helper methods for DetailState

impl DetailState {
    /// Sets up the app with a review comment from a pre-configured builder.
    ///
    /// This encapsulates the common pattern of building a comment and
    /// initialising the `ReviewApp` with it.
    fn setup_app_with_comment(&self, builder: ReviewCommentBuilder) {
        let comment = builder.build();
        let app = ReviewApp::new(vec![comment]);
        self.app.set(app);
    }

    /// Sets up an empty app with no comments.
    fn setup_empty_app(&self) {
        let app = ReviewApp::new(vec![]);
        self.app.set(app);
    }

    /// Gets the rendered view string.
    #[expect(clippy::expect_used, reason = "test helper; panics acceptable")]
    fn get_rendered_view(&self) -> String {
        self.rendered_view
            .with_ref(Clone::clone)
            .expect("view not rendered")
    }

    /// Asserts that the rendered view contains the expected text.
    fn assert_view_contains(&self, expected: &str, diagnostic: &str) {
        let view = self.get_rendered_view();
        assert!(view.contains(expected), "{diagnostic}:\n{view}");
    }
}

// Given steps

#[given("a TUI with a review comment by {author} on {file} at line {line:u32}")]
fn given_review_comment_with_location(
    detail_state: &DetailState,
    author: String,
    file: String,
    line: u32,
) {
    let builder = ReviewCommentBuilder::new(1)
        .author(&author)
        .file_path(&file)
        .line_number(line)
        .body("Test comment body")
        .diff_hunk("@@ -1,3 +1,4 @@\n+fn test() {}");
    detail_state.setup_app_with_comment(builder);
}

#[given("a TUI with a review comment with body {body}")]
fn given_review_comment_with_body(detail_state: &DetailState, body: String) {
    let body_text = body.trim_matches('"');
    let builder = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/lib.rs")
        .line_number(10)
        .body(body_text)
        .diff_hunk("@@ -1,3 +1,4 @@\n+fn test() {}");
    detail_state.setup_app_with_comment(builder);
}

#[given("a TUI with a review comment with a diff hunk")]
fn given_review_comment_with_diff_hunk(detail_state: &DetailState) {
    let diff_hunk =
        "@@ -10,6 +10,10 @@\n fn existing() {}\n+fn new_function() {\n+    let x = 1;\n+}";
    let builder = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/main.rs")
        .line_number(12)
        .body("Please review this change")
        .diff_hunk(diff_hunk);
    detail_state.setup_app_with_comment(builder);
}

#[given("a TUI with a review comment with a 120-character code line")]
fn given_review_comment_with_long_code_line(detail_state: &DetailState) {
    let long_line = "x".repeat(120);
    let diff_hunk = format!("@@ -1,1 +1,1 @@\n+let long = \"{long_line}\";");
    let builder = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/main.rs")
        .line_number(1)
        .body("Long line")
        .diff_hunk(&diff_hunk);
    detail_state.setup_app_with_comment(builder);
}

#[given("a TUI with a review comment on a file with unknown extension")]
fn given_review_comment_with_unknown_extension(detail_state: &DetailState) {
    let builder = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("data.unknown_ext_xyz")
        .line_number(1)
        .body("Check this data")
        .diff_hunk("@@ -1,1 +1,1 @@\n+some data content");
    detail_state.setup_app_with_comment(builder);
}

#[given("a TUI with a review comment without diff hunk")]
fn given_review_comment_without_diff_hunk(detail_state: &DetailState) {
    let builder = ReviewCommentBuilder::new(1)
        .author("bob")
        .file_path("src/lib.rs")
        .line_number(5)
        .body("General comment");
    detail_state.setup_app_with_comment(builder);
}

#[given("a TUI with no comments")]
fn given_tui_with_no_comments(detail_state: &DetailState) {
    detail_state.setup_empty_app();
}

// When steps

#[when("the view is rendered")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn when_view_is_rendered(detail_state: &DetailState) {
    let view = detail_state
        .app
        .with_ref(ReviewApp::view)
        .expect("app not initialised");
    detail_state.rendered_view.set(view);
}

// Then steps

#[then("the detail pane shows author {author}")]
fn then_shows_author(detail_state: &DetailState, author: String) {
    let expected = format!("[{author}]");
    detail_state.assert_view_contains(&expected, &format!("expected author [{author}] in view"));
}

#[then("the detail pane shows file path {file}")]
fn then_shows_file_path(detail_state: &DetailState, file: String) {
    detail_state.assert_view_contains(&file, &format!("expected file path {file} in view"));
}

#[then("the detail pane shows line number {line:u32}")]
fn then_shows_line_number(detail_state: &DetailState, line: u32) {
    let line_marker = format!(":{line}");
    detail_state.assert_view_contains(
        &line_marker,
        &format!("expected line number :{line} in view"),
    );
}

#[then("the detail pane shows the body text")]
fn then_shows_body_text(detail_state: &DetailState) {
    detail_state.assert_view_contains("refactor", "expected body text in view");
}

#[then("the detail pane shows code context")]
fn then_shows_code_context(detail_state: &DetailState) {
    let view = detail_state.get_rendered_view();
    // The diff hunk should be visible (may have ANSI codes)
    assert!(
        view.contains("fn") || view.contains("new_function") || view.contains("@@"),
        "expected code context in view:\n{view}"
    );
}

#[then("all code lines are at most {max:usize} characters wide")]
fn then_code_lines_within_width(detail_state: &DetailState, max: usize) {
    let view = detail_state.get_rendered_view();

    // Strip ANSI codes before checking width
    let stripped = strip_ansi_codes(&view);
    for line in stripped.lines() {
        let width = line.chars().count();
        assert!(width <= max, "line exceeds {max} chars ({width}): '{line}'");
    }
}

#[then("the code context is displayed as plain text")]
fn then_code_is_plain_text(detail_state: &DetailState) {
    let view = detail_state.get_rendered_view();
    // Plain text means the content is visible and not syntax highlighted
    assert!(
        view.contains("some data content") || view.contains("data"),
        "expected plain text code in view:\n{view}"
    );
    // Verify no ANSI escape codes are present (proves plain text fallback)
    assert!(
        !view.contains("\x1b["),
        "expected no ANSI codes for plain text fallback:\n{view}"
    );
}

#[then("the detail pane shows no-context placeholder")]
fn then_shows_no_context_placeholder(detail_state: &DetailState) {
    detail_state.assert_view_contains("No code context", "expected no-context placeholder in view");
}

#[then("the detail pane shows no-selection placeholder")]
fn then_shows_no_selection_placeholder(detail_state: &DetailState) {
    detail_state.assert_view_contains(
        "No comment selected",
        "expected no-selection placeholder in view",
    );
}

// Scenario bindings

#[scenario(path = "tests/features/comment_detail.feature", index = 0)]
fn comment_detail_shows_author_and_file(detail_state: DetailState) {
    let _ = detail_state;
}

#[scenario(path = "tests/features/comment_detail.feature", index = 1)]
fn comment_detail_shows_body(detail_state: DetailState) {
    let _ = detail_state;
}

#[scenario(path = "tests/features/comment_detail.feature", index = 2)]
fn comment_detail_shows_code_context(detail_state: DetailState) {
    let _ = detail_state;
}

#[scenario(path = "tests/features/comment_detail.feature", index = 3)]
fn code_context_wraps_to_80_columns(detail_state: DetailState) {
    let _ = detail_state;
}

#[scenario(path = "tests/features/comment_detail.feature", index = 4)]
fn fallback_to_plain_text(detail_state: DetailState) {
    let _ = detail_state;
}

#[scenario(path = "tests/features/comment_detail.feature", index = 5)]
fn shows_no_context_placeholder(detail_state: DetailState) {
    let _ = detail_state;
}

#[scenario(path = "tests/features/comment_detail.feature", index = 6)]
fn shows_no_selection_placeholder(detail_state: DetailState) {
    let _ = detail_state;
}

// Additional Given steps for truncation tests

#[given("a TUI with a comment producing more lines than visible height")]
fn given_comment_producing_many_lines(detail_state: &DetailState) {
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
fn given_comment_with_blank_lines(detail_state: &DetailState) {
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

// Additional When steps for truncation tests

#[when("the view is rendered with max height {height:usize}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn when_view_rendered_with_max_height(detail_state: &DetailState, height: usize) {
    let comment = detail_state
        .standalone_comment
        .with_ref(Clone::clone)
        .expect("comment not set");
    let component = CommentDetailComponent::new();
    let ctx = CommentDetailViewContext {
        selected_comment: Some(&comment),
        max_width: 80,
        max_height: height,
    };
    let view = component.view(&ctx);
    detail_state.rendered_view.set(view);
}

// Additional Then steps for truncation tests

#[then("the output has at most {max:usize} lines")]
fn then_output_has_max_lines(detail_state: &DetailState, max: usize) {
    let view = detail_state.get_rendered_view();
    let line_count = view.lines().count();
    assert!(
        line_count <= max,
        "expected at most {max} lines, got {line_count}:\n{view}"
    );
}

#[then("the last line is an ellipsis indicator")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_last_line_is_ellipsis(detail_state: &DetailState) {
    let view = detail_state.get_rendered_view();
    let stripped = strip_ansi_codes(&view);
    let last_line = stripped
        .lines()
        .last()
        .expect("view should have at least one line");
    assert_eq!(
        last_line, "...",
        "expected last line to be '...', got '{last_line}'"
    );
}

#[then("blank lines are preserved in the truncated output")]
fn then_blank_lines_preserved(detail_state: &DetailState) {
    let view = detail_state.get_rendered_view();
    let stripped = strip_ansi_codes(&view);
    // The output should contain at least one line that is just a diff marker "+"
    // (representing a blank line in the diff context) within the code section.
    // Note: The diff hunk lines like "+" represent blank added lines.
    let has_diff_blank = stripped.lines().any(|line| line == "+");
    assert!(
        has_diff_blank,
        "expected diff blank line '+' to be preserved in output:\n{stripped}"
    );
}

// Scenario bindings for truncation tests

#[scenario(path = "tests/features/comment_detail.feature", index = 7)]
fn detail_pane_truncates_to_max_height(detail_state: DetailState) {
    let _ = detail_state;
}

#[scenario(path = "tests/features/comment_detail.feature", index = 8)]
fn detail_pane_preserves_blank_lines(detail_state: DetailState) {
    let _ = detail_state;
}
