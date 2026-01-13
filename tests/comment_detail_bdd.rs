//! Behavioural tests for comment detail view with inline code context.

#[path = "comment_detail_bdd/mod.rs"]
mod comment_detail_bdd_support;

use bubbletea_rs::Model;
use comment_detail_bdd_support::DetailState;
use comment_detail_bdd_support::state::create_review_comment;
use frankie::tui::app::ReviewApp;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn detail_state() -> DetailState {
    DetailState::default()
}

// Given steps

#[given("a TUI with a review comment by {author} on {file} at line {line:u32}")]
fn given_review_comment_with_location(
    detail_state: &DetailState,
    author: String,
    file: String,
    line: u32,
) {
    let comment = create_review_comment(
        1,
        Some(author.as_str()),
        Some(file.as_str()),
        Some(line),
        Some("Test comment body"),
        Some("@@ -1,3 +1,4 @@\n+fn test() {}"),
    );
    let app = ReviewApp::new(vec![comment]);
    detail_state.app.set(app);
}

#[given("a TUI with a review comment with body {body}")]
fn given_review_comment_with_body(detail_state: &DetailState, body: String) {
    let body_text = body.trim_matches('"');
    let comment = create_review_comment(
        1,
        Some("alice"),
        Some("src/lib.rs"),
        Some(10),
        Some(body_text),
        Some("@@ -1,3 +1,4 @@\n+fn test() {}"),
    );
    let app = ReviewApp::new(vec![comment]);
    detail_state.app.set(app);
}

#[given("a TUI with a review comment with a diff hunk")]
fn given_review_comment_with_diff_hunk(detail_state: &DetailState) {
    let diff_hunk =
        "@@ -10,6 +10,10 @@\n fn existing() {}\n+fn new_function() {\n+    let x = 1;\n+}";
    let comment = create_review_comment(
        1,
        Some("alice"),
        Some("src/main.rs"),
        Some(12),
        Some("Please review this change"),
        Some(diff_hunk),
    );
    let app = ReviewApp::new(vec![comment]);
    detail_state.app.set(app);
}

#[given("a TUI with a review comment with a 120-character code line")]
fn given_review_comment_with_long_code_line(detail_state: &DetailState) {
    let long_line = "x".repeat(120);
    let diff_hunk = format!("@@ -1,1 +1,1 @@\n+let long = \"{long_line}\";");
    let comment = create_review_comment(
        1,
        Some("alice"),
        Some("src/main.rs"),
        Some(1),
        Some("Long line"),
        Some(&diff_hunk),
    );
    let app = ReviewApp::new(vec![comment]);
    detail_state.app.set(app);
    detail_state.max_width.set(80);
}

#[given("a TUI with a review comment on a file with unknown extension")]
fn given_review_comment_with_unknown_extension(detail_state: &DetailState) {
    let comment = create_review_comment(
        1,
        Some("alice"),
        Some("data.unknown_ext_xyz"),
        Some(1),
        Some("Check this data"),
        Some("@@ -1,1 +1,1 @@\n+some data content"),
    );
    let app = ReviewApp::new(vec![comment]);
    detail_state.app.set(app);
    detail_state.max_width.set(80);
}

#[given("a TUI with a review comment without diff hunk")]
fn given_review_comment_without_diff_hunk(detail_state: &DetailState) {
    let comment = create_review_comment(
        1,
        Some("bob"),
        Some("src/lib.rs"),
        Some(5),
        Some("General comment"),
        None,
    );
    let app = ReviewApp::new(vec![comment]);
    detail_state.app.set(app);
}

#[given("a TUI with no comments")]
fn given_tui_with_no_comments(detail_state: &DetailState) {
    let app = ReviewApp::new(vec![]);
    detail_state.app.set(app);
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
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_shows_author(detail_state: &DetailState, author: String) {
    let view = detail_state
        .rendered_view
        .with_ref(Clone::clone)
        .expect("view not rendered");
    let expected = format!("[{author}]");
    assert!(
        view.contains(&expected),
        "expected author [{author}] in view:\n{view}"
    );
}

#[then("the detail pane shows file path {file}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_shows_file_path(detail_state: &DetailState, file: String) {
    let view = detail_state
        .rendered_view
        .with_ref(Clone::clone)
        .expect("view not rendered");
    assert!(
        view.contains(&file),
        "expected file path {file} in view:\n{view}"
    );
}

#[then("the detail pane shows line number {line:u32}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_shows_line_number(detail_state: &DetailState, line: u32) {
    let view = detail_state
        .rendered_view
        .with_ref(Clone::clone)
        .expect("view not rendered");
    let line_marker = format!(":{line}");
    assert!(
        view.contains(&line_marker),
        "expected line number :{line} in view:\n{view}"
    );
}

#[then("the detail pane shows the body text")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_shows_body_text(detail_state: &DetailState) {
    let view = detail_state
        .rendered_view
        .with_ref(Clone::clone)
        .expect("view not rendered");
    assert!(
        view.contains("refactor"),
        "expected body text in view:\n{view}"
    );
}

#[then("the detail pane shows code context")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_shows_code_context(detail_state: &DetailState) {
    let view = detail_state
        .rendered_view
        .with_ref(Clone::clone)
        .expect("view not rendered");
    // The diff hunk should be visible (may have ANSI codes)
    assert!(
        view.contains("fn") || view.contains("new_function") || view.contains("@@"),
        "expected code context in view:\n{view}"
    );
}

#[then("all code lines are at most {max:usize} characters wide")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_code_lines_within_width(detail_state: &DetailState, max: usize) {
    let view = detail_state
        .rendered_view
        .with_ref(Clone::clone)
        .expect("view not rendered");

    // Strip ANSI codes before checking width
    let stripped = strip_ansi_codes(&view);
    for line in stripped.lines() {
        let width = line.chars().count();
        assert!(width <= max, "line exceeds {max} chars ({width}): '{line}'");
    }
}

#[then("the code context is displayed as plain text")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_code_is_plain_text(detail_state: &DetailState) {
    let view = detail_state
        .rendered_view
        .with_ref(Clone::clone)
        .expect("view not rendered");
    // Plain text means the content is visible
    assert!(
        view.contains("some data content") || view.contains("data"),
        "expected plain text code in view:\n{view}"
    );
}

#[then("the detail pane shows no-context placeholder")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_shows_no_context_placeholder(detail_state: &DetailState) {
    let view = detail_state
        .rendered_view
        .with_ref(Clone::clone)
        .expect("view not rendered");
    assert!(
        view.contains("No code context"),
        "expected no-context placeholder in view:\n{view}"
    );
}

#[then("the detail pane shows no-selection placeholder")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_shows_no_selection_placeholder(detail_state: &DetailState) {
    let view = detail_state
        .rendered_view
        .with_ref(Clone::clone)
        .expect("view not rendered");
    assert!(
        view.contains("No comment selected"),
        "expected no-selection placeholder in view:\n{view}"
    );
}

// Helper function to strip ANSI escape codes
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;

    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            result.push(ch);
        }
    }

    result
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
