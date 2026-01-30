//! Git operations for time-travel navigation.
//!
//! This module provides a trait-based abstraction for Git operations needed
//! by the time-travel feature, along with a git2-based implementation. The
//! trait enables dependency injection for testing without real repositories.

mod git2_impl;
mod helpers;

use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;

use super::commit::{CommitSnapshot, LineMappingRequest, LineMappingVerification};
use super::error::GitOperationError;
use super::types::{CommitSha, RepoFilePath};

// Re-export the Git2Operations implementation
pub use git2_impl::Git2Operations;

/// Trait defining Git operations required for time-travel navigation.
///
/// This trait enables dependency injection, allowing tests to use mock
/// implementations without requiring real Git repositories.
pub trait GitOperations: Send + Sync + Debug {
    /// Gets a snapshot of a commit including optional file content.
    ///
    /// # Arguments
    ///
    /// * `sha` - The commit SHA to retrieve.
    /// * `file_path` - Optional path to a file to include content for.
    ///
    /// # Errors
    ///
    /// Returns an error if the commit cannot be found or accessed.
    fn get_commit_snapshot(
        &self,
        sha: &CommitSha,
        file_path: Option<&RepoFilePath>,
    ) -> Result<CommitSnapshot, GitOperationError>;

    /// Gets the content of a file at a specific commit.
    ///
    /// # Arguments
    ///
    /// * `sha` - The commit SHA.
    /// * `file_path` - Path to the file within the repository.
    ///
    /// # Errors
    ///
    /// Returns an error if the commit or file cannot be found.
    fn get_file_at_commit(
        &self,
        sha: &CommitSha,
        file_path: &RepoFilePath,
    ) -> Result<String, GitOperationError>;

    /// Verifies line mapping between two commits.
    ///
    /// Determines whether a line from the old commit exists at the same
    /// position, has moved, or has been deleted in the new commit.
    ///
    /// # Arguments
    ///
    /// * `request` - The line mapping request containing old/new SHAs, file path, and line number.
    ///
    /// # Errors
    ///
    /// Returns an error if the diff cannot be computed.
    fn verify_line_mapping(
        &self,
        request: &LineMappingRequest,
    ) -> Result<LineMappingVerification, GitOperationError>;

    /// Gets parent commits of the specified commit.
    ///
    /// # Arguments
    ///
    /// * `sha` - The commit SHA to get parents for.
    /// * `limit` - Maximum number of ancestors to return.
    ///
    /// # Errors
    ///
    /// Returns an error if the commit cannot be found.
    fn get_parent_commits(
        &self,
        sha: &CommitSha,
        limit: usize,
    ) -> Result<Vec<CommitSha>, GitOperationError>;

    /// Checks if a commit exists in the repository.
    fn commit_exists(&self, sha: &CommitSha) -> bool;
}

/// Creates a shared `GitOperations` instance for use in the TUI.
///
/// # Errors
///
/// Returns an error if the repository cannot be opened at the given path.
pub fn create_git_ops(repo_path: &Path) -> Result<Arc<dyn GitOperations>, GitOperationError> {
    let ops = Git2Operations::open(repo_path)?;
    Ok(Arc::new(ops))
}

#[cfg(test)]
mod tests;
