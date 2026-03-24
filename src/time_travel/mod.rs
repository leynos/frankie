//! Library-facing types for deriving time-travel inputs from review comments.
//!
//! This module provides the public API for extracting time-travel parameters
//! from [`ReviewComment`] metadata. It allows callers outside `crate::tui` to
//! derive the commit SHA, file path, and optional line number needed to
//! initiate a time-travel view.
//!
//! # Example
//!
//! ```
//! use frankie::ReviewComment;
//! use frankie::time_travel::TimeTravelParams;
//!
//! let comment = ReviewComment {
//!     commit_sha: Some("abc123".to_owned()),
//!     file_path: Some("src/main.rs".to_owned()),
//!     line_number: Some(42),
//!     ..ReviewComment::default()
//! };
//!
//! let params = TimeTravelParams::from_comment(&comment)
//!     .expect("comment has required metadata");
//! assert_eq!(params.commit_sha().as_str(), "abc123");
//! assert_eq!(params.file_path().as_str(), "src/main.rs");
//! assert_eq!(params.line_number(), Some(42));
//! ```

use thiserror::Error;

use crate::github::models::ReviewComment;
use crate::local::{CommitSha, RepoFilePath};

/// Typed failures for time-travel parameter extraction.
///
/// Each variant identifies a specific piece of metadata that was missing
/// from the [`ReviewComment`], enabling callers to produce targeted error
/// messages rather than an undifferentiated failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TimeTravelParamsError {
    /// The review comment does not carry a commit SHA.
    #[error("review comment is missing a commit SHA")]
    MissingCommitSha,
    /// The review comment does not carry a file path.
    #[error("review comment is missing a file path")]
    MissingFilePath,
}

/// Parameters for initiating a time-travel view from a review comment.
///
/// Captures the commit SHA, file path, and optional line number needed to
/// load a historical snapshot. Use [`TimeTravelParams::from_comment`] to
/// extract these values from a [`ReviewComment`].
///
/// The line-number rule follows the existing convention: prefer
/// `line_number` when present, otherwise fall back to
/// `original_line_number`. A missing line number is not an error because
/// time travel can still load a file snapshot without line mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeTravelParams {
    /// The commit SHA where the comment was made.
    commit_sha: CommitSha,
    /// Path to the file within the repository.
    file_path: RepoFilePath,
    /// Line number in the file, if available.
    line_number: Option<u32>,
}

impl TimeTravelParams {
    /// Extracts time-travel parameters from a review comment.
    ///
    /// Returns a typed error identifying which required field is missing.
    /// When both `commit_sha` and `file_path` are absent, the commit SHA
    /// error takes precedence because it is checked first.
    ///
    /// # Errors
    ///
    /// Returns [`TimeTravelParamsError::MissingCommitSha`] when the comment
    /// has no commit SHA, or [`TimeTravelParamsError::MissingFilePath`] when
    /// the comment has no file path.
    ///
    /// # Example
    ///
    /// ```
    /// use frankie::ReviewComment;
    /// use frankie::time_travel::{TimeTravelParams, TimeTravelParamsError};
    ///
    /// let missing_sha = ReviewComment {
    ///     file_path: Some("src/lib.rs".to_owned()),
    ///     ..ReviewComment::default()
    /// };
    /// assert_eq!(
    ///     TimeTravelParams::from_comment(&missing_sha).unwrap_err(),
    ///     TimeTravelParamsError::MissingCommitSha,
    /// );
    /// ```
    pub fn from_comment(comment: &ReviewComment) -> Result<Self, TimeTravelParamsError> {
        let commit_sha = comment
            .commit_sha
            .as_ref()
            .ok_or(TimeTravelParamsError::MissingCommitSha)?;
        let file_path = comment
            .file_path
            .as_ref()
            .ok_or(TimeTravelParamsError::MissingFilePath)?;

        Ok(Self {
            commit_sha: CommitSha::new(commit_sha.clone()),
            file_path: RepoFilePath::new(file_path.clone()),
            line_number: comment.line_number.or(comment.original_line_number),
        })
    }

    /// Returns the commit SHA.
    #[must_use]
    pub const fn commit_sha(&self) -> &CommitSha {
        &self.commit_sha
    }

    /// Returns the file path.
    #[must_use]
    pub const fn file_path(&self) -> &RepoFilePath {
        &self.file_path
    }

    /// Returns the line number, if available.
    #[must_use]
    pub const fn line_number(&self) -> Option<u32> {
        self.line_number
    }
}

#[cfg(test)]
mod tests;
