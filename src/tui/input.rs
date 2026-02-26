//! Input handling for the TUI application.
//!
//! This module provides key-to-message mapping for translating terminal key
//! events into application messages.

use super::messages::AppMsg;
use crate::ai::CommentRewriteMode;

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
    /// Session resume prompt (y/n/Esc).
    ResumePrompt,
    /// Inline reply drafting mode.
    ReplyDraft,
}

/// Maps a key event to an application message.
///
/// Returns `None` for unrecognised key events, allowing them to be ignored.
/// This is a convenience wrapper that assumes the `ReviewList` context.
/// For context-aware key mapping, use `map_key_to_message_with_context`.
#[must_use]
pub const fn map_key_to_message(key: &bubbletea_rs::event::KeyMsg) -> Option<AppMsg> {
    map_key_to_message_with_context(key, InputContext::ReviewList)
}

const fn shared_keys(key: &bubbletea_rs::event::KeyMsg) -> Option<AppMsg> {
    use crossterm::event::KeyCode;

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

/// Maps a key event to an application message with view context.
///
/// Different view modes may interpret the same key differently. For example,
/// `h` and `l` are navigation keys in time-travel mode but have no function
/// in the review list.
#[must_use]
#[doc(hidden)]
pub const fn map_key_to_message_with_context(
    key: &bubbletea_rs::event::KeyMsg,
    context: InputContext,
) -> Option<AppMsg> {
    use crossterm::event::KeyCode;

    match context {
        InputContext::TimeTravel => match key.key {
            KeyCode::Char('h') => Some(AppMsg::PreviousCommit),
            KeyCode::Char('l') => Some(AppMsg::NextCommit),
            KeyCode::Esc => Some(AppMsg::ExitTimeTravel),
            KeyCode::Char('q') => Some(AppMsg::Quit),
            _ => shared_keys(key),
        },
        InputContext::DiffContext => match key.key {
            KeyCode::Char('[') => Some(AppMsg::PreviousHunk),
            KeyCode::Char(']') => Some(AppMsg::NextHunk),
            KeyCode::Esc => Some(AppMsg::HideDiffContext),
            KeyCode::Char('q') => Some(AppMsg::Quit),
            _ => shared_keys(key),
        },
        InputContext::ResumePrompt => match key.key {
            KeyCode::Char('y') => Some(AppMsg::ResumeAccepted),
            KeyCode::Char('n') | KeyCode::Esc => Some(AppMsg::ResumeDeclined),
            KeyCode::Char('q') => Some(AppMsg::Quit),
            _ => None,
        },
        InputContext::ReviewList => match key.key {
            KeyCode::Char('x') => Some(AppMsg::StartCodexExecution),
            KeyCode::Char('a') => Some(AppMsg::StartReplyDraft),
            _ => shared_keys(key),
        },
        InputContext::ReplyDraft => match key.key {
            KeyCode::Enter => Some(AppMsg::ReplyDraftRequestSend),
            KeyCode::Backspace => Some(AppMsg::ReplyDraftBackspace),
            KeyCode::Esc => Some(AppMsg::ReplyDraftCancel),
            KeyCode::Char('E') => Some(AppMsg::ReplyDraftRequestAiRewrite {
                mode: CommentRewriteMode::Expand,
            }),
            KeyCode::Char('W') => Some(AppMsg::ReplyDraftRequestAiRewrite {
                mode: CommentRewriteMode::Reword,
            }),
            KeyCode::Char('Y') => Some(AppMsg::ReplyDraftAiApply),
            KeyCode::Char('N') => Some(AppMsg::ReplyDraftAiDiscard),
            KeyCode::Char(character @ '1'..='9') => {
                let template_index = character as usize - '1' as usize;
                Some(AppMsg::ReplyDraftInsertTemplate { template_index })
            }
            KeyCode::Char(character) => Some(AppMsg::ReplyDraftInsertChar(character)),
            _ => None,
        },
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
    #[case::review_list_x_start_codex(
        KeyCode::Char('x'),
        Some(InputContext::ReviewList),
        Some(AppMsg::StartCodexExecution)
    )]
    #[case::review_list_a_start_reply_draft(
        KeyCode::Char('a'),
        Some(InputContext::ReviewList),
        Some(AppMsg::StartReplyDraft)
    )]
    #[case::time_travel_x_unmapped(KeyCode::Char('x'), Some(InputContext::TimeTravel), None)]
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
    #[case::resume_prompt_y_accepted(
        KeyCode::Char('y'),
        Some(InputContext::ResumePrompt),
        Some(AppMsg::ResumeAccepted)
    )]
    #[case::resume_prompt_n_declined(
        KeyCode::Char('n'),
        Some(InputContext::ResumePrompt),
        Some(AppMsg::ResumeDeclined)
    )]
    #[case::resume_prompt_esc_declined(
        KeyCode::Esc,
        Some(InputContext::ResumePrompt),
        Some(AppMsg::ResumeDeclined)
    )]
    #[case::resume_prompt_j_unmapped(KeyCode::Char('j'), Some(InputContext::ResumePrompt), None)]
    #[case::reply_draft_insert_template(
        KeyCode::Char('2'),
        Some(InputContext::ReplyDraft),
        Some(AppMsg::ReplyDraftInsertTemplate { template_index: 1 })
    )]
    #[case::reply_draft_ai_expand(
        KeyCode::Char('E'),
        Some(InputContext::ReplyDraft),
        Some(AppMsg::ReplyDraftRequestAiRewrite { mode: CommentRewriteMode::Expand })
    )]
    #[case::reply_draft_ai_reword(
        KeyCode::Char('W'),
        Some(InputContext::ReplyDraft),
        Some(AppMsg::ReplyDraftRequestAiRewrite { mode: CommentRewriteMode::Reword })
    )]
    #[case::reply_draft_ai_apply(
        KeyCode::Char('Y'),
        Some(InputContext::ReplyDraft),
        Some(AppMsg::ReplyDraftAiApply)
    )]
    #[case::reply_draft_ai_discard(
        KeyCode::Char('N'),
        Some(InputContext::ReplyDraft),
        Some(AppMsg::ReplyDraftAiDiscard)
    )]
    #[case::reply_draft_insert_char(
        KeyCode::Char('q'),
        Some(InputContext::ReplyDraft),
        Some(AppMsg::ReplyDraftInsertChar('q'))
    )]
    #[case::reply_draft_backspace(
        KeyCode::Backspace,
        Some(InputContext::ReplyDraft),
        Some(AppMsg::ReplyDraftBackspace)
    )]
    #[case::reply_draft_enter_send(
        KeyCode::Enter,
        Some(InputContext::ReplyDraft),
        Some(AppMsg::ReplyDraftRequestSend)
    )]
    #[case::reply_draft_esc_cancel(
        KeyCode::Esc,
        Some(InputContext::ReplyDraft),
        Some(AppMsg::ReplyDraftCancel)
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
                assert_eq!(std::mem::discriminant(&r), std::mem::discriminant(&e));
            }
            (None, None) => {}
            (r, e) => {
                panic!("Some/None mismatch: result={r:?}, expected={e:?}");
            }
        }
    }

    #[test]
    fn reply_draft_template_index_maps_digit_to_zero_based_slot() {
        let result =
            map_key_to_message_with_context(&key_msg(KeyCode::Char('9')), InputContext::ReplyDraft);

        assert!(matches!(
            result,
            Some(AppMsg::ReplyDraftInsertTemplate { template_index: 8 })
        ));
    }
}
