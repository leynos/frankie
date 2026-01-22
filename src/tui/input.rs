//! Input handling for the TUI application.
//!
//! This module provides key-to-message mapping for translating terminal key
//! events into application messages.

use super::messages::AppMsg;

/// View mode for context-aware key mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputContext {
    /// Default review list view.
    #[default]
    ReviewList,
    /// Full-screen diff context view.
    DiffContext,
    /// Time-travel navigation view.
    TimeTravel,
}

/// Maps a key event to an application message.
///
/// Returns `None` for unrecognised key events, allowing them to be ignored.
/// This is a convenience wrapper that assumes the `ReviewList` context.
/// For context-aware key mapping, use `map_key_to_message_with_context`.
#[must_use]
pub fn map_key_to_message(key: &bubbletea_rs::event::KeyMsg) -> Option<AppMsg> {
    map_key_to_message_with_context(key, InputContext::ReviewList)
}

/// Maps a key event to an application message with view context.
///
/// Different view modes may interpret the same key differently. For example,
/// `h` and `l` are navigation keys in time-travel mode but have no function
/// in the review list.
#[must_use]
#[doc(hidden)]
#[expect(
    clippy::missing_const_for_fn,
    reason = "KeyCode match patterns prevent const evaluation"
)]
pub fn map_key_to_message_with_context(
    key: &bubbletea_rs::event::KeyMsg,
    context: InputContext,
) -> Option<AppMsg> {
    use crossterm::event::KeyCode;

    // Context-specific mappings first
    match context {
        InputContext::TimeTravel => match key.key {
            KeyCode::Char('h') => return Some(AppMsg::PreviousCommit),
            KeyCode::Char('l') => return Some(AppMsg::NextCommit),
            KeyCode::Esc => return Some(AppMsg::ExitTimeTravel),
            KeyCode::Char('q') => return Some(AppMsg::Quit),
            _ => {}
        },
        InputContext::DiffContext => match key.key {
            KeyCode::Char('[') => return Some(AppMsg::PreviousHunk),
            KeyCode::Char(']') => return Some(AppMsg::NextHunk),
            KeyCode::Esc => return Some(AppMsg::HideDiffContext),
            KeyCode::Char('q') => return Some(AppMsg::Quit),
            _ => {}
        },
        InputContext::ReviewList => {}
    }

    // Default/shared mappings
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
        KeyCode::Char('t') => Some(AppMsg::EnterTimeTravel),
        KeyCode::Char('[') => Some(AppMsg::PreviousHunk),
        KeyCode::Char(']') => Some(AppMsg::NextHunk),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bubbletea_rs::event::KeyMsg;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn key_msg(key: KeyCode) -> KeyMsg {
        KeyMsg {
            key,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[test]
    fn time_travel_h_maps_to_previous_commit() {
        let msg =
            map_key_to_message_with_context(&key_msg(KeyCode::Char('h')), InputContext::TimeTravel);
        assert!(matches!(msg, Some(AppMsg::PreviousCommit)));
    }

    #[test]
    fn time_travel_l_maps_to_next_commit() {
        let msg =
            map_key_to_message_with_context(&key_msg(KeyCode::Char('l')), InputContext::TimeTravel);
        assert!(matches!(msg, Some(AppMsg::NextCommit)));
    }

    #[test]
    fn time_travel_esc_exits() {
        let msg = map_key_to_message_with_context(&key_msg(KeyCode::Esc), InputContext::TimeTravel);
        assert!(matches!(msg, Some(AppMsg::ExitTimeTravel)));
    }

    #[test]
    fn review_list_t_enters_time_travel() {
        let msg =
            map_key_to_message_with_context(&key_msg(KeyCode::Char('t')), InputContext::ReviewList);
        assert!(matches!(msg, Some(AppMsg::EnterTimeTravel)));
    }

    #[test]
    fn diff_context_esc_hides() {
        let msg =
            map_key_to_message_with_context(&key_msg(KeyCode::Esc), InputContext::DiffContext);
        assert!(matches!(msg, Some(AppMsg::HideDiffContext)));
    }

    #[test]
    fn default_context_is_review_list() {
        let msg1 = map_key_to_message(&key_msg(KeyCode::Char('j')));
        let msg2 =
            map_key_to_message_with_context(&key_msg(KeyCode::Char('j')), InputContext::ReviewList);
        assert!(matches!(msg1, Some(AppMsg::CursorDown)));
        assert!(matches!(msg2, Some(AppMsg::CursorDown)));
    }
}
