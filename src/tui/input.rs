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
    use rstest::rstest;

    fn key_msg(key: KeyCode) -> KeyMsg {
        KeyMsg {
            key,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[rstest]
    #[case::time_travel_h_previous(
        KeyCode::Char('h'),
        Some(InputContext::TimeTravel),
        Some(AppMsg::PreviousCommit)
    )]
    #[case::time_travel_l_next(
        KeyCode::Char('l'),
        Some(InputContext::TimeTravel),
        Some(AppMsg::NextCommit)
    )]
    #[case::time_travel_esc_exit(
        KeyCode::Esc,
        Some(InputContext::TimeTravel),
        Some(AppMsg::ExitTimeTravel)
    )]
    #[case::review_list_t_enter(
        KeyCode::Char('t'),
        Some(InputContext::ReviewList),
        Some(AppMsg::EnterTimeTravel)
    )]
    #[case::diff_context_esc_hide(
        KeyCode::Esc,
        Some(InputContext::DiffContext),
        Some(AppMsg::HideDiffContext)
    )]
    #[case::review_list_j_down(
        KeyCode::Char('j'),
        Some(InputContext::ReviewList),
        Some(AppMsg::CursorDown)
    )]
    #[case::default_context_j_down(KeyCode::Char('j'), None, Some(AppMsg::CursorDown))]
    fn key_mapping(
        #[case] key: KeyCode,
        #[case] ctx: Option<InputContext>,
        #[case] expected: Option<AppMsg>,
    ) {
        let result = ctx.map_or_else(
            || map_key_to_message(&key_msg(key)),
            |context| map_key_to_message_with_context(&key_msg(key), context),
        );

        // Compare enum variants using discriminant
        match (result, expected) {
            (Some(r), Some(e)) => {
                assert_eq!(
                    std::mem::discriminant(&r),
                    std::mem::discriminant(&e),
                    "Expected {e:?}, got {r:?}"
                );
            }
            (None, None) => {}
            (r, e) => panic!("Expected {e:?}, got {r:?}"),
        }
    }
}
