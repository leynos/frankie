//! Navigation-focused tests for cursor and viewport behaviour.

use rstest::rstest;

use super::*;
use crate::github::models::test_support::create_reviews;

#[rstest]
fn cursor_down_scrolls_when_moving_beyond_bottom_of_viewport() {
    let mut app = ReviewApp::new(create_reviews(8));
    app.review_list.set_visible_height(3);

    for _ in 0..3 {
        app.handle_message(&AppMsg::CursorDown);
    }

    assert_eq!(app.cursor_position(), 3);
    assert_eq!(app.filter_state.scroll_offset, 1);
}

#[rstest]
fn cursor_up_scrolls_when_moving_above_top_of_viewport() {
    let mut app = ReviewApp::new(create_reviews(10));
    app.review_list.set_visible_height(3);

    app.handle_message(&AppMsg::End);
    assert_eq!(app.cursor_position(), 9);
    assert_eq!(app.filter_state.scroll_offset, 7);

    for _ in 0..3 {
        app.handle_message(&AppMsg::CursorUp);
    }

    assert_eq!(app.cursor_position(), 6);
    assert_eq!(app.filter_state.scroll_offset, 6);
}

#[rstest]
fn page_down_adjusts_scroll_offset_to_keep_cursor_visible() {
    let mut app = ReviewApp::new(create_reviews(10));
    app.review_list.set_visible_height(4);

    app.handle_message(&AppMsg::PageDown);

    assert_eq!(app.cursor_position(), 4);
    assert_eq!(app.filter_state.scroll_offset, 1);
}

#[rstest]
fn page_up_adjusts_scroll_offset_to_keep_cursor_visible() {
    let mut app = ReviewApp::new(create_reviews(10));
    app.review_list.set_visible_height(4);
    app.handle_message(&AppMsg::End);

    app.handle_message(&AppMsg::PageUp);

    assert_eq!(app.cursor_position(), 5);
    assert_eq!(app.filter_state.scroll_offset, 5);
}

#[rstest]
fn home_and_end_navigation_keep_cursor_visible() {
    let mut app = ReviewApp::new(create_reviews(10));
    app.review_list.set_visible_height(4);

    app.handle_message(&AppMsg::End);
    assert_eq!(app.cursor_position(), 9);
    assert_eq!(app.filter_state.scroll_offset, 6);

    app.handle_message(&AppMsg::Home);
    assert_eq!(app.cursor_position(), 0);
    assert_eq!(app.filter_state.scroll_offset, 0);
}

#[rstest]
fn short_list_does_not_scroll_when_viewport_is_taller_than_list() {
    let mut app = ReviewApp::new(create_reviews(3));
    app.review_list.set_visible_height(10);

    app.handle_message(&AppMsg::PageDown);
    app.handle_message(&AppMsg::End);

    assert_eq!(app.cursor_position(), 2);
    assert_eq!(app.filter_state.scroll_offset, 0);
}

#[rstest]
fn end_navigation_does_not_overscroll_past_last_page() {
    let mut app = ReviewApp::new(create_reviews(5));
    app.review_list.set_visible_height(3);

    app.handle_message(&AppMsg::End);

    assert_eq!(app.cursor_position(), 4);
    assert_eq!(app.filter_state.scroll_offset, 2);
}
