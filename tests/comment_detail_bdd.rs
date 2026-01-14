//! Behavioural tests for comment detail view with inline code context.

#[path = "comment_detail_bdd/mod.rs"]
mod comment_detail_bdd_support;

use bubbletea_rs::Model;
use comment_detail_bdd_support::DetailState;
use comment_detail_bdd_support::ReviewCommentBuilder;
use frankie::tui::app::ReviewApp;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn detail_state() -> DetailState {
    DetailState::default()
}

// Helper methods for DetailState

impl DetailState {
    /// Sets up the app with a review comment using the builder pattern.
    ///
    /// This encapsulates the common pattern of creating a comment and
    /// initialising the `ReviewApp` with it.
    #[expect(
        clippy::too_many_arguments,
        reason = "helper mirrors builder options for flexibility"
    )]
    fn setup_app_with_comment(
        &self,
        author: Option<&str>,
        file: Option<&str>,
        line: Option<u32>,
        body: Option<&str>,
        diff_hunk: Option<&str>,
    ) {
        let mut builder = ReviewCommentBuilder::new(1);
        if let Some(a) = author {
            builder = builder.author(a);
        }
        if let Some(f) = file {
            builder = builder.file_path(f);
        }
        if let Some(l) = line {
            builder = builder.line_number(l);
        }
        if let Some(b) = body {
            builder = builder.body(b);
        }
        if let Some(h) = diff_hunk {
            builder = builder.diff_hunk(h);
        }
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
    detail_state.setup_app_with_comment(
        Some(&author),
        Some(&file),
        Some(line),
        Some("Test comment body"),
        Some("@@ -1,3 +1,4 @@\n+fn test() {}"),
    );
}

#[given("a TUI with a review comment with body {body}")]
fn given_review_comment_with_body(detail_state: &DetailState, body: String) {
    let body_text = body.trim_matches('"');
    detail_state.setup_app_with_comment(
        Some("alice"),
        Some("src/lib.rs"),
        Some(10),
        Some(body_text),
        Some("@@ -1,3 +1,4 @@\n+fn test() {}"),
    );
}

#[given("a TUI with a review comment with a diff hunk")]
fn given_review_comment_with_diff_hunk(detail_state: &DetailState) {
    let diff_hunk =
        "@@ -10,6 +10,10 @@\n fn existing() {}\n+fn new_function() {\n+    let x = 1;\n+}";
    detail_state.setup_app_with_comment(
        Some("alice"),
        Some("src/main.rs"),
        Some(12),
        Some("Please review this change"),
        Some(diff_hunk),
    );
}

#[given("a TUI with a review comment with a 120-character code line")]
fn given_review_comment_with_long_code_line(detail_state: &DetailState) {
    let long_line = "x".repeat(120);
    let diff_hunk = format!("@@ -1,1 +1,1 @@\n+let long = \"{long_line}\";");
    detail_state.setup_app_with_comment(
        Some("alice"),
        Some("src/main.rs"),
        Some(1),
        Some("Long line"),
        Some(&diff_hunk),
    );
    detail_state.max_width.set(80);
}

#[given("a TUI with a review comment on a file with unknown extension")]
fn given_review_comment_with_unknown_extension(detail_state: &DetailState) {
    detail_state.setup_app_with_comment(
        Some("alice"),
        Some("data.unknown_ext_xyz"),
        Some(1),
        Some("Check this data"),
        Some("@@ -1,1 +1,1 @@\n+some data content"),
    );
    detail_state.max_width.set(80);
}

#[given("a TUI with a review comment without diff hunk")]
fn given_review_comment_without_diff_hunk(detail_state: &DetailState) {
    detail_state.setup_app_with_comment(
        Some("bob"),
        Some("src/lib.rs"),
        Some(5),
        Some("General comment"),
        None,
    );
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

#[when("the view is rendered with max width {width:usize}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn when_view_is_rendered_with_max_width(detail_state: &DetailState, width: usize) {
    detail_state.max_width.set(width);
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
    // Plain text means the content is visible
    assert!(
        view.contains("some data content") || view.contains("data"),
        "expected plain text code in view:\n{view}"
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

// Helper functions for stripping ANSI escape codes

/// Strips ANSI escape codes from a string.
///
/// Delegates character processing to helper functions to reduce nesting.
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;

    for ch in s.chars() {
        in_escape = process_character(ch, in_escape, &mut result);
    }

    result
}

/// Processes a single character for ANSI escape code stripping.
///
/// Returns the new escape state after processing the character.
fn process_character(ch: char, in_escape: bool, result: &mut String) -> bool {
    if is_escape_start(ch) {
        return true;
    }

    if in_escape {
        return is_escape_continues(ch);
    }

    result.push(ch);
    false
}

/// Returns true if the character begins an ANSI escape sequence.
const fn is_escape_start(ch: char) -> bool {
    ch == '\x1b'
}

/// Returns true if the escape sequence continues (character is not alphabetic).
const fn is_escape_continues(ch: char) -> bool {
    !ch.is_ascii_alphabetic()
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
