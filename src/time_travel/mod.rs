//! Library-facing types for time-travel state and comment-derived inputs.
//!
//! This module provides the public API for working with time-travel outside
//! `crate::tui`. It exposes both parameter extraction from [`ReviewComment`]
//! metadata and the runtime state container used by renderers and hosts.
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
use crate::local::{CommitSha, GitOperationError, GitOperations, LineMappingRequest, RepoFilePath};

mod state;

pub use state::{TimeTravelInitParams, TimeTravelState};

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
    /// Creates a new `TimeTravelParams` with the given commit SHA, file path,
    /// and optional line number.
    ///
    /// This constructor is primarily intended for testing scenarios where
    /// you need to create parameters directly rather than extracting them
    /// from a [`ReviewComment`].
    #[must_use]
    pub const fn new(
        commit_sha: CommitSha,
        file_path: RepoFilePath,
        line_number: Option<u32>,
    ) -> Self {
        Self {
            commit_sha,
            file_path,
            line_number,
        }
    }

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

/// Loads the initial time-travel state for a comment.
///
/// This function provides the core loading logic for time-travel mode,
/// fetching the commit snapshot, parent commit history, and optionally
/// verifying line mappings when both a line number and HEAD SHA are provided.
///
/// The `commit_history_limit` parameter is defensively clamped to a minimum
/// of 1 to ensure at least one commit is loaded.
///
/// # Errors
///
/// Returns a [`GitOperationError`] if:
/// - The commit snapshot cannot be retrieved
/// - The parent commit history cannot be fetched
///
/// # Example
///
/// ```no_run
/// use frankie::local::{CommitSha, RepoFilePath};
/// use frankie::time_travel::{TimeTravelParams, load_time_travel_state};
/// # use frankie::local::GitOperations;
///
/// # fn example(git_ops: &dyn GitOperations) -> Result<(), Box<dyn std::error::Error>> {
/// let params = TimeTravelParams::new(
///     CommitSha::new("abc123".to_owned()),
///     RepoFilePath::new("src/main.rs".to_owned()),
///     Some(42),
/// );
///
/// let state = load_time_travel_state(git_ops, &params, None, 50)?;
/// # Ok(())
/// # }
/// ```
pub fn load_time_travel_state(
    git_ops: &dyn GitOperations,
    params: &TimeTravelParams,
    head_sha: Option<&CommitSha>,
    commit_history_limit: usize,
) -> Result<TimeTravelState, GitOperationError> {
    // Get commit snapshot with file content
    let snapshot = git_ops.get_commit_snapshot(params.commit_sha(), Some(params.file_path()))?;

    // Normalize limit to at least 1 for defensive safety
    let effective_limit = commit_history_limit.max(1);

    // Get commit history
    let commit_history = git_ops.get_parent_commits(params.commit_sha(), effective_limit)?;

    // Verify line mapping if we have a line number and HEAD
    let line_mapping = verify_line_mapping(git_ops, params, head_sha);

    Ok(TimeTravelState::new(TimeTravelInitParams {
        snapshot,
        file_path: params.file_path().clone(),
        original_line: params.line_number(),
        line_mapping,
        commit_history,
        current_index: 0,
    }))
}

/// Verifies line mapping between the comment's commit and HEAD.
///
/// Returns `None` if either the line number or HEAD SHA is missing,
/// or if the verification fails.
fn verify_line_mapping(
    git_ops: &dyn GitOperations,
    params: &TimeTravelParams,
    head_sha: Option<&CommitSha>,
) -> Option<crate::local::LineMappingVerification> {
    let (line, head) = params.line_number().zip(head_sha)?;
    let request = LineMappingRequest::new(
        params.commit_sha().as_str().to_owned(),
        head.as_str().to_owned(),
        params.file_path().as_str().to_owned(),
        line,
    );
    git_ops.verify_line_mapping(&request).ok()
}

#[cfg(test)]
mod tests;
