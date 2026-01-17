//! Input handling for the TUI application.
//!
//! This module provides key-to-message mapping for translating terminal key
//! events into application messages.

use super::messages::AppMsg;

/// Maps a key event to an application message.
///
/// Returns `None` for unrecognised key events, allowing them to be ignored.
#[must_use]
#[expect(
    clippy::missing_const_for_fn,
    reason = "KeyCode match patterns prevent const evaluation"
)]
pub fn map_key_to_message(key: &bubbletea_rs::event::KeyMsg) -> Option<AppMsg> {
    use crossterm::event::KeyCode;

    match key.key {
        KeyCode::Char('q') => Some(AppMsg::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(AppMsg::CursorDown),
        KeyCode::Char('k') | KeyCode::Up => Some(AppMsg::CursorUp),
        KeyCode::PageDown => Some(AppMsg::PageDown),
        KeyCode::PageUp => Some(AppMsg::PageUp),
        KeyCode::Home | KeyCode::Char('g') => Some(AppMsg::Home),
        KeyCode::End | KeyCode::Char('G') => Some(AppMsg::End),
        KeyCode::Char('f') => Some(AppMsg::CycleFilter),
        KeyCode::Esc => Some(AppMsg::EscapePressed),
        KeyCode::Char('r') => Some(AppMsg::RefreshRequested),
        KeyCode::Char('?') => Some(AppMsg::ToggleHelp),
        KeyCode::Char('c') => Some(AppMsg::ShowDiffContext),
        KeyCode::Char('[') => Some(AppMsg::PreviousHunk),
        KeyCode::Char(']') => Some(AppMsg::NextHunk),
        _ => None,
    }
}
