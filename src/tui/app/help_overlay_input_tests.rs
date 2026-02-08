//! Regression tests for help overlay key handling in the TUI update loop.

use bubbletea_rs::Model;
use crossterm::event::{KeyCode, KeyModifiers};
use rstest::{fixture, rstest};

use super::*;

fn key_msg(key: KeyCode) -> bubbletea_rs::event::KeyMsg {
    bubbletea_rs::event::KeyMsg {
        key,
        modifiers: KeyModifiers::empty(),
    }
}

#[fixture]
fn app() -> ReviewApp {
    ReviewApp::empty()
}

#[rstest]
fn help_overlay_closes_on_unmapped_key(mut app: ReviewApp) {
    app.handle_message(&AppMsg::ToggleHelp);
    assert!(app.show_help);

    let cmd = app.update(Box::new(key_msg(KeyCode::Char('x'))));

    assert!(cmd.is_none());
    assert!(!app.show_help);
}

#[rstest]
fn help_overlay_consumes_q_without_quitting(mut app: ReviewApp) {
    app.handle_message(&AppMsg::ToggleHelp);
    assert!(app.show_help);

    let cmd = app.update(Box::new(key_msg(KeyCode::Char('q'))));

    assert!(cmd.is_none());
    assert!(!app.show_help);
}

#[rstest]
fn help_overlay_unmapped_then_q_quits(mut app: ReviewApp) {
    app.handle_message(&AppMsg::ToggleHelp);
    assert!(app.show_help);

    let close_help_cmd = app.update(Box::new(key_msg(KeyCode::Char('x'))));
    assert!(close_help_cmd.is_none());
    assert!(!app.show_help);

    let quit_cmd = app.update(Box::new(key_msg(KeyCode::Char('q'))));
    assert!(quit_cmd.is_some());
}

#[rstest]
fn q_still_quits_when_help_overlay_is_hidden(mut app: ReviewApp) {
    assert!(!app.show_help);

    let cmd = app.update(Box::new(key_msg(KeyCode::Char('q'))));

    assert!(cmd.is_some());
}
