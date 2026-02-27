//! Shared domain models for AI-powered comment rewriting.

use std::fmt;
use std::str::FromStr;

use thiserror::Error;

use crate::github::models::ReviewComment;

/// Rewriting mode requested by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentRewriteMode {
    /// Expand terse text into a fuller response.
    Expand,
    /// Reword text while preserving intent.
    Reword,
}

impl CommentRewriteMode {
    /// Human-readable action label used in UI output.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Expand => "expand",
            Self::Reword => "reword",
        }
    }
}

impl fmt::Display for CommentRewriteMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Parse error for [`CommentRewriteMode`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unsupported rewrite mode '{value}': valid options are 'expand' or 'reword'")]
pub struct CommentRewriteModeParseError {
    value: String,
}

impl FromStr for CommentRewriteMode {
    type Err = CommentRewriteModeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "expand" => Ok(Self::Expand),
            "reword" => Ok(Self::Reword),
            _ => Err(CommentRewriteModeParseError {
                value: value.to_owned(),
            }),
        }
    }
}

/// Optional review context used to guide AI rewriting.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommentRewriteContext {
    /// Reviewer login for social tone alignment.
    pub reviewer: Option<String>,
    /// File path related to the comment.
    pub file_path: Option<String>,
    /// Line number related to the comment.
    pub line_number: Option<u32>,
    /// Original review-comment body.
    pub comment_body: Option<String>,
}

impl From<&ReviewComment> for CommentRewriteContext {
    fn from(comment: &ReviewComment) -> Self {
        Self {
            reviewer: comment.author.clone(),
            file_path: comment.file_path.clone(),
            line_number: comment.line_number,
            comment_body: comment.body.clone(),
        }
    }
}

/// Input payload for an AI rewrite request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRewriteRequest {
    mode: CommentRewriteMode,
    source_text: String,
    context: CommentRewriteContext,
}

impl CommentRewriteRequest {
    /// Construct a request from explicit mode/text/context values.
    #[must_use]
    pub fn new(
        mode: CommentRewriteMode,
        source_text: impl Into<String>,
        context: CommentRewriteContext,
    ) -> Self {
        Self {
            mode,
            source_text: source_text.into(),
            context,
        }
    }

    /// Requested rewrite mode.
    #[must_use]
    pub const fn mode(&self) -> CommentRewriteMode {
        self.mode
    }

    /// Input text that should be rewritten.
    #[must_use]
    pub const fn source_text(&self) -> &str {
        self.source_text.as_str()
    }

    /// Optional review context attached to the request.
    #[must_use]
    pub const fn context(&self) -> &CommentRewriteContext {
        &self.context
    }
}

/// AI-generated rewrite payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRewriteGenerated {
    /// Generated candidate text.
    pub rewritten_text: String,
    /// Provenance label required by acceptance criteria.
    pub origin_label: String,
}

/// Graceful fallback payload when AI rewriting fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRewriteFallback {
    /// Original unmodified source text.
    pub original_text: String,
    /// User-readable fallback reason.
    pub reason: String,
}

/// Domain result for AI rewrite attempts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentRewriteOutcome {
    /// AI produced a candidate rewrite.
    Generated(CommentRewriteGenerated),
    /// AI request failed and original text is preserved.
    Fallback(CommentRewriteFallback),
}

impl CommentRewriteOutcome {
    /// Construct a generated outcome with standard provenance label.
    #[must_use]
    pub fn generated(rewritten_text: impl Into<String>) -> Self {
        Self::Generated(CommentRewriteGenerated {
            rewritten_text: rewritten_text.into(),
            origin_label: "AI-originated".to_owned(),
        })
    }

    /// Construct a fallback outcome preserving original text.
    #[must_use]
    pub fn fallback(original_text: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Fallback(CommentRewriteFallback {
            original_text: original_text.into(),
            reason: reason.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{CommentRewriteMode, CommentRewriteOutcome, CommentRewriteRequest};

    #[rstest]
    #[case("expand", Some(CommentRewriteMode::Expand))]
    #[case("reword", Some(CommentRewriteMode::Reword))]
    #[case("EXPAND", Some(CommentRewriteMode::Expand))]
    #[case("rephrase", None)]
    fn parse_mode(#[case] value: &str, #[case] expected: Option<CommentRewriteMode>) {
        let parsed = value.parse::<CommentRewriteMode>();
        match expected {
            Some(mode) => assert_eq!(parsed.ok(), Some(mode)),
            None => assert!(parsed.is_err(), "expected parse error for {value}"),
        }
    }

    #[test]
    fn request_accessors_return_expected_values() {
        let request = CommentRewriteRequest::new(
            CommentRewriteMode::Expand,
            "hello",
            super::CommentRewriteContext::default(),
        );

        assert_eq!(request.mode(), CommentRewriteMode::Expand);
        assert_eq!(request.source_text(), "hello");
        assert_eq!(request.context(), &super::CommentRewriteContext::default());
    }

    #[test]
    fn outcome_generated_sets_origin_label() {
        let outcome = CommentRewriteOutcome::generated("rewritten");

        let super::CommentRewriteOutcome::Generated(payload) = outcome else {
            panic!("expected generated outcome");
        };

        assert_eq!(payload.origin_label, "AI-originated");
        assert_eq!(payload.rewritten_text, "rewritten");
    }
}
