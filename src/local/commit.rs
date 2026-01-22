//! Types for representing commit snapshots and line mapping results.
//!
//! This module provides the core data types used for time-travel navigation
//! across PR history, including commit metadata, file content snapshots, and
//! line mapping verification results.

use chrono::{DateTime, Utc};

/// Metadata for a Git commit.
///
/// This struct encapsulates the common metadata fields for a commit, reducing
/// the number of parameters needed for constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitMetadata {
    /// The full commit SHA.
    pub sha: String,
    /// The commit message (first line only).
    pub message: String,
    /// The commit author name.
    pub author: String,
    /// The commit timestamp.
    pub timestamp: DateTime<Utc>,
}

impl CommitMetadata {
    /// Creates new commit metadata.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Const fn with String parameters is misleading since String allocation is not const"
    )]
    pub fn new(sha: String, message: String, author: String, timestamp: DateTime<Utc>) -> Self {
        Self {
            sha,
            message,
            author,
            timestamp,
        }
    }
}

/// A snapshot of a commit with relevant metadata and optional file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSnapshot {
    /// The full commit SHA.
    sha: String,
    /// Short form of the commit SHA (first 7 characters).
    short_sha: String,
    /// The commit message (first line only).
    message: String,
    /// The commit author name.
    author: String,
    /// The commit timestamp.
    timestamp: DateTime<Utc>,
    /// File content at this commit, if requested and available.
    file_content: Option<String>,
    /// Path to the file, if file content was requested.
    file_path: Option<String>,
}

impl CommitSnapshot {
    /// Private helper to construct a snapshot from metadata and optional file data.
    fn from_metadata(
        metadata: CommitMetadata,
        file_path: Option<String>,
        file_content: Option<String>,
    ) -> Self {
        let short_sha = metadata.sha.chars().take(7).collect();
        Self {
            sha: metadata.sha,
            short_sha,
            message: metadata.message,
            author: metadata.author,
            timestamp: metadata.timestamp,
            file_content,
            file_path,
        }
    }

    /// Creates a new commit snapshot.
    #[must_use]
    pub fn new(metadata: CommitMetadata) -> Self {
        Self::from_metadata(metadata, None, None)
    }

    /// Creates a commit snapshot with file content.
    #[must_use]
    pub fn with_file_content(
        metadata: CommitMetadata,
        file_path: String,
        file_content: String,
    ) -> Self {
        Self::from_metadata(metadata, Some(file_path), Some(file_content))
    }

    /// Returns the full commit SHA.
    #[must_use]
    pub fn sha(&self) -> &str {
        &self.sha
    }

    /// Returns the short commit SHA (7 characters).
    #[must_use]
    pub fn short_sha(&self) -> &str {
        &self.short_sha
    }

    /// Returns the commit message (first line).
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the commit author name.
    #[must_use]
    pub fn author(&self) -> &str {
        &self.author
    }

    /// Returns the commit timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> &DateTime<Utc> {
        &self.timestamp
    }

    /// Returns the file content at this commit, if available.
    #[must_use]
    pub fn file_content(&self) -> Option<&str> {
        self.file_content.as_deref()
    }

    /// Returns the file path, if file content was requested.
    #[must_use]
    pub fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }
}

/// Result of verifying a line mapping between two commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMappingStatus {
    /// Line exists at the exact same position in the new commit.
    Exact,
    /// Line was moved to a different position.
    Moved,
    /// Line was deleted in the new commit.
    Deleted,
    /// Line could not be found or mapped (file may not exist).
    NotFound,
}

impl LineMappingStatus {
    /// Returns a human-readable symbol for the status.
    #[must_use]
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Exact => "✓",
            Self::Moved => "→",
            Self::Deleted => "✗",
            Self::NotFound => "?",
        }
    }

    /// Returns a human-readable description of the status.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Exact => "exact match",
            Self::Moved => "moved",
            Self::Deleted => "deleted",
            Self::NotFound => "not found",
        }
    }
}

/// Verification result for a line mapping between commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMappingVerification {
    /// Original line number in the source commit.
    original_line: u32,
    /// Current line number in the target commit, if applicable.
    current_line: Option<u32>,
    /// The mapping status.
    status: LineMappingStatus,
}

impl LineMappingVerification {
    /// Creates a verification result for an exact match.
    #[must_use]
    pub const fn exact(line: u32) -> Self {
        Self {
            original_line: line,
            current_line: Some(line),
            status: LineMappingStatus::Exact,
        }
    }

    /// Creates a verification result for a moved line.
    #[must_use]
    pub const fn moved(original: u32, current: u32) -> Self {
        Self {
            original_line: original,
            current_line: Some(current),
            status: LineMappingStatus::Moved,
        }
    }

    /// Creates a verification result for a deleted line.
    #[must_use]
    pub const fn deleted(line: u32) -> Self {
        Self {
            original_line: line,
            current_line: None,
            status: LineMappingStatus::Deleted,
        }
    }

    /// Creates a verification result for a line that could not be found.
    #[must_use]
    pub const fn not_found(line: u32) -> Self {
        Self {
            original_line: line,
            current_line: None,
            status: LineMappingStatus::NotFound,
        }
    }

    /// Returns the original line number.
    #[must_use]
    pub const fn original_line(&self) -> u32 {
        self.original_line
    }

    /// Returns the current line number, if applicable.
    #[must_use]
    pub const fn current_line(&self) -> Option<u32> {
        self.current_line
    }

    /// Returns the mapping status.
    #[must_use]
    pub const fn status(&self) -> LineMappingStatus {
        self.status
    }

    /// Returns the line offset (positive = moved down, negative = moved up).
    ///
    /// Returns `None` if the line was deleted or if the conversion to `i32` fails.
    #[must_use]
    pub fn offset(&self) -> Option<i32> {
        self.current_line.and_then(|current| {
            let current_i32 = i32::try_from(current).ok()?;
            let original_i32 = i32::try_from(self.original_line).ok()?;
            Some(current_i32.saturating_sub(original_i32))
        })
    }

    /// Formats the verification as a display string.
    #[must_use]
    pub fn display(&self) -> String {
        match self.status {
            LineMappingStatus::Exact => {
                format!(
                    "{} Line {} → {} ({})",
                    self.status.symbol(),
                    self.original_line,
                    self.original_line,
                    self.status.description()
                )
            }
            LineMappingStatus::Moved => {
                let current = self.current_line.unwrap_or(0);
                let offset = self.offset().unwrap_or(0);
                let offset_str = if offset > 0 {
                    format!("+{offset}")
                } else {
                    format!("{offset}")
                };
                format!(
                    "{} Line {} → {} ({} {} lines)",
                    self.status.symbol(),
                    self.original_line,
                    current,
                    self.status.description(),
                    offset_str
                )
            }
            LineMappingStatus::Deleted | LineMappingStatus::NotFound => {
                format!(
                    "{} Line {} ({})",
                    self.status.symbol(),
                    self.original_line,
                    self.status.description()
                )
            }
        }
    }
}

/// Request parameters for verifying line mapping between two commits.
///
/// This struct encapsulates the parameters needed to verify whether a line
/// from one commit maps to the same location in another commit.
#[derive(Debug, Clone)]
pub struct LineMappingRequest {
    /// The source commit SHA (where the comment was made).
    pub old_sha: String,
    /// The target commit SHA (typically HEAD).
    pub new_sha: String,
    /// Path to the file.
    pub file_path: String,
    /// The line number in the old commit.
    pub line: u32,
}

impl LineMappingRequest {
    /// Creates a new line mapping request.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Const fn with String parameters is misleading since String allocation is not const"
    )]
    pub fn new(old_sha: String, new_sha: String, file_path: String, line: u32) -> Self {
        Self {
            old_sha,
            new_sha,
            file_path,
            line,
        }
    }
}

#[cfg(test)]
#[path = "commit/tests.rs"]
mod tests;
