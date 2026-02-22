//! Reply-draft configuration storage and defaults for TUI startup.

use std::sync::OnceLock;

/// Global storage for reply-drafting configuration.
///
/// This is set before TUI startup from CLI/config sources. When not provided,
/// the application falls back to built-in defaults.
pub(super) static REPLY_DRAFT_CONFIG: OnceLock<ReplyDraftConfig> = OnceLock::new();

/// Static fallback reply-drafting configuration.
pub(super) static DEFAULT_REPLY_DRAFT_CONFIG: OnceLock<ReplyDraftConfig> = OnceLock::new();

/// `NewType` for validated reply draft max length values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplyDraftMaxLength(usize);

impl ReplyDraftMaxLength {
    /// Creates a validated max-length value, normalizing invalid input.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        let normalized = if value == 0 { 1 } else { value };
        Self(normalized)
    }

    /// Returns the inner max-length value.
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl Default for ReplyDraftMaxLength {
    fn default() -> Self {
        Self::new(crate::config::DEFAULT_REPLY_MAX_LENGTH)
    }
}

/// Configuration for template-based reply drafting inside the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyDraftConfig {
    /// Maximum character count for reply drafts.
    pub max_length: ReplyDraftMaxLength,
    /// Ordered template list mapped to keyboard slots `1`-`9`.
    pub templates: Vec<String>,
}

impl Default for ReplyDraftConfig {
    fn default() -> Self {
        Self {
            max_length: ReplyDraftMaxLength::default(),
            templates: crate::config::default_reply_templates(),
        }
    }
}

impl ReplyDraftConfig {
    /// Creates a reply-drafting config while normalizing invalid lengths.
    #[must_use]
    pub const fn new(max_length: ReplyDraftMaxLength, templates: Vec<String>) -> Self {
        Self {
            max_length,
            templates,
        }
    }
}

/// Sets reply-drafting configuration for TUI startup.
///
/// Returns `true` when the value is set for the first time, or `false` when a
/// prior value already exists.
pub fn set_reply_draft_config(config: ReplyDraftConfig) -> bool {
    REPLY_DRAFT_CONFIG.set(config).is_ok()
}

/// Gets reply-drafting configuration, falling back to defaults.
pub(crate) fn get_reply_draft_config() -> ReplyDraftConfig {
    REPLY_DRAFT_CONFIG.get().cloned().unwrap_or_else(|| {
        DEFAULT_REPLY_DRAFT_CONFIG
            .get_or_init(ReplyDraftConfig::default)
            .clone()
    })
}
