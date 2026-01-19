//! Types for representing commit snapshots and line mapping results.
//!
//! This module provides the core data types used for time-travel navigation
//! across PR history, including commit metadata, file content snapshots, and
//! line mapping verification results.

use chrono::{DateTime, Utc};

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
    /// Creates a new commit snapshot.
    #[must_use]
    pub fn new(sha: String, message: String, author: String, timestamp: DateTime<Utc>) -> Self {
        let short_sha = sha.chars().take(7).collect();
        Self {
            sha,
            short_sha,
            message,
            author,
            timestamp,
            file_content: None,
            file_path: None,
        }
    }

    /// Creates a commit snapshot with file content.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "All fields required for snapshot"
    )]
    pub fn with_file_content(
        sha: String,
        message: String,
        author: String,
        timestamp: DateTime<Utc>,
        file_path: String,
        file_content: String,
    ) -> Self {
        let short_sha = sha.chars().take(7).collect();
        Self {
            sha,
            short_sha,
            message,
            author,
            timestamp,
            file_content: Some(file_content),
            file_path: Some(file_path),
        }
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
    #[must_use]
    pub fn offset(&self) -> Option<i32> {
        self.current_line.map(|current| {
            i32::try_from(current).unwrap_or(i32::MAX)
                - i32::try_from(self.original_line).unwrap_or(0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_snapshot_new() {
        let timestamp = Utc::now();
        let snapshot = CommitSnapshot::new(
            "abc1234567890".to_owned(),
            "Fix bug in login".to_owned(),
            "Alice".to_owned(),
            timestamp,
        );

        assert_eq!(snapshot.sha(), "abc1234567890");
        assert_eq!(snapshot.short_sha(), "abc1234");
        assert_eq!(snapshot.message(), "Fix bug in login");
        assert_eq!(snapshot.author(), "Alice");
        assert_eq!(snapshot.timestamp(), &timestamp);
        assert!(snapshot.file_content().is_none());
        assert!(snapshot.file_path().is_none());
    }

    #[test]
    fn commit_snapshot_with_file() {
        let timestamp = Utc::now();
        let snapshot = CommitSnapshot::with_file_content(
            "def5678901234".to_owned(),
            "Add feature".to_owned(),
            "Bob".to_owned(),
            timestamp,
            "src/main.rs".to_owned(),
            "fn main() {}".to_owned(),
        );

        assert_eq!(snapshot.file_content(), Some("fn main() {}"));
        assert_eq!(snapshot.file_path(), Some("src/main.rs"));
    }

    #[test]
    fn line_mapping_exact() {
        let verification = LineMappingVerification::exact(42);
        assert_eq!(verification.original_line(), 42);
        assert_eq!(verification.current_line(), Some(42));
        assert_eq!(verification.status(), LineMappingStatus::Exact);
        assert_eq!(verification.offset(), Some(0));
        assert!(verification.display().contains("exact match"));
    }

    #[test]
    fn line_mapping_moved() {
        let verification = LineMappingVerification::moved(42, 50);
        assert_eq!(verification.original_line(), 42);
        assert_eq!(verification.current_line(), Some(50));
        assert_eq!(verification.status(), LineMappingStatus::Moved);
        assert_eq!(verification.offset(), Some(8));
        assert!(verification.display().contains("+8"));
    }

    #[test]
    fn line_mapping_moved_up() {
        let verification = LineMappingVerification::moved(50, 42);
        assert_eq!(verification.offset(), Some(-8));
        assert!(verification.display().contains("-8"));
    }

    #[test]
    fn line_mapping_deleted() {
        let verification = LineMappingVerification::deleted(42);
        assert_eq!(verification.original_line(), 42);
        assert!(verification.current_line().is_none());
        assert_eq!(verification.status(), LineMappingStatus::Deleted);
        assert!(verification.offset().is_none());
        assert!(verification.display().contains("deleted"));
    }

    #[test]
    fn line_mapping_not_found() {
        let verification = LineMappingVerification::not_found(42);
        assert_eq!(verification.status(), LineMappingStatus::NotFound);
        assert!(verification.display().contains("not found"));
    }

    #[test]
    fn status_symbols() {
        assert_eq!(LineMappingStatus::Exact.symbol(), "✓");
        assert_eq!(LineMappingStatus::Moved.symbol(), "→");
        assert_eq!(LineMappingStatus::Deleted.symbol(), "✗");
        assert_eq!(LineMappingStatus::NotFound.symbol(), "?");
    }
}
