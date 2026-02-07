//! Regression tests for help overlay key handling in the TUI update loop.

use bubbletea_rs::Model;
use crossterm::event::{KeyCode, KeyModifiers};

use super::*;

fn key_msg(key: KeyCode) -> bubbletea_rs::event::KeyMsg {
    bubbletea_rs::event::KeyMsg {
        key,
        modifiers: KeyModifiers::empty(),
    }
}

#[test]
fn help_overlay_closes_on_unmapped_key() {
    let mut app = ReviewApp::empty();
    app.handle_message(&AppMsg::ToggleHelp);
    assert!(app.show_help);

    let cmd = app.update(Box::new(key_msg(KeyCode::Char('x'))));

    assert!(cmd.is_none());
    assert!(!app.show_help);
}

#[test]
fn help_overlay_consumes_q_without_quitting() {
    let mut app = ReviewApp::empty();
    app.handle_message(&AppMsg::ToggleHelp);
    assert!(app.show_help);

    let cmd = app.update(Box::new(key_msg(KeyCode::Char('q'))));

    assert!(cmd.is_none());
    assert!(!app.show_help);
}

#[test]
fn q_still_quits_when_help_overlay_is_hidden() {
    let mut app = ReviewApp::empty();
    assert!(!app.show_help);

    let cmd = app.update(Box::new(key_msg(KeyCode::Char('q'))));

    assert!(cmd.is_some());
}
