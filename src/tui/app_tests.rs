//! Tests for the review TUI application model.

use rstest::{fixture, rstest};

use super::*;

#[fixture]
fn sample_reviews() -> Vec<ReviewComment> {
    vec![
        ReviewComment {
            id: 1,
            body: Some("First comment".to_owned()),
            author: Some("alice".to_owned()),
            file_path: Some("src/main.rs".to_owned()),
            line_number: Some(10),
            original_line_number: None,
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
            updated_at: None,
        },
        ReviewComment {
            id: 2,
            body: Some("Second comment".to_owned()),
            author: Some("bob".to_owned()),
            file_path: Some("src/lib.rs".to_owned()),
            line_number: Some(20),
            original_line_number: None,
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: Some(1), // This is a reply
            created_at: None,
            updated_at: None,
        },
    ]
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
    with_new.push(ReviewComment {
        id: 3,
        body: Some("Third comment".to_owned()),
        author: Some("charlie".to_owned()),
        ..Default::default()
    });

    app.handle_message(&AppMsg::SyncComplete {
        reviews: with_new,
        latency_ms: 75,
    });

    assert_eq!(app.filtered_count(), 3);
}

#[rstest]
fn navigation_updates_selected_id(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    assert_eq!(app.current_selected_id(), Some(1));

    app.handle_message(&AppMsg::CursorDown);
    assert_eq!(app.current_selected_id(), Some(2));

    app.handle_message(&AppMsg::CursorUp);
    assert_eq!(app.current_selected_id(), Some(1));

    app.handle_message(&AppMsg::End);
    assert_eq!(app.current_selected_id(), Some(2));

    app.handle_message(&AppMsg::Home);
    assert_eq!(app.current_selected_id(), Some(1));
}
