//! Behavioural tests for time-travel navigation.

#[path = "time_travel_bdd/mod.rs"]
mod time_travel_support;

use std::sync::Arc;

use bubbletea_rs::Model;
use frankie::local::{CommitSnapshot, LineMappingVerification};
use frankie::tui::app::ReviewApp;
use frankie::tui::components::test_utils::strip_ansi_codes;
use frankie::tui::messages::AppMsg;
use frankie::tui::state::TimeTravelState;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use time_travel_support::{MockGitOperations, TimeTravelTestState};

use frankie::tui::components::test_utils::ReviewCommentBuilder;

/// Creates a mock time-travel state for testing at a specific history index.
fn create_mock_time_travel_state_at_index(index: usize) -> TimeTravelState {
    let commit_history = vec![
        "abc1234567890".to_owned(),
        "def5678901234".to_owned(),
        "ghi9012345678".to_owned(),
    ];
    let sha = commit_history.get(index).cloned().unwrap_or_default();
    let snapshot = CommitSnapshot::with_file_content(
        sha,
        "Fix login validation".to_owned(),
        "Alice".to_owned(),
        chrono::Utc::now(),
        "src/auth.rs".to_owned(),
        "fn login() {\n    // validation\n}".to_owned(),
    );
    let line_mapping = Some(LineMappingVerification::exact(42));
    let mut state = TimeTravelState::new(
        snapshot.clone(),
        "src/auth.rs".to_owned(),
        Some(42),
        line_mapping.clone(),
        commit_history,
    );
    // Update to the specified index
    state.update_snapshot(snapshot, line_mapping, index);
    state
}

/// Creates a mock time-travel state for testing.
fn create_mock_time_travel_state() -> TimeTravelState {
    create_mock_time_travel_state_at_index(0)
}

#[fixture]
fn state() -> TimeTravelTestState {
    TimeTravelTestState::default()
}

/// Error type for BDD test step failures.
type StepError = &'static str;

/// Result type for BDD test steps.
type StepResult = Result<(), StepError>;

impl TimeTravelTestState {
    fn setup_app_with_comments(&self, comments: Vec<frankie::github::models::ReviewComment>) {
        let mut app = ReviewApp::new(comments);

        // Conditionally add git ops
        if self.repo_available.with_ref(|r| *r).unwrap_or(true) {
            let mock_git = self
                .mock_git_ops
                .with_ref(Clone::clone)
                .unwrap_or_else(|| Arc::new(MockGitOperations::new()));
            app = app.with_git_ops(mock_git, "HEAD123".to_owned());
        }

        self.app.set(app);
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

// Given steps

#[given("a TUI with review comments that have commit SHAs")]
fn given_comments_with_commit_shas(state: &TimeTravelTestState) {
    // Set defaults
    state.repo_available.set(true);
    state.commit_found.set(true);
}

#[given("a local repository is available")]
fn given_local_repository_available(state: &TimeTravelTestState) {
    state.repo_available.set(true);
    let mock = MockGitOperations::new();
    state.mock_git_ops.set(Arc::new(mock));

    // Now set up the app with comments
    let comment = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/auth.rs")
        .line_number(42)
        .body("Check validation")
        .commit_sha("abc1234567890")
        .build();
    state.setup_app_with_comments(vec![comment]);
}

#[given("no local repository is available")]
fn given_no_local_repository(state: &TimeTravelTestState) {
    state.repo_available.set(false);

    let comment = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/auth.rs")
        .line_number(42)
        .body("Check validation")
        .commit_sha("abc1234567890")
        .build();
    state.setup_app_with_comments(vec![comment]);
}

#[given("the commit is not found in the repository")]
fn given_commit_not_found(state: &TimeTravelTestState) {
    state.commit_found.set(false);
    let mock = MockGitOperations::new().with_commit_exists(false);
    state.mock_git_ops.set(Arc::new(mock));

    let comment = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/auth.rs")
        .line_number(42)
        .body("Check validation")
        .commit_sha("nonexistent123")
        .build();
    state.setup_app_with_comments(vec![comment]);
}

#[given("the line mapping shows exact match")]
fn given_line_mapping_exact(state: &TimeTravelTestState) {
    // Default mock already returns exact match
    let _ = state;
}

#[given("time-travel mode is entered for the selected comment")]
fn given_time_travel_entered(state: &TimeTravelTestState) -> StepResult {
    // First enter time-travel mode (this sets loading state)
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::EnterTimeTravel))
        .ok_or("app should be initialised before entering time-travel")?;

    // Then simulate the loaded callback with mock data
    let mock_state = create_mock_time_travel_state();
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::TimeTravelLoaded(Box::new(mock_state))))
        .ok_or("app should handle loaded message")?;
    Ok(())
}

#[given("the previous commit is navigated to")]
fn given_previous_commit(state: &TimeTravelTestState) -> StepResult {
    // Send the navigation message
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::PreviousCommit))
        .ok_or("app should be initialised before navigation")?;

    // Simulate the navigation completed callback
    let mock_state = create_mock_time_travel_state_at_index(1);
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::CommitNavigated(Box::new(mock_state))))
        .ok_or("app should handle navigated message")?;
    Ok(())
}

// When steps

#[when("time-travel mode is entered for the selected comment")]
fn when_time_travel_entered(state: &TimeTravelTestState) -> StepResult {
    // First enter time-travel mode (this sets loading state)
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::EnterTimeTravel))
        .ok_or("app should be initialised before entering time-travel")?;

    // Then simulate the loaded callback with mock data
    let mock_state = create_mock_time_travel_state();
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::TimeTravelLoaded(Box::new(mock_state))))
        .ok_or("app should handle loaded message")?;
    Ok(())
}

#[when("the previous commit is navigated to")]
fn when_previous_commit(state: &TimeTravelTestState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::PreviousCommit))
        .ok_or("app should be initialised before navigation")?;

    // Simulate the navigation completed callback (move to index 1)
    let mock_state = create_mock_time_travel_state_at_index(1);
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::CommitNavigated(Box::new(mock_state))))
        .ok_or("app should handle navigated message")?;
    Ok(())
}

#[when("the next commit is navigated to")]
fn when_next_commit(state: &TimeTravelTestState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::NextCommit))
        .ok_or("app should be initialised before navigation")?;

    // Simulate the navigation completed callback (move back to index 0)
    let mock_state = create_mock_time_travel_state_at_index(0);
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::CommitNavigated(Box::new(mock_state))))
        .ok_or("app should handle navigated message")?;
    Ok(())
}

#[when("time-travel mode is exited")]
fn when_time_travel_exited(state: &TimeTravelTestState) -> StepResult {
    state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::ExitTimeTravel))
        .ok_or("app should be initialised before exiting time-travel")?;
    Ok(())
}

#[when("the view is rendered")]
fn when_view_rendered(state: &TimeTravelTestState) -> StepResult {
    state.render_view()
}

// Then steps

#[then("the view shows the time-travel header")]
fn then_view_shows_header(state: &TimeTravelTestState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains("Commit:"),
        "expected time-travel header in view:\n{stripped}"
    );
    Ok(())
}

#[then("the view shows the commit message")]
fn then_view_shows_commit_message(state: &TimeTravelTestState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains("Fix login validation"),
        "expected commit message in view:\n{stripped}"
    );
    Ok(())
}

#[then("the view shows the file path")]
fn then_view_shows_file_path(state: &TimeTravelTestState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains("src/auth.rs"),
        "expected file path in view:\n{stripped}"
    );
    Ok(())
}

#[then("the view shows line mapping status")]
fn then_view_shows_line_mapping(state: &TimeTravelTestState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    // Check for line mapping indicator (exact match symbol)
    assert!(
        stripped.contains("42") || stripped.contains("Line"),
        "expected line mapping status in view:\n{stripped}"
    );
    Ok(())
}

#[then("the view shows commit position {position}")]
fn then_view_shows_commit_position(state: &TimeTravelTestState, position: String) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains(&position),
        "expected commit position {position} in view:\n{stripped}"
    );
    Ok(())
}

#[then("the view shows commit not found error")]
fn then_view_shows_commit_not_found(state: &TimeTravelTestState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains("not found"),
        "expected commit not found error in view:\n{stripped}"
    );
    Ok(())
}

#[then("the view shows no repository error")]
fn then_view_shows_no_repository(state: &TimeTravelTestState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains("No local repository"),
        "expected no repository error in view:\n{stripped}"
    );
    Ok(())
}

#[then("the review list is visible")]
fn then_review_list_visible(state: &TimeTravelTestState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    assert!(
        stripped.contains("Filter:"),
        "expected review list view:\n{stripped}"
    );
    Ok(())
}

// Scenario bindings

#[scenario(path = "tests/features/time_travel.feature", index = 0)]
fn time_travel_enter_mode(state: TimeTravelTestState) {
    let _ = state;
}

#[scenario(path = "tests/features/time_travel.feature", index = 1)]
fn time_travel_line_mapping(state: TimeTravelTestState) {
    let _ = state;
}

#[scenario(path = "tests/features/time_travel.feature", index = 2)]
fn time_travel_previous_commit(state: TimeTravelTestState) {
    let _ = state;
}

#[scenario(path = "tests/features/time_travel.feature", index = 3)]
fn time_travel_next_commit(state: TimeTravelTestState) {
    let _ = state;
}

#[scenario(path = "tests/features/time_travel.feature", index = 4)]
fn time_travel_missing_commit(state: TimeTravelTestState) {
    let _ = state;
}

#[scenario(path = "tests/features/time_travel.feature", index = 5)]
fn time_travel_missing_repository(state: TimeTravelTestState) {
    let _ = state;
}

#[scenario(path = "tests/features/time_travel.feature", index = 6)]
fn time_travel_exit_mode(state: TimeTravelTestState) {
    let _ = state;
}
