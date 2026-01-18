//! Behavioural tests for full-screen diff context navigation.

#[path = "full_screen_diff_context_bdd/mod.rs"]
mod full_screen_diff_context_support;

use bubbletea_rs::Model;
use frankie::tui::app::ReviewApp;
use frankie::tui::components::test_utils::strip_ansi_codes;
use frankie::tui::messages::AppMsg;
use full_screen_diff_context_support::DiffContextState;
use full_screen_diff_context_support::ReviewCommentBuilder;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn state() -> DiffContextState {
    DiffContextState::default()
}

/// Error type for BDD test step failures.
type StepError = &'static str;

/// Result type for BDD test steps.
type StepResult = Result<(), StepError>;

impl DiffContextState {
    fn setup_app_with_comments(&self, comments: Vec<frankie::github::models::ReviewComment>) {
        self.app.set(ReviewApp::new(comments));
    }

    fn render_view(&self) -> StepResult {
        let view = self
            .app
            .with_ref(ReviewApp::view)
            .ok_or("app should be initialised before rendering view")?;
        self.rendered_view.set(view);
        Ok(())
    }

    fn view(&self) -> Result<String, StepError> {
        self.rendered_view
            .with_ref(Clone::clone)
            .ok_or("view should be rendered before inspection")
    }
}

#[given("a TUI with review comments that contain diff hunks")]
fn given_review_comments_with_hunks(state: &DiffContextState) {
    let first = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/main.rs")
        .line_number(10)
        .body("Check main")
        .diff_hunk("@@ -1 +1 @@\n+fn main() {}")
        .build();

    let second = ReviewCommentBuilder::new(2)
        .author("bob")
        .file_path("src/zzz.rs")
        .line_number(20)
        .body("Check helper")
        .diff_hunk("@@ -5 +5 @@\n+fn helper() {}")
        .build();

    state.setup_app_with_comments(vec![first, second]);
}

#[given("a TUI with review comments without diff hunks")]
fn given_review_comments_without_hunks(state: &DiffContextState) {
    let comment = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/main.rs")
        .line_number(10)
        .body("General feedback")
        .build();

    state.setup_app_with_comments(vec![comment]);
}

#[when("the full-screen diff context is opened")]
fn when_full_screen_context_opened(state: &DiffContextState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::ShowDiffContext))
        .ok_or("app should be initialised before opening diff context")?;
    Ok(())
}

#[when("the next hunk is selected")]
fn when_next_hunk_selected(state: &DiffContextState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::NextHunk))
        .ok_or("app should be initialised before navigating")?;
    Ok(())
}

#[when("the previous hunk is selected")]
fn when_previous_hunk_selected(state: &DiffContextState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::PreviousHunk))
        .ok_or("app should be initialised before navigating")?;
    Ok(())
}

#[when("the diff context is closed")]
fn when_diff_context_closed(state: &DiffContextState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::EscapePressed))
        .ok_or("app should be initialised before closing diff context")?;
    Ok(())
}

#[when("a navigation key is pressed in diff context")]
fn when_navigation_key_pressed_in_diff_context(state: &DiffContextState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::CursorDown))
        .ok_or("app should be initialised before sending navigation key")?;
    Ok(())
}

#[when("a filter key is pressed in diff context")]
fn when_filter_key_pressed_in_diff_context(state: &DiffContextState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::CycleFilter))
        .ok_or("app should be initialised before sending filter key")?;
    Ok(())
}

#[given("the second review comment is selected")]
fn given_second_review_comment_selected(state: &DiffContextState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::CursorDown))
        .ok_or("app should be initialised before moving selection")?;
    Ok(())
}

#[when("the view is rendered")]
fn when_view_is_rendered(state: &DiffContextState) -> StepResult {
    state.render_view()
}

#[then("the view shows hunk position {position}")]
fn then_view_shows_hunk_position(state: &DiffContextState, position: String) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains(&format!("Hunk {position}")),
        "expected hunk position {position} in view:\n{stripped}"
    );
    Ok(())
}

#[then("the view shows file path {file}")]
fn then_view_shows_file_path(state: &DiffContextState, file: String) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains(&format!("File: {file}")),
        "expected file path {file} in view:\n{stripped}"
    );
    Ok(())
}

#[then("the view shows no diff context placeholder")]
fn then_view_shows_placeholder(state: &DiffContextState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains("No diff context available"),
        "expected placeholder in view:\n{stripped}"
    );
    Ok(())
}

#[then("the review list is visible")]
fn then_review_list_visible(state: &DiffContextState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains("Filter:"),
        "expected review list view after exit:\n{stripped}"
    );
    Ok(())
}

#[then("the second review comment remains selected")]
fn then_second_review_comment_remains_selected(state: &DiffContextState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains("> [bob]"),
        "expected second comment to remain selected:\n{stripped}"
    );
    Ok(())
}

// Scenario bindings

#[scenario(path = "tests/features/full_screen_diff_context.feature", index = 0)]
fn diff_context_opens_with_first_hunk(state: DiffContextState) {
    let _ = state;
}

#[scenario(path = "tests/features/full_screen_diff_context.feature", index = 1)]
fn diff_context_moves_to_next_hunk(state: DiffContextState) {
    let _ = state;
}

#[scenario(path = "tests/features/full_screen_diff_context.feature", index = 2)]
fn diff_context_clamps_previous_at_start(state: DiffContextState) {
    let _ = state;
}

#[scenario(path = "tests/features/full_screen_diff_context.feature", index = 3)]
fn diff_context_blocks_navigation_keys(state: DiffContextState) {
    let _ = state;
}

#[scenario(path = "tests/features/full_screen_diff_context.feature", index = 4)]
fn diff_context_blocks_filter_keys(state: DiffContextState) {
    let _ = state;
}

#[scenario(path = "tests/features/full_screen_diff_context.feature", index = 5)]
fn diff_context_shows_placeholder_without_hunks(state: DiffContextState) {
    let _ = state;
}

#[scenario(path = "tests/features/full_screen_diff_context.feature", index = 6)]
fn diff_context_exit_returns_to_list(state: DiffContextState) {
    let _ = state;
}

#[scenario(path = "tests/features/full_screen_diff_context.feature", index = 7)]
fn diff_context_exit_preserves_selection(state: DiffContextState) {
    let _ = state;
}
