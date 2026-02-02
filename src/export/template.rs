//! Template-driven comment export using Jinja2-compatible syntax.
//!
//! This module provides customisable export formatting using the `minijinja`
//! template engine. Users can supply a template file with Jinja2 syntax to
//! control the structure and content of exported comments.
//!
//! # Template Syntax
//!
//! Templates use Jinja2 syntax:
//! - `{{ variable }}` — variable interpolation
//! - `{% for item in list %}...{% endfor %}` — loops
//! - `{{ list | length }}` — filters
//!
//! # Available Variables
//!
//! **Document-level:**
//! - `pr_url` — pull request URL
//! - `generated_at` — export timestamp (ISO 8601)
//! - `comments` — list of comment objects
//!
//! **Comment-level** (inside `{% for comment in comments %}`):
//! - `comment.id` — comment ID
//! - `comment.file` — file path
//! - `comment.line` — line number
//! - `comment.reviewer` — comment author
//! - `comment.status` — "reply" or "comment"
//! - `comment.body` — comment text
//! - `comment.context` — diff hunk
//! - `comment.commit` — commit SHA
//! - `comment.timestamp` — creation timestamp
//! - `comment.reply_to` — parent comment ID

use std::io::Write;

use chrono::Utc;
use minijinja::{Environment, context};
use serde::Serialize;

use crate::github::IntakeError;

use super::ExportedComment;

/// Template context for a single comment.
///
/// This struct maps [`ExportedComment`] fields to template-friendly names
/// and includes derived fields like `status`.
#[derive(Debug, Clone, Serialize)]
struct TemplateComment {
    /// Comment identifier.
    id: u64,
    /// File path (empty string if not present).
    file: String,
    /// Line number (empty string if not present).
    line: String,
    /// Comment author (empty string if not present).
    reviewer: String,
    /// Derived status: "reply" if this is a reply, "comment" otherwise.
    status: &'static str,
    /// Comment body text (empty string if not present).
    body: String,
    /// Diff hunk context (empty string if not present).
    context: String,
    /// Commit SHA (empty string if not present).
    commit: String,
    /// Creation timestamp (empty string if not present).
    timestamp: String,
    /// Parent comment ID if this is a reply (empty string if not present).
    reply_to: String,
}

impl From<&ExportedComment> for TemplateComment {
    fn from(comment: &ExportedComment) -> Self {
        Self {
            id: comment.id,
            file: comment.file_path.clone().unwrap_or_default(),
            line: comment
                .line_number
                .map_or_else(String::new, |n| n.to_string()),
            reviewer: comment.author.clone().unwrap_or_default(),
            status: if comment.in_reply_to_id.is_some() {
                "reply"
            } else {
                "comment"
            },
            body: comment.body.clone().unwrap_or_default(),
            context: comment.diff_hunk.clone().unwrap_or_default(),
            commit: comment.commit_sha.clone().unwrap_or_default(),
            timestamp: comment.created_at.clone().unwrap_or_default(),
            reply_to: comment
                .in_reply_to_id
                .map_or_else(String::new, |id| id.to_string()),
        }
    }
}

/// Writes comments using a user-provided Jinja2 template.
///
/// The template has access to document-level variables (`pr_url`, `generated_at`,
/// `comments`) and can iterate over comments using `{% for comment in comments %}`.
///
/// # Arguments
///
/// * `writer` — output destination
/// * `comments` — list of comments to export
/// * `pr_url` — pull request URL for template context
/// * `template_content` — raw Jinja2 template string
///
/// # Errors
///
/// Returns [`IntakeError::Configuration`] if the template has syntax errors or
/// references undefined variables. Returns [`IntakeError::Io`] if writing fails.
///
/// # Example Template
///
/// ```jinja2
/// # Export for {{ pr_url }}
///
/// {% for c in comments %}
/// ## {{ c.file }}:{{ c.line }}
/// {{ c.body }}
/// {% endfor %}
/// ```
pub fn write_template<W: Write>(
    writer: &mut W,
    comments: &[ExportedComment],
    pr_url: &str,
    template_content: &str,
) -> Result<(), IntakeError> {
    let mut env = Environment::new();

    // Disable auto-escaping since users control the output format
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    env.add_template("export", template_content)
        .map_err(|e| IntakeError::Configuration {
            message: format!("invalid template syntax: {e}"),
        })?;

    let template_comments: Vec<TemplateComment> =
        comments.iter().map(TemplateComment::from).collect();

    let generated_at = Utc::now().to_rfc3339();

    let ctx = context! {
        pr_url => pr_url,
        generated_at => generated_at,
        comments => template_comments,
    };

    let tmpl = env.get_template("export").map_err(|e| IntakeError::Io {
        message: format!("failed to retrieve template: {e}"),
    })?;

    let output = tmpl.render(ctx).map_err(|e| IntakeError::Configuration {
        message: format!("template rendering failed: {e}"),
    })?;

    writer
        .write_all(output.as_bytes())
        .map_err(|e| IntakeError::Io {
            message: format!("failed to write template output: {e}"),
        })?;

    Ok(())
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
