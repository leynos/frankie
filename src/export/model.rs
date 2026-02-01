//! Export data models for structured comment output.
//!
//! This module defines the serializable structures used for exporting review
//! comments and the format selection enum for CLI integration.

use std::fmt;
use std::str::FromStr;

use serde::Serialize;

use crate::github::{IntakeError, ReviewComment};

/// A review comment prepared for export with all relevant metadata.
///
/// This structure is designed for serialization and includes only the fields
/// needed for structured export. It is constructed from a [`ReviewComment`]
/// via the [`From`] trait implementation.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExportedComment {
    /// Comment identifier.
    pub id: u64,
    /// Author login.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// File path the comment is attached to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Line number in the diff.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
    /// Original line number before changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_line_number: Option<u32>,
    /// Comment body text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Diff hunk context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_hunk: Option<String>,
    /// Commit SHA this comment was made against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// ID of the parent comment if this is a reply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to_id: Option<u64>,
    /// Creation timestamp (ISO 8601 format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl From<&ReviewComment> for ExportedComment {
    fn from(comment: &ReviewComment) -> Self {
        Self {
            id: comment.id,
            author: comment.author.clone(),
            file_path: comment.file_path.clone(),
            line_number: comment.line_number,
            original_line_number: comment.original_line_number,
            body: comment.body.clone(),
            diff_hunk: comment.diff_hunk.clone(),
            commit_sha: comment.commit_sha.clone(),
            in_reply_to_id: comment.in_reply_to_id,
            created_at: comment.created_at.clone(),
        }
    }
}

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Human-readable Markdown with syntax-highlighted code blocks.
    Markdown,
    /// Machine-readable JSON Lines (one object per line).
    Jsonl,
}

impl FromStr for ExportFormat {
    type Err = IntakeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => Ok(Self::Markdown),
            "jsonl" | "json-lines" | "jsonlines" => Ok(Self::Jsonl),
            _ => Err(IntakeError::Configuration {
                message: format!(
                    "unsupported export format '{s}': valid options are 'markdown' or 'jsonl'"
                ),
            }),
        }
    }
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Markdown => write!(f, "markdown"),
            Self::Jsonl => write!(f, "jsonl"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn from_review_comment_preserves_all_fields() {
        let comment = ReviewComment {
            id: 123,
            body: Some("Fix this".to_owned()),
            author: Some("alice".to_owned()),
            file_path: Some("src/lib.rs".to_owned()),
            line_number: Some(42),
            original_line_number: Some(40),
            diff_hunk: Some("@@ -40,3 +40,5 @@".to_owned()),
            commit_sha: Some("abc123".to_owned()),
            in_reply_to_id: Some(100),
            created_at: Some("2025-01-15T10:00:00Z".to_owned()),
            updated_at: Some("2025-01-15T11:00:00Z".to_owned()),
        };

        let exported = ExportedComment::from(&comment);

        assert_eq!(exported.id, 123);
        assert_eq!(exported.body.as_deref(), Some("Fix this"));
        assert_eq!(exported.author.as_deref(), Some("alice"));
        assert_eq!(exported.file_path.as_deref(), Some("src/lib.rs"));
        assert_eq!(exported.line_number, Some(42));
        assert_eq!(exported.original_line_number, Some(40));
        assert_eq!(exported.diff_hunk.as_deref(), Some("@@ -40,3 +40,5 @@"));
        assert_eq!(exported.commit_sha.as_deref(), Some("abc123"));
        assert_eq!(exported.in_reply_to_id, Some(100));
        assert_eq!(exported.created_at.as_deref(), Some("2025-01-15T10:00:00Z"));
    }

    #[rstest]
    fn from_review_comment_handles_none_values() {
        let comment = ReviewComment {
            id: 456,
            ..Default::default()
        };

        let exported = ExportedComment::from(&comment);

        assert_eq!(exported.id, 456);
        assert!(exported.body.is_none());
        assert!(exported.author.is_none());
        assert!(exported.file_path.is_none());
        assert!(exported.line_number.is_none());
    }

    #[rstest]
    #[case("markdown", ExportFormat::Markdown)]
    #[case("Markdown", ExportFormat::Markdown)]
    #[case("MARKDOWN", ExportFormat::Markdown)]
    #[case("md", ExportFormat::Markdown)]
    #[case("jsonl", ExportFormat::Jsonl)]
    #[case("JSONL", ExportFormat::Jsonl)]
    #[case("json-lines", ExportFormat::Jsonl)]
    #[case("jsonlines", ExportFormat::Jsonl)]
    fn export_format_parses_valid_values(
        #[case] input: &str,
        #[case] expected: ExportFormat,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let parsed: ExportFormat = input.parse()?;
        if parsed != expected {
            return Err(format!("expected {expected:?}, got {parsed:?}").into());
        }
        Ok(())
    }

    #[rstest]
    #[case("xml")]
    #[case("csv")]
    #[case("yaml")]
    #[case("")]
    fn export_format_rejects_invalid_values(#[case] input: &str) {
        let result: Result<ExportFormat, _> = input.parse();
        assert!(result.is_err());
        let err = result.expect_err("should reject invalid format");
        assert!(
            matches!(err, IntakeError::Configuration { ref message } if message.contains("unsupported export format")),
            "expected Configuration error with 'unsupported export format', got {err:?}"
        );
    }

    #[rstest]
    fn export_format_display() {
        assert_eq!(ExportFormat::Markdown.to_string(), "markdown");
        assert_eq!(ExportFormat::Jsonl.to_string(), "jsonl");
    }
}
