//! Domain model for automated comment resolution verification.

use std::fmt::{self, Display, Formatter};

/// Verified/unverified status for a review comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentVerificationStatus {
    /// The referenced line was removed or changed between commits.
    Verified,
    /// The referenced line appears unchanged or verification could not be
    /// performed deterministically.
    Unverified,
}

impl CommentVerificationStatus {
    /// Human-readable symbol for display in TUI/CLI outputs.
    #[must_use]
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Verified => "✓",
            Self::Unverified => "✗",
        }
    }

    /// Stable database representation for persistence.
    #[must_use]
    pub const fn as_db_value(&self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
        }
    }

    /// Parses a database value into a status.
    #[must_use]
    pub fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "verified" => Some(Self::Verified),
            "unverified" => Some(Self::Unverified),
            _ => None,
        }
    }

    /// User-facing string representation for display.
    #[must_use]
    pub const fn as_display_str(&self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
        }
    }
}

impl Display for CommentVerificationStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_display_str())
    }
}

/// Evidence kinds explaining why a status was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentVerificationEvidenceKind {
    /// The comment is missing required metadata (commit SHA, file path, line).
    InsufficientMetadata,
    /// The referenced line was deleted (or the file/line could not be mapped).
    LineRemoved,
    /// The referenced line's content changed between commits.
    LineChanged,
    /// The referenced line's content appears unchanged between commits.
    LineUnchanged,
    /// Repository data could not be loaded deterministically.
    RepositoryDataUnavailable,
    /// A referenced line number was outside the file bounds.
    LineOutOfBounds,
}

impl CommentVerificationEvidenceKind {
    /// Stable database representation for persistence.
    #[must_use]
    pub const fn as_db_value(&self) -> &'static str {
        match self {
            Self::InsufficientMetadata => "insufficient_metadata",
            Self::LineRemoved => "line_removed",
            Self::LineChanged => "line_changed",
            Self::LineUnchanged => "line_unchanged",
            Self::RepositoryDataUnavailable => "repository_data_unavailable",
            Self::LineOutOfBounds => "line_out_of_bounds",
        }
    }

    /// Parses a database value into an evidence kind.
    #[must_use]
    pub fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "insufficient_metadata" => Some(Self::InsufficientMetadata),
            "line_removed" => Some(Self::LineRemoved),
            "line_changed" => Some(Self::LineChanged),
            "line_unchanged" => Some(Self::LineUnchanged),
            "repository_data_unavailable" => Some(Self::RepositoryDataUnavailable),
            "line_out_of_bounds" => Some(Self::LineOutOfBounds),
            _ => None,
        }
    }

    /// User-facing string representation for display.
    #[must_use]
    pub const fn as_display_str(&self) -> &'static str {
        match self {
            Self::InsufficientMetadata => "insufficient metadata",
            Self::LineRemoved => "line removed",
            Self::LineChanged => "line changed",
            Self::LineUnchanged => "line unchanged",
            Self::RepositoryDataUnavailable => "repository data unavailable",
            Self::LineOutOfBounds => "line out of bounds",
        }
    }
}

impl Display for CommentVerificationEvidenceKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_display_str())
    }
}

/// Evidence explaining a verification verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentVerificationEvidence {
    /// The evidence category.
    pub kind: CommentVerificationEvidenceKind,
    /// Optional human-readable detail for display.
    pub message: Option<String>,
}

/// Verification result for a single GitHub review comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentVerificationResult {
    github_comment_id: u64,
    target_sha: String,
    status: CommentVerificationStatus,
    evidence: CommentVerificationEvidence,
}

impl CommentVerificationResult {
    /// Creates a new verification result.
    #[must_use]
    pub const fn new(
        github_comment_id: u64,
        target_sha: String,
        status: CommentVerificationStatus,
        evidence: CommentVerificationEvidence,
    ) -> Self {
        Self {
            github_comment_id,
            target_sha,
            status,
            evidence,
        }
    }

    /// Returns the GitHub review comment ID.
    #[must_use]
    pub const fn github_comment_id(&self) -> u64 {
        self.github_comment_id
    }

    /// Returns the target commit SHA the verification was run against.
    #[must_use]
    pub const fn target_sha(&self) -> &str {
        self.target_sha.as_str()
    }

    /// Returns the verification status.
    #[must_use]
    pub const fn status(&self) -> CommentVerificationStatus {
        self.status
    }

    /// Returns the evidence explaining the status.
    #[must_use]
    pub const fn evidence(&self) -> &CommentVerificationEvidence {
        &self.evidence
    }
}
