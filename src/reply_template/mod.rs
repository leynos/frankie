//! Shared reply-template rendering APIs for Frankie adapters and library users.
//!
//! This module keeps reply-template rendering out of TUI state so external
//! consumers can render templates through a stable, top-level library API.

use minijinja::{Environment, context};
use thiserror::Error;

use crate::github::models::ReviewComment;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

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

/// Normalized reply-template variables used by the shared renderer.
///
/// This type owns the stable library contract for reply-template rendering.
/// Adapters translate transport-specific inputs, such as [`ReviewComment`],
/// into these ready-to-render fields before calling
/// [`render_reply_template`].
///
/// # Examples
///
/// ```
/// use frankie::{ReplyTemplateContext, render_reply_template};
///
/// # fn main() -> Result<(), frankie::ReplyTemplateError> {
/// let context = ReplyTemplateContext {
///     comment_id: 42,
///     reviewer: "alice".to_owned(),
///     file: "src/lib.rs".to_owned(),
///     line: "12".to_owned(),
///     body: "Please split this into smaller functions.".to_owned(),
/// };
///
/// let rendered = render_reply_template(
///     "Thanks {{ reviewer }} for reviewing {{ file }}:{{ line }}",
///     &context,
/// )?;
///
/// assert_eq!(rendered, "Thanks alice for reviewing src/lib.rs:12");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTemplateContext {
    /// The review comment identifier exposed as `comment_id`.
    pub comment_id: u64,
    /// The reviewer login exposed as `reviewer`.
    pub reviewer: String,
    /// The file path exposed as `file`.
    pub file: String,
    /// The line number exposed as `line`.
    pub line: String,
    /// The comment body exposed as `body`.
    pub body: String,
}

impl From<&ReviewComment> for ReplyTemplateContext {
    fn from(comment: &ReviewComment) -> Self {
        Self {
            comment_id: comment.id,
            reviewer: comment
                .author
                .clone()
                .unwrap_or_else(|| "reviewer".to_owned()),
            file: comment
                .file_path
                .clone()
                .unwrap_or_else(|| "(unknown file)".to_owned()),
            line: comment
                .line_number
                .map_or_else(String::new, |value| value.to_string()),
            body: comment.body.clone().unwrap_or_default(),
        }
    }
}

/// Renders a reply template with data from a reply-template context.
///
/// Templates can use the following variables:
/// - `comment_id`
/// - `reviewer`
/// - `file`
/// - `line`
/// - `body`
///
/// Missing comment fields fall back to the same defaults used by the TUI:
/// `"reviewer"` for `reviewer`, `"(unknown file)"` for `file`, and empty
/// strings for `line` and `body`.
///
/// This function creates a fresh `MiniJinja` environment per call so the
/// public API stays stateless for on-demand adapter usage.
///
/// # Errors
///
/// Returns [`ReplyTemplateError::InvalidSyntax`] when `template_source` fails
/// to parse, or [`ReplyTemplateError::RenderFailed`] when rendering fails.
///
/// # Examples
///
/// ```
/// use frankie::{ReplyTemplateContext, render_reply_template};
///
/// # fn main() -> Result<(), frankie::ReplyTemplateError> {
/// let context = ReplyTemplateContext {
///     comment_id: 42,
///     reviewer: "alice".to_owned(),
///     file: "src/lib.rs".to_owned(),
///     line: "12".to_owned(),
///     body: "Please split this into smaller functions.".to_owned(),
/// };
///
/// let rendered = render_reply_template(
///     "Thanks {{ reviewer }} for reviewing {{ file }}:{{ line }}",
///     &context,
/// )?;
///
/// assert_eq!(rendered, "Thanks alice for reviewing src/lib.rs:12");
/// # Ok(())
/// # }
/// ```
pub fn render_reply_template(
    template_source: &str,
    reply_context: &ReplyTemplateContext,
) -> Result<String, ReplyTemplateError> {
    let mut environment = Environment::new();
    environment.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    environment
        .add_template("reply", template_source)
        .map_err(|error| ReplyTemplateError::InvalidSyntax {
            message: error.to_string(),
        })?;

    let template =
        environment
            .get_template("reply")
            .map_err(|error| ReplyTemplateError::RenderFailed {
                message: error.to_string(),
            })?;

    template
        .render(context! {
            comment_id => reply_context.comment_id,
            reviewer => reply_context.reviewer.as_str(),
            file => reply_context.file.as_str(),
            line => reply_context.line.as_str(),
            body => reply_context.body.as_str(),
        })
        .map_err(|error| ReplyTemplateError::RenderFailed {
            message: error.to_string(),
        })
}
