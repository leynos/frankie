//! Tests for the review TUI application model.

use bubbletea_rs::Model;
use rstest::{fixture, rstest};
use unicode_width::UnicodeWidthStr;

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

fn many_reviews(count: u64) -> Vec<ReviewComment> {
    (1..=count)
        .map(|id| minimal_review(id, &format!("Comment {id}"), "alice"))
        .collect()
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

#[test]
fn with_dimensions_applies_explicit_terminal_size() {
    let app = ReviewApp::with_dimensions(many_reviews(3), 120, 40);
    let body_height = 40usize.saturating_sub(4);
    let detail_height = body_height.saturating_sub(app.review_list.visible_height());

    assert_eq!(app.width, 120);
    assert_eq!(app.height, 40);
    assert_eq!(app.review_list.visible_height(), 3);
    assert_eq!(detail_height, body_height.saturating_sub(3));
}

#[test]
fn resize_updates_visible_list_height_with_shared_layout_rules() {
    let mut app = ReviewApp::with_dimensions(many_reviews(20), 120, 8);

    app.handle_message(&AppMsg::WindowResized {
        width: 120,
        height: 16,
    });

    assert_eq!(app.review_list.visible_height(), 10);
}

#[rstest]
#[case::zero_height(0)]
#[case::below_chrome(10)]
#[case::at_chrome_plus_detail(12)]
#[case::one_row_above_threshold(13)]
#[case::normal_terminal(24)]
#[case::large_terminal(80)]
fn short_terminal_clamps_list_height_to_minimum(#[case] height: u16) {
    let app = ReviewApp::with_dimensions(Vec::new(), 80, height);
    assert!(
        app.review_list.visible_height() >= 1,
        "visible_height must never be zero (was {} for height {height})",
        app.review_list.visible_height()
    );
    assert_eq!(app.review_list.visible_height(), 1);
}

#[test]
fn cursor_navigation_adjusts_scroll_to_keep_selection_visible() {
    let mut app = ReviewApp::new(many_reviews(10));
    app.handle_message(&AppMsg::WindowResized {
        width: 120,
        height: 16,
    });

    for _ in 0..5 {
        app.handle_message(&AppMsg::CursorDown);
    }

    assert_eq!(app.cursor_position(), 5);
    assert_eq!(app.filter_state.scroll_offset, 0);

    for _ in 0..4 {
        app.handle_message(&AppMsg::CursorUp);
    }

    assert_eq!(app.cursor_position(), 1);
    assert_eq!(app.filter_state.scroll_offset, 0);
}

#[rstest]
fn filter_changes_adjust_scroll_offset_after_cursor_clamp(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);
    app.handle_message(&AppMsg::WindowResized {
        width: 120,
        height: 13,
    });
    app.handle_message(&AppMsg::CursorDown);
    assert_eq!(app.filter_state.scroll_offset, 0);

    app.handle_message(&AppMsg::SetFilter(ReviewFilter::ByFile(
        "src/main.rs".to_owned(),
    )));

    assert_eq!(app.filtered_count(), 1);
    assert_eq!(app.cursor_position(), 0);
    assert_eq!(app.filter_state.scroll_offset, 0);
}

#[test]
fn short_terminal_still_renders_list_items() {
    let reviews = vec![minimal_review(1, "Visible comment", "alice")];
    let mut app = ReviewApp::with_dimensions(reviews, 80, 8);
    app.handle_message(&AppMsg::WindowResized {
        width: 80,
        height: 8,
    });
    let output = app.view();
    assert!(
        output.contains("alice"),
        "list should render at least one item in a short terminal"
    );
}

#[test]
fn view_clamps_rows_to_safe_display_width_with_emoji_content() {
    let review = ReviewComment {
        file_path: Some("src/tui/app/codex_handlers.unknown_ext_xyz".to_owned()),
        line_number: Some(123),
        body: Some("_🧹 Nitpick_ | _🔵 Trivial_".to_owned()),
        diff_hunk: None,
        ..minimal_review(1, "placeholder", "coderabbitai[bot]")
    };

    let app = ReviewApp::with_dimensions(vec![review], 80, 24);
    let output = app.view();
    let max_display_width = 79usize;

    for line in output.lines() {
        assert!(
            UnicodeWidthStr::width(line) <= max_display_width,
            "line exceeds safe display width: '{line}'"
        );
    }
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
