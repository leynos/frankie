//! Reply draft state helpers for the review TUI.
//!
//! This module encapsulates editable reply draft state tied to a selected
//! review comment. It enforces a maximum character count and tracks
//! send-readiness for inline replies.

use thiserror::Error;

use crate::tui::ReplyDraftMaxLength;

/// Local reply draft state for a selected review comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyDraftState {
    comment_id: u64,
    text: String,
    origin_label: Option<String>,
    max_length: ReplyDraftMaxLength,
    ready_to_send: bool,
}

impl ReplyDraftState {
    /// Creates an empty reply draft for the given comment.
    #[must_use]
    pub const fn new(comment_id: u64, max_length: ReplyDraftMaxLength) -> Self {
        Self {
            comment_id,
            text: String::new(),
            origin_label: None,
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

    /// Returns the optional origin label for the current draft text.
    #[must_use]
    pub fn origin_label(&self) -> Option<&str> {
        self.origin_label.as_deref()
    }

    /// Returns the configured maximum character count.
    #[must_use]
    pub const fn max_length(&self) -> ReplyDraftMaxLength {
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
        self.max_length.as_usize().saturating_sub(self.char_count())
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
        self.mark_manual_edit();
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
        self.mark_manual_edit();
        Ok(())
    }

    /// Removes the last character from the draft, if present.
    pub fn backspace(&mut self) {
        let _ = self.text.pop();
        self.mark_manual_edit();
    }

    /// Clears the draft text and readiness state.
    pub fn clear(&mut self) {
        self.text.clear();
        self.mark_manual_edit();
    }

    /// Replaces the draft text and optional provenance label.
    ///
    /// # Errors
    ///
    /// Returns [`ReplyDraftError::LengthExceeded`] when `text` exceeds the
    /// configured maximum length.
    pub fn replace_text(
        &mut self,
        text: &str,
        origin_label: Option<String>,
    ) -> Result<(), ReplyDraftError> {
        self.ensure_within_limit(text.chars().count())?;
        self.text.clear();
        self.text.push_str(text);
        self.origin_label = origin_label.filter(|label| !label.trim().is_empty());
        self.ready_to_send = false;
        Ok(())
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
        if attempted > self.max_length.as_usize() {
            return Err(ReplyDraftError::LengthExceeded {
                attempted,
                max_length: self.max_length.as_usize(),
            });
        }
        Ok(())
    }

    fn mark_manual_edit(&mut self) {
        self.ready_to_send = false;
        self.origin_label = None;
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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{ReplyDraftError, ReplyDraftState};
    use crate::tui::ReplyDraftMaxLength;

    #[test]
    fn new_draft_starts_empty_and_not_ready() {
        let draft = ReplyDraftState::new(42, ReplyDraftMaxLength::new(60));

        assert_eq!(draft.comment_id(), 42);
        assert_eq!(draft.text(), "");
        assert_eq!(draft.origin_label(), None);
        assert_eq!(draft.max_length().as_usize(), 60);
        assert_eq!(draft.char_count(), 0);
        assert!(!draft.is_ready_to_send());
    }

    #[test]
    fn append_text_respects_max_length() {
        let mut draft = ReplyDraftState::new(42, ReplyDraftMaxLength::new(10));

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
        let mut draft = ReplyDraftState::new(42, ReplyDraftMaxLength::new(10));

        assert!(draft.push_char('a').is_ok());
        assert!(draft.push_char('b').is_ok());
        assert_eq!(draft.text(), "ab");

        draft.backspace();
        assert_eq!(draft.text(), "a");
    }

    #[test]
    fn request_send_requires_non_empty_draft() {
        let mut draft = ReplyDraftState::new(42, ReplyDraftMaxLength::new(10));

        let result = draft.request_send();
        assert_eq!(result, Err(ReplyDraftError::EmptyDraft));
        assert!(!draft.is_ready_to_send());
    }

    #[test]
    fn request_send_marks_ready_when_valid() {
        let mut draft = ReplyDraftState::new(42, ReplyDraftMaxLength::new(10));
        assert!(draft.append_text("done").is_ok());

        assert!(draft.request_send().is_ok());
        assert!(draft.is_ready_to_send());
    }

    #[test]
    fn clear_resets_text_and_readiness() {
        let mut draft = ReplyDraftState::new(42, ReplyDraftMaxLength::new(10));
        assert!(draft.append_text("done").is_ok());
        assert!(draft.request_send().is_ok());

        draft.clear();

        assert_eq!(draft.text(), "");
        assert_eq!(draft.origin_label(), None);
        assert!(!draft.is_ready_to_send());
    }

    #[test]
    fn replace_text_sets_origin_label() {
        let mut draft = ReplyDraftState::new(42, ReplyDraftMaxLength::new(40));

        let result = draft.replace_text("Expanded text", Some("AI-originated".to_owned()));
        assert!(result.is_ok());
        assert_eq!(draft.text(), "Expanded text");
        assert_eq!(draft.origin_label(), Some("AI-originated"));
    }

    #[test]
    fn manual_edit_clears_origin_label() {
        let mut draft = ReplyDraftState::new(42, ReplyDraftMaxLength::new(40));
        assert!(
            draft
                .replace_text("Expanded", Some("AI-originated".to_owned()))
                .is_ok()
        );

        assert!(draft.push_char('!').is_ok());

        assert_eq!(draft.origin_label(), None);
    }

    #[rstest]
    #[case("abc", 3)]
    #[case("é", 1)]
    #[case("🙂", 1)]
    #[case("🙂🙂", 2)]
    fn char_count_uses_unicode_scalar_values(#[case] text: &str, #[case] expected: usize) {
        let mut draft = ReplyDraftState::new(42, ReplyDraftMaxLength::new(20));
        assert!(draft.append_text(text).is_ok());

        assert_eq!(draft.char_count(), expected);
    }
}
