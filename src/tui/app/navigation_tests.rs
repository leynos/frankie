//! Navigation-focused tests for cursor and viewport behaviour.

use rstest::{fixture, rstest};

use super::*;
use crate::github::models::test_support::create_reviews;

#[fixture]
fn setup_app(#[default(10)] review_count: usize, #[default(3)] visible_height: usize) -> ReviewApp {
    let mut app = ReviewApp::new(create_reviews(review_count));
    app.review_list.set_visible_height(visible_height);
    app
}

fn assert_state(app: &ReviewApp, expected_cursor: usize, expected_scroll: usize) {
    assert_eq!(app.cursor_position(), expected_cursor);
    assert_eq!(app.filter_state.scroll_offset, expected_scroll);
}

#[rstest]
fn cursor_down_scrolls_when_moving_beyond_bottom_of_viewport(
    #[with(8, 3)] mut setup_app: ReviewApp,
) {
    for _ in 0..3 {
        setup_app.handle_message(&AppMsg::CursorDown);
    }

    assert_state(&setup_app, 3, 1);
}

#[rstest]
fn cursor_up_scrolls_when_moving_above_top_of_viewport(mut setup_app: ReviewApp) {
    setup_app.handle_message(&AppMsg::End);
    assert_state(&setup_app, 9, 7);

    for _ in 0..3 {
        setup_app.handle_message(&AppMsg::CursorUp);
    }

    assert_state(&setup_app, 6, 6);
}

#[rstest]
fn page_down_adjusts_scroll_offset_to_keep_cursor_visible(#[with(10, 4)] mut setup_app: ReviewApp) {
    setup_app.handle_message(&AppMsg::PageDown);

    assert_state(&setup_app, 4, 1);
}

#[rstest]
fn page_up_adjusts_scroll_offset_to_keep_cursor_visible(#[with(10, 4)] mut setup_app: ReviewApp) {
    setup_app.handle_message(&AppMsg::End);

    setup_app.handle_message(&AppMsg::PageUp);

    assert_state(&setup_app, 5, 5);
}

#[rstest]
fn home_and_end_navigation_keep_cursor_visible(#[with(10, 4)] mut setup_app: ReviewApp) {
    setup_app.handle_message(&AppMsg::End);
    assert_state(&setup_app, 9, 6);

    setup_app.handle_message(&AppMsg::Home);
    assert_state(&setup_app, 0, 0);
}

#[rstest]
fn short_list_does_not_scroll_when_viewport_is_taller_than_list(
    #[with(3, 10)] mut setup_app: ReviewApp,
) {
    setup_app.handle_message(&AppMsg::PageDown);
    setup_app.handle_message(&AppMsg::End);

    assert_state(&setup_app, 2, 0);
}

#[rstest]
fn end_navigation_does_not_overscroll_past_last_page(#[with(5, 3)] mut setup_app: ReviewApp) {
    setup_app.handle_message(&AppMsg::End);

    assert_state(&setup_app, 4, 2);
}

#[rstest]
fn single_row_viewport_keeps_scroll_aligned_with_cursor(#[with(10, 1)] mut setup_app: ReviewApp) {
    for i in 0..9 {
        setup_app.handle_message(&AppMsg::CursorDown);
        assert_state(&setup_app, i + 1, i + 1);
    }

    setup_app.handle_message(&AppMsg::End);
    assert_state(&setup_app, 9, 9);
}
