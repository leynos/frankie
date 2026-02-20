//! Reply drafting handlers for the review TUI.
//!
//! This module implements keyboard-driven template insertion and inline reply
//! editing while enforcing configured length limits.

use bubbletea_rs::Cmd;

use crate::tui::messages::AppMsg;
use crate::tui::state::{ReplyDraftState, ReplyTemplateError, render_reply_template};

use super::ReviewApp;

impl ReviewApp {
    /// Handles reply-drafting messages.
    pub(super) fn handle_reply_draft_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::StartReplyDraft => {
                self.start_reply_draft();
            }
            AppMsg::ReplyDraftInsertTemplate { template_index } => {
                self.insert_reply_template(*template_index);
            }
            AppMsg::ReplyDraftInsertChar(character) => {
                self.insert_reply_character(*character);
            }
            AppMsg::ReplyDraftBackspace => {
                self.backspace_reply_draft();
            }
            AppMsg::ReplyDraftRequestSend => {
                self.request_reply_send();
            }
            AppMsg::ReplyDraftCancel => {
                self.cancel_reply_draft();
            }
            _ => {}
        }

        None
    }

    fn start_reply_draft(&mut self) {
        let Some(comment) = self.selected_comment() else {
            self.error = Some("Reply drafting requires a selected comment".to_owned());
            return;
        };

        self.reply_draft = Some(ReplyDraftState::new(
            comment.id,
            self.reply_draft_config.max_length,
        ));
        self.error = None;
    }

    fn insert_reply_template(&mut self, template_index: usize) {
        let Some(comment) = self.selected_comment().cloned() else {
            self.error = Some("Reply drafting requires a selected comment".to_owned());
            return;
        };

        let Some(template_source) = self.reply_draft_config.templates.get(template_index) else {
            let available = self.reply_draft_config.templates.len();
            self.error = Some(format!(
                "Reply template {} is not configured (available templates: {available})",
                template_index + 1,
            ));
            return;
        };

        let rendered = match render_reply_template(template_source, &comment) {
            Ok(rendered) => rendered,
            Err(
                ReplyTemplateError::InvalidSyntax { message }
                | ReplyTemplateError::RenderFailed { message },
            ) => {
                self.error = Some(format!("Reply template rendering failed: {message}"));
                return;
            }
        };

        let Some(draft) = self.active_reply_draft_mut(comment.id) else {
            return;
        };

        if let Err(error) = draft.append_text(rendered.as_str()) {
            self.error = Some(error.to_string());
            return;
        }

        self.error = None;
    }

    fn insert_reply_character(&mut self, character: char) {
        let Some(selected_id) = self.selected_comment().map(|comment| comment.id) else {
            self.error = Some("Reply drafting requires a selected comment".to_owned());
            return;
        };

        let Some(draft) = self.active_reply_draft_mut(selected_id) else {
            return;
        };

        if let Err(error) = draft.push_char(character) {
            self.error = Some(error.to_string());
            return;
        }

        self.error = None;
    }

    fn backspace_reply_draft(&mut self) {
        let Some(selected_id) = self.selected_comment().map(|comment| comment.id) else {
            self.error = Some("Reply drafting requires a selected comment".to_owned());
            return;
        };

        let Some(draft) = self.active_reply_draft_mut(selected_id) else {
            return;
        };

        draft.backspace();
        self.error = None;
    }

    fn request_reply_send(&mut self) {
        let Some(selected_id) = self.selected_comment().map(|comment| comment.id) else {
            self.error = Some("Reply drafting requires a selected comment".to_owned());
            return;
        };

        let Some(draft) = self.active_reply_draft_mut(selected_id) else {
            return;
        };

        if let Err(error) = draft.request_send() {
            self.error = Some(error.to_string());
            return;
        }

        self.error = None;
    }

    fn cancel_reply_draft(&mut self) {
        self.reply_draft = None;
        self.error = None;
    }

    fn active_reply_draft_mut(&mut self, selected_comment_id: u64) -> Option<&mut ReplyDraftState> {
        let Some(draft) = self.reply_draft.as_mut() else {
            self.error = Some("No active reply draft. Press 'a' to start drafting.".to_owned());
            return None;
        };

        if draft.comment_id() != selected_comment_id {
            self.error = Some(
                "Active reply draft does not match the selected comment. Cancel and restart drafting."
                    .to_owned(),
            );
            return None;
        }

        Some(draft)
    }
}

#[cfg(test)]
#[path = "reply_draft_handlers_tests.rs"]
mod tests;
