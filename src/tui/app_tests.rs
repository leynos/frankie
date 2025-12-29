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
