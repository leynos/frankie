//! Tests for the review TUI application model.

use super::*;

fn make_reviews() -> Vec<ReviewComment> {
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

#[test]
fn new_app_has_all_reviews() {
    let reviews = make_reviews();
    let app = ReviewApp::new(reviews.clone());
    assert_eq!(app.filtered_count(), 2);
}

#[test]
fn cursor_navigation_works() {
    let reviews = make_reviews();
    let mut app = ReviewApp::new(reviews);

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

#[test]
fn filter_changes_preserve_valid_cursor() {
    let reviews = make_reviews();
    let mut app = ReviewApp::new(reviews);

    app.handle_message(&AppMsg::CursorDown);
    assert_eq!(app.cursor_position(), 1);

    // Switch to unresolved filter - only 1 item matches
    app.handle_message(&AppMsg::SetFilter(ReviewFilter::Unresolved));
    assert_eq!(app.filtered_count(), 1);
    assert_eq!(app.cursor_position(), 0); // Clamped to valid range
}

#[test]
fn view_renders_without_panic() {
    let reviews = make_reviews();
    let app = ReviewApp::new(reviews);
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

#[test]
fn refresh_complete_updates_data() {
    let mut app = ReviewApp::empty();
    assert_eq!(app.filtered_count(), 0);

    let new_reviews = make_reviews();
    app.handle_message(&AppMsg::RefreshComplete(new_reviews));

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
