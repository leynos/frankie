//! Tests for the review TUI application model.

use rstest::{fixture, rstest};

use super::*;
use crate::github::models::test_support::minimal_review;

#[fixture]
fn sample_reviews() -> Vec<ReviewComment> {
    vec![
        ReviewComment {
            file_path: Some("src/main.rs".to_owned()),
            line_number: Some(10),
            diff_hunk: Some("@@ -1 +1 @@\n+fn first() {}".to_owned()),
            ..minimal_review(1, "First comment", "alice")
        },
        ReviewComment {
            file_path: Some("src/lib.rs".to_owned()),
            line_number: Some(20),
            in_reply_to_id: Some(1), // This is a reply
            diff_hunk: Some("@@ -2 +2 @@\n+fn second() {}".to_owned()),
            ..minimal_review(2, "Second comment", "bob")
        },
    ]
}

#[fixture]
fn reviews_without_hunks() -> Vec<ReviewComment> {
    vec![ReviewComment {
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(10),
        diff_hunk: None,
        ..minimal_review(1, "First comment", "alice")
    }]
}

#[rstest]
fn new_app_has_all_reviews(sample_reviews: Vec<ReviewComment>) {
    let app = ReviewApp::new(sample_reviews);
    assert_eq!(app.filtered_count(), 2);
}

#[rstest]
fn cursor_navigation_works(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    assert_eq!(app.cursor_position(), 0);

    app.handle_message(&AppMsg::CursorDown);
    assert_eq!(app.cursor_position(), 1);

    app.handle_message(&AppMsg::CursorDown);
    assert_eq!(app.cursor_position(), 1); // Cannot go past end

    app.handle_message(&AppMsg::CursorUp);
    assert_eq!(app.cursor_position(), 0);

    app.handle_message(&AppMsg::CursorUp);
    assert_eq!(app.cursor_position(), 0); // Cannot go below 0
}

#[rstest]
fn filter_changes_preserve_valid_cursor(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    app.handle_message(&AppMsg::CursorDown);
    assert_eq!(app.cursor_position(), 1);

    // Switch to ByFile filter - only 1 item matches (src/main.rs)
    app.handle_message(&AppMsg::SetFilter(ReviewFilter::ByFile(
        "src/main.rs".to_owned(),
    )));
    assert_eq!(app.filtered_count(), 1);
    assert_eq!(app.cursor_position(), 0); // Clamped to valid range
}

#[rstest]
fn view_renders_without_panic(sample_reviews: Vec<ReviewComment>) {
    let app = ReviewApp::new(sample_reviews);
    let output = app.view();

    assert!(output.contains("Frankie"));
    assert!(output.contains("Filter:"));
    assert!(output.contains("alice"));
}

#[test]
fn quit_message_returns_quit_command() {
    let mut app = ReviewApp::empty();
    let cmd = app.handle_message(&AppMsg::Quit);
    assert!(cmd.is_some());
}

#[rstest]
fn refresh_complete_updates_data(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::empty();
    assert_eq!(app.filtered_count(), 0);

    app.handle_message(&AppMsg::RefreshComplete(sample_reviews));

    assert_eq!(app.filtered_count(), 2);
    assert!(!app.loading);
}

#[test]
fn toggle_help_shows_and_hides_overlay() {
    let mut app = ReviewApp::empty();
    assert!(!app.show_help);

    app.handle_message(&AppMsg::ToggleHelp);
    assert!(app.show_help);

    let view = app.view();
    assert!(view.contains("Keyboard Shortcuts"));

    app.handle_message(&AppMsg::ToggleHelp);
    assert!(!app.show_help);
}

#[rstest]
fn show_diff_context_renders_full_screen(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    app.handle_message(&AppMsg::ShowDiffContext);

    let output = app.view();
    assert!(output.contains("Hunk"));
    assert!(output.contains("File:"));
}

#[rstest]
fn escape_clears_filter_in_list_view(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);
    app.handle_message(&AppMsg::SetFilter(ReviewFilter::Unresolved));
    assert_ne!(app.active_filter(), &ReviewFilter::All);

    app.handle_message(&AppMsg::EscapePressed);

    assert_eq!(app.active_filter(), &ReviewFilter::All);
}

#[rstest]
fn escape_exits_diff_context(reviews_without_hunks: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(reviews_without_hunks);

    app.handle_message(&AppMsg::ShowDiffContext);
    app.handle_message(&AppMsg::EscapePressed);

    let output = app.view();
    assert!(output.contains("Filter:"));
}

// Background sync tests

#[rstest]
fn sync_complete_preserves_selection_by_id(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews.clone());

    // Select second item (id=2)
    app.handle_message(&AppMsg::CursorDown);
    assert_eq!(app.cursor_position(), 1);
    assert_eq!(app.current_selected_id(), Some(2));

    // Simulate sync with same data (order may differ internally after merge)
    let cmd = app.handle_message(&AppMsg::SyncComplete {
        reviews: sample_reviews,
        latency_ms: 100,
    });

    // Selection should still be on comment with id=2
    assert_eq!(app.current_selected_id(), Some(2));
    // Command should be returned (re-armed timer)
    assert!(cmd.is_some());
}

#[rstest]
fn sync_complete_clamps_cursor_when_selected_deleted(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews.clone());

    // Select second item (id=2)
    app.handle_message(&AppMsg::CursorDown);
    assert_eq!(app.cursor_position(), 1);
    assert_eq!(app.current_selected_id(), Some(2));

    // Sync with first comment only (second deleted)
    let remaining: Vec<ReviewComment> = sample_reviews.into_iter().take(1).collect();

    app.handle_message(&AppMsg::SyncComplete {
        reviews: remaining,
        latency_ms: 50,
    });

    // Cursor should be clamped to 0 (only item)
    assert_eq!(app.cursor_position(), 0);
    assert_eq!(app.filtered_count(), 1);
}

#[rstest]
fn sync_complete_adds_new_comments(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews.clone());
    assert_eq!(app.filtered_count(), 2);

    // Add a third comment
    let mut with_new = sample_reviews;
    with_new.push(minimal_review(3, "Third comment", "charlie"));

    app.handle_message(&AppMsg::SyncComplete {
        reviews: with_new,
        latency_ms: 75,
    });

    assert_eq!(app.filtered_count(), 3);
}

/// Tests that navigation commands correctly update `selected_comment_id`.
#[rstest]
#[case::cursor_down(AppMsg::CursorDown, 0, Some(2))]
#[case::cursor_up_from_end(AppMsg::CursorUp, 1, Some(1))]
#[case::end_key(AppMsg::End, 0, Some(2))]
#[case::home_key(AppMsg::Home, 1, Some(1))]
fn navigation_updates_selected_id(
    sample_reviews: Vec<ReviewComment>,
    #[case] msg: AppMsg,
    #[case] initial_cursor: usize,
    #[case] expected_id: Option<u64>,
) {
    let mut app = ReviewApp::new(sample_reviews);

    // Move cursor to initial position
    for _ in 0..initial_cursor {
        app.handle_message(&AppMsg::CursorDown);
    }

    app.handle_message(&msg);
    assert_eq!(app.current_selected_id(), expected_id);
}

#[rstest]
fn sync_complete_clamps_cursor_when_all_comments_removed(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    // Move cursor to ensure we start from a non-zero position in a non-empty list
    app.handle_message(&AppMsg::CursorDown);
    assert!(app.filtered_count() > 0);
    assert!(app.current_selected_id().is_some());

    // Sync with an empty list of reviews (all comments removed)
    app.handle_message(&AppMsg::SyncComplete {
        reviews: Vec::new(),
        latency_ms: 50,
    });

    // Cursor and selection should be reset/clamped for the empty list
    assert_eq!(app.filtered_count(), 0);
    assert_eq!(app.cursor_position(), 0);
    assert!(app.current_selected_id().is_none());
}

// Tests for handle_sync_tick

#[rstest]
fn sync_tick_sets_loading_state(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);
    assert!(!app.loading);

    let cmd = app.handle_message(&AppMsg::SyncTick);

    // Should set loading to true
    assert!(app.loading);
    // Should return a command (the fetch command)
    assert!(cmd.is_some());
}

#[test]
fn sync_tick_skips_when_already_loading() {
    let mut app = ReviewApp::empty();

    // Manually set loading state
    app.loading = true;

    let cmd = app.handle_message(&AppMsg::SyncTick);

    // Should still be loading
    assert!(app.loading);
    // Should still return a command (the timer re-arm)
    assert!(cmd.is_some());
}

#[test]
fn sync_tick_clears_error_state() {
    let mut app = ReviewApp::empty();
    app.error = Some("Previous error".to_owned());
    assert!(!app.loading);

    app.handle_message(&AppMsg::SyncTick);

    // Error should be cleared
    assert!(app.error.is_none());
    // Loading should be set
    assert!(app.loading);
}

// Tests for find_filtered_index_by_id

#[rstest]
fn find_filtered_index_by_id_finds_existing_comment(sample_reviews: Vec<ReviewComment>) {
    let app = ReviewApp::new(sample_reviews);

    // First comment (id=1) should be at index 0
    assert_eq!(app.find_filtered_index_by_id(1), Some(0));
    // Second comment (id=2) should be at index 1
    assert_eq!(app.find_filtered_index_by_id(2), Some(1));
}

#[rstest]
fn find_filtered_index_by_id_returns_none_for_missing_id(sample_reviews: Vec<ReviewComment>) {
    let app = ReviewApp::new(sample_reviews);

    // Non-existent ID should return None
    assert_eq!(app.find_filtered_index_by_id(999), None);
}

#[test]
fn find_filtered_index_by_id_returns_none_for_empty_app() {
    let app = ReviewApp::empty();

    assert_eq!(app.find_filtered_index_by_id(1), None);
}

#[rstest]
fn find_filtered_index_by_id_respects_filter(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    // Apply a filter that only matches the first comment (src/main.rs)
    app.handle_message(&AppMsg::SetFilter(ReviewFilter::ByFile(
        "src/main.rs".to_owned(),
    )));

    // Only comment with id=1 should be visible
    assert_eq!(app.filtered_count(), 1);

    // Comment 1 should be at filtered index 0
    assert_eq!(app.find_filtered_index_by_id(1), Some(0));

    // Comment 2 is filtered out, so should return None
    assert_eq!(app.find_filtered_index_by_id(2), None);
}

// Tests for arm_sync_timer

#[tokio::test]
async fn arm_sync_timer_schedules_sync_tick_after_interval() {
    use std::time::Duration;

    // Pause time so we can control advancement
    tokio::time::pause();

    let cmd = ReviewApp::arm_sync_timer();

    // Advance time past the sync interval (30 seconds)
    tokio::time::advance(Duration::from_secs(31)).await;

    // Poll the future to completion
    let result = cmd.await;

    // The future should resolve to Some(Box<AppMsg::SyncTick>)
    assert!(result.is_some(), "arm_sync_timer should return a message");

    let msg = result.expect("result should be Some");
    let app_msg = msg.downcast_ref::<AppMsg>();
    assert!(
        matches!(app_msg, Some(AppMsg::SyncTick)),
        "arm_sync_timer should schedule a SyncTick message"
    );
}

// Tests for ReviewApp::init (via Model trait)

#[test]
fn review_app_init_returns_sync_timer_command() {
    use bubbletea_rs::Model;

    // Try to set initial reviews. Due to OnceLock, this may fail if another
    // test has already set the reviews. The assertions below are conditional
    // on whether we were the first to set them.
    let was_set = crate::tui::set_initial_reviews(vec![minimal_review(1, "Test", "alice")]);

    let (app, cmd) = ReviewApp::init();

    // Only verify specific review data if we were the first to set it
    if was_set {
        assert_eq!(app.filtered_count(), 1);
        assert_eq!(app.current_selected_id(), Some(1));
    }

    // Should return a command (the sync timer) regardless of review content
    assert!(cmd.is_some());
}

#[test]
fn init_returns_expected_commands_and_state_regardless_of_prior_reviews() {
    use bubbletea_rs::Model;

    // OnceLock may retain reviews from prior tests - this test verifies
    // invariants that hold regardless of initial review data.
    let (app, cmd) = ReviewApp::init();

    // Should return a sync timer command regardless of review count
    assert!(cmd.is_some());
    // App should not be in loading state initially
    assert!(!app.loading);
    // No error initially
    assert!(app.error.is_none());
    // Help should not be shown initially
    assert!(!app.show_help);
}
