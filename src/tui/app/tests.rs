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
fn empty_review_list_enforces_minimum_list_height(#[case] height: u16) {
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
fn tiny_terminal_skips_detail_pane_and_keeps_status_bar_visible() {
    let review = ReviewComment {
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(10),
        diff_hunk: Some("@@ -1 +1 @@\n+fn tiny() {}".to_owned()),
        ..minimal_review(1, "Visible comment", "alice")
    };
    let mut app = ReviewApp::with_dimensions(vec![review], 80, 5);
    app.handle_message(&AppMsg::WindowResized {
        width: 80,
        height: 5,
    });

    let output = app.view();

    assert_eq!(output.lines().count(), 5);
    assert!(
        output.contains("q:quit"),
        "status bar should remain visible in a tiny terminal"
    );
    assert!(
        !output.contains('─'),
        "detail pane should be skipped when no detail rows are available"
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

#[path = "tests_sync.rs"]
mod sync;
