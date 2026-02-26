//! Reply drafting handlers for the review TUI.
//!
//! This module implements keyboard-driven template insertion and inline reply
//! editing while enforcing configured length limits.

use std::any::Any;
use std::sync::Arc;

use bubbletea_rs::Cmd;

use crate::ai::{
    CommentRewriteContext, CommentRewriteMode, CommentRewriteOutcome, CommentRewriteRequest,
    CommentRewriteService, build_side_by_side_diff_preview, rewrite_with_fallback,
};
use crate::tui::messages::AppMsg;
use crate::tui::state::{ReplyDraftState, ReplyTemplateError, render_reply_template};

use super::{ReplyDraftAiPreview, ReviewApp};

impl ReviewApp {
    /// Handles reply-drafting messages.
    pub(super) fn handle_reply_draft_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::StartReplyDraft => {
                self.start_reply_draft();
                None
            }
            AppMsg::ReplyDraftInsertTemplate { template_index } => {
                self.insert_reply_template(*template_index);
                None
            }
            AppMsg::ReplyDraftInsertChar(character) => {
                self.insert_reply_character(*character);
                None
            }
            AppMsg::ReplyDraftBackspace => {
                self.backspace_reply_draft();
                None
            }
            AppMsg::ReplyDraftRequestSend => {
                self.request_reply_send();
                None
            }
            AppMsg::ReplyDraftCancel => {
                self.cancel_reply_draft();
                None
            }
            AppMsg::ReplyDraftRequestAiRewrite { mode } => self.request_ai_rewrite(*mode),
            AppMsg::ReplyDraftAiRewriteReady { mode, outcome } => {
                self.handle_ai_rewrite_ready(*mode, outcome);
                None
            }
            AppMsg::ReplyDraftAiApply => {
                self.apply_ai_rewrite_preview();
                None
            }
            AppMsg::ReplyDraftAiDiscard => {
                self.discard_ai_rewrite_preview();
                None
            }
            _ => None,
        }
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
        self.reply_draft_ai_preview = None;
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

        let template_source_owned = template_source.clone();

        if self.reply_draft.is_none() {
            self.error = Some("No active reply draft. Press 'a' to start drafting.".to_owned());
            return;
        }

        if self.active_reply_draft_mut(comment.id).is_none() {
            self.error = Some(
                "Active reply draft does not match the selected comment. Cancel and restart drafting."
                    .to_owned(),
            );
            return;
        }

        let rendered = match render_reply_template(template_source_owned.as_str(), &comment) {
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
            self.error = Some(
                "Active reply draft does not match the selected comment. Cancel and restart drafting."
                    .to_owned(),
            );
            return;
        };

        if let Err(error) = draft.append_text(rendered.as_str()) {
            self.error = Some(error.to_string());
            return;
        }

        self.reply_draft_ai_preview = None;
        self.error = None;
    }

    fn insert_reply_character(&mut self, character: char) {
        self.with_active_draft_operation(|draft| draft.push_char(character));
    }

    fn backspace_reply_draft(&mut self) {
        match self.get_active_draft_for_editing() {
            Ok(draft) => {
                draft.backspace();
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error);
            }
        }
    }

    fn request_reply_send(&mut self) {
        self.with_active_draft_operation(ReplyDraftState::request_send);
    }

    fn cancel_reply_draft(&mut self) {
        self.reply_draft = None;
        self.reply_draft_ai_preview = None;
        self.error = None;
    }

    /// Executes an operation on the active draft, handling all error mapping.
    fn with_active_draft_operation<E>(
        &mut self,
        operation: impl FnOnce(&mut ReplyDraftState) -> Result<(), E>,
    ) where
        E: std::fmt::Display,
    {
        match self.get_active_draft_for_editing() {
            Ok(draft) => {
                if let Err(error) = operation(draft) {
                    self.error = Some(error.to_string());
                } else {
                    self.reply_draft_ai_preview = None;
                    self.error = None;
                }
            }
            Err(error) => {
                self.error = Some(error);
            }
        }
    }

    /// Gets the active reply draft for editing, validating preconditions.
    ///
    /// Returns an error if:
    /// - No comment is selected
    /// - No reply draft is active
    /// - The active draft doesn't match the selected comment
    fn get_active_draft_for_editing(&mut self) -> Result<&mut ReplyDraftState, String> {
        let selected_id = self
            .selected_comment()
            .map(|comment| comment.id)
            .ok_or_else(|| "Reply drafting requires a selected comment".to_owned())?;

        if self.reply_draft.is_none() {
            return Err("No active reply draft. Press 'a' to start drafting.".to_owned());
        }

        self.active_reply_draft_mut(selected_id).ok_or_else(|| {
            "Active reply draft does not match the selected comment. Cancel and restart drafting."
                .to_owned()
        })
    }

    fn active_reply_draft_mut(&mut self, selected_comment_id: u64) -> Option<&mut ReplyDraftState> {
        let draft = self.reply_draft.as_mut()?;

        if draft.comment_id() != selected_comment_id {
            return None;
        }

        Some(draft)
    }

    fn request_ai_rewrite(&mut self, mode: CommentRewriteMode) -> Option<Cmd> {
        let Some(comment) = self.selected_comment().cloned() else {
            self.error = Some("Reply drafting requires a selected comment".to_owned());
            return None;
        };

        let Some(draft) = self.active_reply_draft_mut(comment.id) else {
            self.error = Some("No active reply draft. Press 'a' to start drafting.".to_owned());
            return None;
        };

        let source_text = draft.text().to_owned();
        if source_text.trim().is_empty() {
            self.error = Some("Reply draft is empty; type text before AI rewrite.".to_owned());
            return None;
        }

        let request =
            CommentRewriteRequest::new(mode, source_text, CommentRewriteContext::from(&comment));

        self.error = None;
        Some(spawn_ai_rewrite_request(
            Arc::clone(&self.comment_rewrite_service),
            request,
            mode,
        ))
    }

    fn handle_ai_rewrite_ready(
        &mut self,
        mode: CommentRewriteMode,
        outcome: &CommentRewriteOutcome,
    ) {
        match outcome {
            CommentRewriteOutcome::Generated(generated) => {
                let Some(comment_id) = self.selected_comment().map(|comment| comment.id) else {
                    self.error = Some("Reply drafting requires a selected comment".to_owned());
                    return;
                };
                let Some(draft) = self.active_reply_draft_mut(comment_id) else {
                    self.error =
                        Some("No active reply draft. Press 'a' to start drafting.".to_owned());
                    return;
                };

                let preview = build_side_by_side_diff_preview(
                    draft.text(),
                    generated.rewritten_text.as_str(),
                );
                self.reply_draft_ai_preview = Some(ReplyDraftAiPreview {
                    mode,
                    rewritten_text: generated.rewritten_text.clone(),
                    origin_label: generated.origin_label.clone(),
                    side_by_side_preview: preview,
                });
                self.error = None;
            }
            CommentRewriteOutcome::Fallback(fallback) => {
                self.reply_draft_ai_preview = None;
                self.error = Some(fallback.reason.clone());
            }
        }
    }

    fn apply_ai_rewrite_preview(&mut self) {
        let Some(preview) = self.reply_draft_ai_preview.clone() else {
            self.error = Some("No AI rewrite preview to apply.".to_owned());
            return;
        };

        match self.get_active_draft_for_editing() {
            Ok(draft) => {
                if let Err(error) =
                    draft.replace_text(preview.rewritten_text.as_str(), Some(preview.origin_label))
                {
                    self.error = Some(error.to_string());
                    return;
                }

                self.reply_draft_ai_preview = None;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error);
            }
        }
    }

    fn discard_ai_rewrite_preview(&mut self) {
        self.reply_draft_ai_preview = None;
        self.error = None;
    }
}

fn spawn_ai_rewrite_request(
    service: Arc<dyn CommentRewriteService>,
    request: CommentRewriteRequest,
    mode: CommentRewriteMode,
) -> Cmd {
    Box::pin(async move {
        let outcome = match tokio::task::spawn_blocking(move || {
            rewrite_with_fallback(service.as_ref(), &request)
        })
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => CommentRewriteOutcome::fallback(
                String::new(),
                format!("AI rewrite task failed: {error}"),
            ),
        };

        Some(Box::new(AppMsg::ReplyDraftAiRewriteReady { mode, outcome }) as Box<dyn Any + Send>)
    })
}

#[cfg(test)]
#[path = "reply_draft_handlers_tests.rs"]
mod tests;
