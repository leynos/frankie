//! Shared reply-template rendering APIs for Frankie adapters and library users.
//!
//! This module keeps reply-template rendering out of TUI state so external
//! consumers can render templates through a stable, top-level library API.

use minijinja::{Environment, context};
use thiserror::Error;

use crate::github::models::ReviewComment;

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

/// Renders a reply template with data from a selected review comment.
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
/// # Errors
///
/// Returns [`ReplyTemplateError::InvalidSyntax`] when `template_source` fails
/// to parse, or [`ReplyTemplateError::RenderFailed`] when rendering fails.
///
/// # Examples
///
/// ```
/// use frankie::{ReviewComment, render_reply_template};
///
/// # fn main() -> Result<(), frankie::ReplyTemplateError> {
/// let comment = ReviewComment {
///     id: 42,
///     author: Some("alice".to_owned()),
///     file_path: Some("src/lib.rs".to_owned()),
///     line_number: Some(12),
///     body: Some("Please split this into smaller functions.".to_owned()),
///     ..ReviewComment::default()
/// };
///
/// let rendered = render_reply_template(
///     "Thanks {{ reviewer }} for reviewing {{ file }}:{{ line }}",
///     &comment,
/// )?;
///
/// assert_eq!(rendered, "Thanks alice for reviewing src/lib.rs:12");
/// # Ok(())
/// # }
/// ```
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
