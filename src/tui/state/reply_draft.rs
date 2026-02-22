//! Reply draft state and template rendering helpers for the review TUI.
//!
//! This module encapsulates editable reply draft state tied to a selected
//! review comment. It enforces a maximum character count, tracks send-readiness,
//! and renders configured templates using `MiniJinja`.

use minijinja::{Environment, context};
use thiserror::Error;

use crate::github::models::ReviewComment;

/// Local reply draft state for a selected review comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyDraftState {
    comment_id: u64,
    text: String,
    max_length: usize,
    ready_to_send: bool,
}

impl ReplyDraftState {
    /// Creates an empty reply draft for the given comment.
    #[must_use]
    pub fn new(comment_id: u64, max_length: usize) -> Self {
        debug_assert!(
            max_length >= 1,
            "reply draft max_length must be normalized before state creation"
        );
        Self {
            comment_id,
            text: String::new(),
            max_length,
            ready_to_send: false,
        }
    }

    /// Returns the selected comment ID associated with this draft.
    #[must_use]
    pub const fn comment_id(&self) -> u64 {
        self.comment_id
    }

    /// Returns the current draft text.
    #[must_use]
    pub const fn text(&self) -> &str {
        self.text.as_str()
    }

    /// Returns the configured maximum character count.
    #[must_use]
    pub const fn max_length(&self) -> usize {
        self.max_length
    }

    /// Returns whether the draft has been marked ready to send.
    #[must_use]
    pub const fn is_ready_to_send(&self) -> bool {
        self.ready_to_send
    }

    /// Returns the current character count using Unicode scalar values.
    #[must_use]
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Returns remaining characters before the draft reaches its limit.
    #[must_use]
    pub fn remaining_chars(&self) -> usize {
        self.max_length.saturating_sub(self.char_count())
    }

    /// Appends free-form text to the draft, enforcing max length.
    ///
    /// # Errors
    ///
    /// Returns [`ReplyDraftError::LengthExceeded`] when appending `suffix`
    /// would exceed the configured maximum length.
    pub fn append_text(&mut self, suffix: &str) -> Result<(), ReplyDraftError> {
        if suffix.is_empty() {
            return Ok(());
        }

        let attempted = self.char_count().saturating_add(suffix.chars().count());
        self.ensure_within_limit(attempted)?;

        self.text.push_str(suffix);
        self.ready_to_send = false;
        Ok(())
    }

    /// Appends one character to the draft, enforcing max length.
    ///
    /// # Errors
    ///
    /// Returns [`ReplyDraftError::LengthExceeded`] when appending `character`
    /// would exceed the configured maximum length.
    pub fn push_char(&mut self, character: char) -> Result<(), ReplyDraftError> {
        let attempted = self.char_count().saturating_add(1);
        self.ensure_within_limit(attempted)?;

        self.text.push(character);
        self.ready_to_send = false;
        Ok(())
    }

    /// Removes the last character from the draft, if present.
    pub fn backspace(&mut self) {
        let _ = self.text.pop();
        self.ready_to_send = false;
    }

    /// Clears the draft text and readiness state.
    pub fn clear(&mut self) {
        self.text.clear();
        self.ready_to_send = false;
    }

    /// Marks the draft as ready to send.
    ///
    /// The draft must be non-empty and within the configured length limit.
    ///
    /// # Errors
    ///
    /// Returns [`ReplyDraftError::EmptyDraft`] when the draft is empty or
    /// whitespace-only, or [`ReplyDraftError::LengthExceeded`] when the draft
    /// length exceeds the configured maximum.
    pub fn request_send(&mut self) -> Result<(), ReplyDraftError> {
        if self.text.trim().is_empty() {
            return Err(ReplyDraftError::EmptyDraft);
        }

        let current_count = self.char_count();
        self.ensure_within_limit(current_count)?;

        self.ready_to_send = true;
        Ok(())
    }

    const fn ensure_within_limit(&self, attempted: usize) -> Result<(), ReplyDraftError> {
        if attempted > self.max_length {
            return Err(ReplyDraftError::LengthExceeded {
                attempted,
                max_length: self.max_length,
            });
        }
        Ok(())
    }
}

/// Errors raised while mutating or validating reply drafts.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReplyDraftError {
    /// The draft text would exceed the configured character limit.
    #[error("reply draft length {attempted} exceeds configured limit {max_length}")]
    LengthExceeded {
        /// Character count after the attempted mutation.
        attempted: usize,
        /// Configured maximum character count.
        max_length: usize,
    },
    /// Sending was requested for an empty draft.
    #[error("reply draft is empty")]
    EmptyDraft,
}

/// Errors raised while rendering a reply template.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReplyTemplateError {
    /// The template source failed to parse.
    #[error("invalid reply template syntax: {message}")]
    InvalidSyntax {
        /// Human-readable parser message from `MiniJinja`.
        message: String,
    },
    /// Rendering failed after successful parsing.
    #[error("reply template rendering failed: {message}")]
    RenderFailed {
        /// Human-readable rendering failure from `MiniJinja`.
        message: String,
    },
}

/// Renders a reply template with data from a selected review comment.
///
/// Templates can use the following variables:
/// - `comment_id`
/// - `reviewer`
/// - `file`
/// - `line`
/// - `body`
///
/// # Errors
///
/// Returns [`ReplyTemplateError::InvalidSyntax`] when `template_source` fails
/// to parse, or [`ReplyTemplateError::RenderFailed`] when rendering fails.
pub fn render_reply_template(
    template_source: &str,
    comment: &ReviewComment,
) -> Result<String, ReplyTemplateError> {
    let mut environment = Environment::new();
    environment.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    environment
        .add_template("reply", template_source)
        .map_err(|error| ReplyTemplateError::InvalidSyntax {
            message: error.to_string(),
        })?;

    let reviewer = comment
        .author
        .clone()
        .unwrap_or_else(|| "reviewer".to_owned());
    let file = comment
        .file_path
        .clone()
        .unwrap_or_else(|| "(unknown file)".to_owned());
    let line = comment
        .line_number
        .map_or_else(String::new, |value| value.to_string());
    let body = comment.body.clone().unwrap_or_default();

    let template =
        environment
            .get_template("reply")
            .map_err(|error| ReplyTemplateError::RenderFailed {
                message: error.to_string(),
            })?;

    template
        .render(context! {
            comment_id => comment.id,
            reviewer => reviewer,
            file => file,
            line => line,
            body => body,
        })
        .map_err(|error| ReplyTemplateError::RenderFailed {
            message: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::{ReplyDraftError, ReplyDraftState, render_reply_template};
    use crate::github::models::ReviewComment;

    #[fixture]
    fn sample_comment() -> ReviewComment {
        ReviewComment {
            id: 42,
            author: Some("alice".to_owned()),
            file_path: Some("src/lib.rs".to_owned()),
            line_number: Some(12),
            body: Some("Please split this into smaller functions.".to_owned()),
            ..ReviewComment::default()
        }
    }

    #[test]
    fn new_draft_starts_empty_and_not_ready() {
        let draft = ReplyDraftState::new(42, 60);

        assert_eq!(draft.comment_id(), 42);
        assert_eq!(draft.text(), "");
        assert_eq!(draft.max_length(), 60);
        assert_eq!(draft.char_count(), 0);
        assert!(!draft.is_ready_to_send());
    }

    #[test]
    fn append_text_respects_max_length() {
        let mut draft = ReplyDraftState::new(42, 10);

        let result = draft.append_text("hello world");
        assert_eq!(
            result,
            Err(ReplyDraftError::LengthExceeded {
                attempted: 11,
                max_length: 10,
            })
        );
        assert_eq!(draft.text(), "");
    }

    #[test]
    fn push_char_and_backspace_update_draft() {
        let mut draft = ReplyDraftState::new(42, 10);

        assert!(draft.push_char('a').is_ok());
        assert!(draft.push_char('b').is_ok());
        assert_eq!(draft.text(), "ab");

        draft.backspace();
        assert_eq!(draft.text(), "a");
    }

    #[test]
    fn request_send_requires_non_empty_draft() {
        let mut draft = ReplyDraftState::new(42, 10);

        let result = draft.request_send();
        assert_eq!(result, Err(ReplyDraftError::EmptyDraft));
        assert!(!draft.is_ready_to_send());
    }

    #[test]
    fn request_send_marks_ready_when_valid() {
        let mut draft = ReplyDraftState::new(42, 10);
        assert!(draft.append_text("done").is_ok());

        assert!(draft.request_send().is_ok());
        assert!(draft.is_ready_to_send());
    }

    #[test]
    fn clear_resets_text_and_readiness() {
        let mut draft = ReplyDraftState::new(42, 10);
        assert!(draft.append_text("done").is_ok());
        assert!(draft.request_send().is_ok());

        draft.clear();

        assert_eq!(draft.text(), "");
        assert!(!draft.is_ready_to_send());
    }

    #[rstest]
    #[case("abc", 3)]
    #[case("é", 1)]
    #[case("🙂", 1)]
    #[case("🙂🙂", 2)]
    fn char_count_uses_unicode_scalar_values(#[case] text: &str, #[case] expected: usize) {
        let mut draft = ReplyDraftState::new(42, 20);
        assert!(draft.append_text(text).is_ok());

        assert_eq!(draft.char_count(), expected);
    }

    #[rstest]
    fn render_reply_template_includes_comment_fields(sample_comment: ReviewComment) {
        let rendered =
            render_reply_template("{{ reviewer }} {{ file }}:{{ line }}", &sample_comment)
                .expect("template should render");

        assert_eq!(rendered, "alice src/lib.rs:12");
    }

    #[rstest]
    fn render_reply_template_reports_invalid_syntax(sample_comment: ReviewComment) {
        let result = render_reply_template("{{ reviewer", &sample_comment);

        assert!(
            matches!(result, Err(super::ReplyTemplateError::InvalidSyntax { .. })),
            "expected invalid syntax error, got {result:?}"
        );
    }
}
