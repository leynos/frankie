//! Built-in default reply templates shared across Frankie surfaces.
//!
//! These defaults seed reply drafting when no templates are configured. They
//! are domain policy: the canonical content lives here, and configuration and
//! TUI adapters consume this module rather than defining their own copies.

/// The built-in default reply templates, in their stable presentation order.
///
/// This constant is the single source of truth for Frankie's default reply
/// templates. The ordering is part of the public contract: the interactive
/// TUI binds entries positionally to keyboard slots `1`-`9`. Future minor
/// versions may **append** entries; positional indices are not a stable
/// identity, so do not rely on a fixed length or index for a specific
/// template.
pub const DEFAULT_REPLY_TEMPLATES: &[&str] = &[
    "Thanks for the review on {{ file }}:{{ line }}. I will update this.",
    "Good catch, {{ reviewer }}. I will address this in the next commit.",
    "I have addressed this feedback and pushed an update.",
];

/// Returns an owned copy of [`DEFAULT_REPLY_TEMPLATES`].
///
/// This is the convenient form for seeding owned configuration such as
/// [`crate::config::FrankieConfig`]'s `reply_templates` field. It always
/// derives from [`DEFAULT_REPLY_TEMPLATES`], so the two forms cannot drift.
///
/// # Examples
///
/// ```
/// use frankie::{DEFAULT_REPLY_TEMPLATES, default_reply_templates};
///
/// let owned = default_reply_templates();
/// assert_eq!(owned.len(), DEFAULT_REPLY_TEMPLATES.len());
/// assert!(!owned.is_empty());
/// ```
#[must_use]
pub fn default_reply_templates() -> Vec<String> {
    DEFAULT_REPLY_TEMPLATES
        .iter()
        .map(|template| (*template).to_owned())
        .collect()
}

#[cfg(test)]
#[path = "defaults_tests.rs"]
mod defaults_tests;
