//! Behavioural tests for time-travel navigation.

#![expect(clippy::expect_used, reason = "Test setup invariants")]

#[path = "time_travel_bdd/mod.rs"]
mod time_travel_support;

use std::sync::Arc;

use bubbletea_rs::Model;
use frankie::local::{CommitMetadata, CommitSnapshot, LineMappingVerification};
use frankie::tui::app::ReviewApp;
use frankie::tui::components::test_utils::strip_ansi_codes;
use frankie::tui::messages::AppMsg;
use frankie::tui::state::{TimeTravelInitParams, TimeTravelState};
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
    let sha = commit_history.get(index).cloned().expect(
        "invalid commit index for commit_history in create_mock_time_travel_state_at_index",
    );
    let metadata = CommitMetadata::new(
        sha,
        "Fix login validation".to_owned(),
        "Alice".to_owned(),
        chrono::Utc::now(),
    );
    let snapshot = CommitSnapshot::with_file_content(
        metadata,
        "src/auth.rs".to_owned(),
        "fn login() {\n    // validation\n}".to_owned(),
    );
    let line_mapping = Some(LineMappingVerification::exact(42));
    TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: "src/auth.rs".to_owned(),
        original_line: Some(42),
        line_mapping,
        commit_history,
        current_index: index,
    })
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
    /// Creates a default review comment for testing.
    fn default_comment() -> frankie::github::models::ReviewComment {
        ReviewCommentBuilder::new(1)
            .author("alice")
            .file_path("src/auth.rs")
            .line_number(42)
            .body("Check validation")
            .commit_sha("abc1234567890")
            .build()
    }

    /// Creates a review comment with a specific commit SHA.
    fn comment_with_sha(sha: &str) -> frankie::github::models::ReviewComment {
        ReviewCommentBuilder::new(1)
            .author("alice")
            .file_path("src/auth.rs")
            .line_number(42)
            .body("Check validation")
            .commit_sha(sha)
            .build()
    }

    /// Sets up the repository with the given availability and optional mock.
    fn setup_repository(&self, available: bool, mock: Option<Arc<MockGitOperations>>) {
        self.repo_available.set(available);
        if let Some(m) = mock {
            self.mock_git_ops.set(m);
        }
    }

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

    /// Handles a message and optionally simulates a callback.
    fn handle_with_callback(&self, msg: &AppMsg, callback: Option<AppMsg>) -> StepResult {
        self.app
            .with_mut(|app| app.handle_message(msg))
            .ok_or("app should be initialised before handling message")?;

        if let Some(cb_msg) = callback {
            self.app
                .with_mut(|app| app.handle_message(&cb_msg))
                .ok_or("app should handle callback message")?;
        }
        Ok(())
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

    /// Asserts that the view contains the expected string.
    fn assert_view_contains(&self, expected: &str) -> StepResult {
        let view = self.view()?;
        let stripped = strip_ansi_codes(&view);
        if stripped.contains(expected) {
            Ok(())
        } else {
            Err("expected string not found in view")
        }
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
    state.setup_repository(true, Some(Arc::new(MockGitOperations::new())));
    state.setup_app_with_comments(vec![TimeTravelTestState::default_comment()]);
}

#[given("no local repository is available")]
fn given_no_local_repository(state: &TimeTravelTestState) {
    state.setup_repository(false, None);
    state.setup_app_with_comments(vec![TimeTravelTestState::default_comment()]);
}

#[given("the commit is not found in the repository")]
fn given_commit_not_found(state: &TimeTravelTestState) {
    state.commit_found.set(false);
    let mock = MockGitOperations::new().with_commit_exists(false);
    state.setup_repository(true, Some(Arc::new(mock)));
    state.setup_app_with_comments(vec![TimeTravelTestState::comment_with_sha(
        "nonexistent123",
    )]);
}

#[given("the line mapping shows exact match")]
fn given_line_mapping_exact(state: &TimeTravelTestState) {
    // Default mock already returns exact match
    let _ = state;
}

#[given("the line mapping shows line moved from {from} to {to}")]
fn given_line_mapping_moved(state: &TimeTravelTestState, from: u32, to: u32) {
    let mock = MockGitOperations::new().with_line_mapping(LineMappingVerification::moved(from, to));
    state.setup_repository(true, Some(Arc::new(mock)));
    state.setup_app_with_comments(vec![TimeTravelTestState::default_comment()]);
}

#[given("the line mapping shows line {line} deleted")]
fn given_line_mapping_deleted(state: &TimeTravelTestState, line: u32) {
    let mock = MockGitOperations::new().with_line_mapping(LineMappingVerification::deleted(line));
    state.setup_repository(true, Some(Arc::new(mock)));
    state.setup_app_with_comments(vec![TimeTravelTestState::default_comment()]);
}

#[given("time-travel mode is entered for the selected comment")]
#[when("time-travel mode is entered for the selected comment")]
fn time_travel_entered(state: &TimeTravelTestState) -> StepResult {
    let mock_state = create_mock_time_travel_state();
    state.handle_with_callback(
        &AppMsg::EnterTimeTravel,
        Some(AppMsg::TimeTravelLoaded(Box::new(mock_state))),
    )
}

#[given("the previous commit is navigated to")]
#[when("the previous commit is navigated to")]
fn previous_commit(state: &TimeTravelTestState) -> StepResult {
    let mock_state = create_mock_time_travel_state_at_index(1);
    state.handle_with_callback(
        &AppMsg::PreviousCommit,
        Some(AppMsg::CommitNavigated(Box::new(mock_state))),
    )
}

#[when("the next commit is navigated to")]
fn when_next_commit(state: &TimeTravelTestState) -> StepResult {
    let mock_state = create_mock_time_travel_state_at_index(0);
    state.handle_with_callback(
        &AppMsg::NextCommit,
        Some(AppMsg::CommitNavigated(Box::new(mock_state))),
    )
}

#[when("time-travel mode is exited")]
fn when_time_travel_exited(state: &TimeTravelTestState) -> StepResult {
    state.handle_with_callback(&AppMsg::ExitTimeTravel, None)
}

#[when("the view is rendered")]
fn when_view_rendered(state: &TimeTravelTestState) -> StepResult {
    state.render_view()
}

// Then steps

#[then("the view shows the time-travel header")]
fn then_view_shows_header(state: &TimeTravelTestState) -> StepResult {
    state.assert_view_contains("Commit:")
}

#[then("the view shows the commit message")]
fn then_view_shows_commit_message(state: &TimeTravelTestState) -> StepResult {
    state.assert_view_contains("Fix login validation")
}

#[then("the view shows the file path")]
fn then_view_shows_file_path(state: &TimeTravelTestState) -> StepResult {
    state.assert_view_contains("src/auth.rs")
}

#[then("the view shows line mapping status")]
fn then_view_shows_line_mapping(state: &TimeTravelTestState) -> StepResult {
    let view = state.view()?;
    let stripped = strip_ansi_codes(&view);
    // Check for line mapping indicator (exact match symbol)
    if stripped.contains("42") || stripped.contains("Line") {
        Ok(())
    } else {
        Err("expected line mapping status (line number or 'Line' text) not found in view")
    }
}

#[then("the view shows commit position {position}")]
fn then_view_shows_commit_position(state: &TimeTravelTestState, position: String) -> StepResult {
    state.assert_view_contains(&position)
}

#[then("the view shows commit not found error")]
fn then_view_shows_commit_not_found(state: &TimeTravelTestState) -> StepResult {
    state.assert_view_contains("not found")
}

#[then("the view shows no repository error")]
fn then_view_shows_no_repository(state: &TimeTravelTestState) -> StepResult {
    state.assert_view_contains("No local repository")
}

#[then("the review list is visible")]
fn then_review_list_visible(state: &TimeTravelTestState) -> StepResult {
    state.assert_view_contains("Filter:")
}

#[then("the view shows the line moved by {offset}")]
fn then_view_shows_line_moved(state: &TimeTravelTestState, offset: String) -> StepResult {
    state.assert_view_contains(&offset)
}

#[then("the view shows the line was deleted")]
fn then_view_shows_line_deleted(state: &TimeTravelTestState) -> StepResult {
    state.assert_view_contains("deleted")
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

#[scenario(path = "tests/features/time_travel.feature", index = 7)]
fn time_travel_line_mapping_moved(state: TimeTravelTestState) {
    let _ = state;
}

#[scenario(path = "tests/features/time_travel.feature", index = 8)]
fn time_travel_line_mapping_deleted(state: TimeTravelTestState) {
    let _ = state;
}
