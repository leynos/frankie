//! Git operations for time-travel navigation.
//!
//! This module provides a trait-based abstraction for Git operations needed
//! by the time-travel feature, along with a git2-based implementation. The
//! trait enables dependency injection for testing without real repositories.

use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::{TimeZone, Utc};
use git2::{DiffOptions, Oid, Repository};

use super::commit::{CommitMetadata, CommitSnapshot, LineMappingRequest, LineMappingVerification};
use super::error::GitOperationError;
use super::types::{CommitSha, RepoFilePath};

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

/// Git2-based implementation of `GitOperations`.
///
/// Uses a `Mutex` to wrap the `Repository` because `git2::Repository` is not
/// `Sync`. This allows the implementation to be used in async contexts.
pub struct Git2Operations {
    repo: Mutex<Repository>,
}

impl std::fmt::Debug for Git2Operations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Git2Operations")
            .field("repo", &"<git2::Repository>")
            .finish()
    }
}

impl Git2Operations {
    /// Helper to construct `Git2Operations` from a `Repository` result.
    fn from_repo_result(
        result: Result<Repository, git2::Error>,
    ) -> Result<Self, GitOperationError> {
        let repo = result.map_err(|e| GitOperationError::RepositoryNotAvailable {
            message: e.message().to_owned(),
        })?;
        Ok(Self {
            repo: Mutex::new(repo),
        })
    }

    /// Opens a repository at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not a valid Git repository.
    pub fn open(repo_path: &Path) -> Result<Self, GitOperationError> {
        Self::from_repo_result(Repository::open(repo_path))
    }

    /// Discovers and opens a repository containing the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if no Git repository is found.
    pub fn discover(start_path: &Path) -> Result<Self, GitOperationError> {
        Self::from_repo_result(Repository::discover(start_path))
    }

    /// Creates a new instance wrapping an existing repository.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Mutex::new is not const-stable"
    )]
    pub fn from_repository(repo: Repository) -> Self {
        Self {
            repo: Mutex::new(repo),
        }
    }

    /// Parses a SHA string into an Oid using the locked repository.
    fn parse_sha_with_repo(repo: &Repository, sha: &str) -> Result<Oid, GitOperationError> {
        // Try to parse as full SHA first
        if let Ok(oid) = Oid::from_str(sha) {
            return Ok(oid);
        }

        // Try as a short SHA or ref
        let obj = repo
            .revparse_single(sha)
            .map_err(|_| GitOperationError::CommitNotFound {
                sha: sha.to_owned(),
            })?;

        Ok(obj.id())
    }

    /// Gets the blob OID for a file at a specific commit.
    fn get_file_blob_oid(
        commit: &git2::Commit<'_>,
        file_path: &str,
    ) -> Result<Oid, GitOperationError> {
        let tree = commit.tree()?;
        let entry =
            tree.get_path(Path::new(file_path))
                .map_err(|_| GitOperationError::FileNotFound {
                    path: file_path.to_owned(),
                    sha: commit.id().to_string(),
                })?;
        Ok(entry.id())
    }

    /// Checks if a file was deleted in a tree.
    fn is_file_deleted(new_tree: &git2::Tree<'_>, file_path: &str) -> bool {
        new_tree.get_path(Path::new(file_path)).is_err()
    }

    /// Checks if two commit OIDs are the same.
    fn are_commits_same(old_oid: Oid, new_oid: Oid) -> bool {
        old_oid == new_oid
    }

    /// Creates a diff for a specific file between two trees.
    fn create_file_diff<'a>(
        repo: &'a Repository,
        old_tree: &git2::Tree<'_>,
        new_tree: &git2::Tree<'_>,
        file_path: &str,
    ) -> Result<git2::Diff<'a>, GitOperationError> {
        let mut diff_opts = DiffOptions::new();
        diff_opts.pathspec(file_path);

        repo.diff_tree_to_tree(Some(old_tree), Some(new_tree), Some(&mut diff_opts))
            .map_err(|e| GitOperationError::DiffComputationFailed {
                message: e.message().to_owned(),
            })
    }

    /// Checks if a diff has no changes.
    fn has_no_changes(diff: &git2::Diff<'_>) -> bool {
        diff.deltas().next().is_none()
    }

    /// Checks if a line is within a hunk's old range.
    const fn is_line_in_hunk(line: u32, old_start: u32, old_lines: u32) -> bool {
        line >= old_start && line < old_start + old_lines
    }

    /// Checks if a line was deleted in a hunk.
    const fn is_line_deleted_in_hunk(
        line: u32,
        old_start: u32,
        old_lines: u32,
        new_lines: u32,
    ) -> bool {
        if old_lines > new_lines {
            let removed_start = old_start + new_lines;
            line >= removed_start
        } else {
            false
        }
    }

    /// Calculates the offset contribution from a hunk.
    fn calculate_hunk_offset(old_lines: u32, new_lines: u32) -> i32 {
        i32::try_from(new_lines).unwrap_or(0) - i32::try_from(old_lines).unwrap_or(0)
    }

    /// Computes the line offset by processing diff hunks.
    fn compute_line_offset_from_hunks(
        diff: &git2::Diff<'_>,
        target_line: u32,
    ) -> Result<(i32, bool), GitOperationError> {
        let mut line_offset: i32 = 0;
        let mut line_deleted = false;
        let mut passed_line = false;

        diff.foreach(
            &mut |_, _| true,
            None,
            Some(&mut |_delta, hunk| {
                let old_start = hunk.old_start();
                let old_lines = hunk.old_lines();
                let new_lines = hunk.new_lines();

                if passed_line {
                    return true;
                }

                if Self::is_line_in_hunk(target_line, old_start, old_lines) {
                    line_deleted =
                        Self::is_line_deleted_in_hunk(target_line, old_start, old_lines, new_lines);
                    passed_line = true;
                } else if target_line >= old_start + old_lines {
                    line_offset += Self::calculate_hunk_offset(old_lines, new_lines);
                } else {
                    passed_line = true;
                }

                true
            }),
            None,
        )
        .map_err(|e| GitOperationError::DiffComputationFailed {
            message: e.message().to_owned(),
        })?;

        Ok((line_offset, line_deleted))
    }

    /// Creates the appropriate line mapping result from offset and deletion state.
    fn create_line_mapping_result(
        original_line: u32,
        line_offset: i32,
        line_deleted: bool,
    ) -> LineMappingVerification {
        if line_deleted {
            return LineMappingVerification::deleted(original_line);
        }

        let new_line = u32::try_from(i32::try_from(original_line).unwrap_or(0) + line_offset)
            .unwrap_or(original_line);

        if new_line == original_line {
            LineMappingVerification::exact(original_line)
        } else {
            LineMappingVerification::moved(original_line, new_line)
        }
    }
}

impl GitOperations for Git2Operations {
    fn get_commit_snapshot(
        &self,
        sha: &CommitSha,
        file_path: Option<&RepoFilePath>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        #[expect(
            clippy::expect_used,
            reason = "Mutex poisoning is an unrecoverable error"
        )]
        let repo = self.repo.lock().expect("Git repository mutex poisoned");
        let oid = Self::parse_sha_with_repo(&repo, sha.as_str())?;
        let commit = repo
            .find_commit(oid)
            .map_err(|_| GitOperationError::CommitNotFound {
                sha: sha.to_string(),
            })?;

        let message = commit
            .message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_owned();

        let author = commit.author();
        let author_name = author.name().unwrap_or("Unknown").to_owned();

        let timestamp = Utc
            .timestamp_opt(commit.time().seconds(), 0)
            .single()
            .unwrap_or_else(Utc::now);

        let metadata = CommitMetadata::new(oid.to_string(), message, author_name, timestamp);

        if let Some(path) = file_path {
            let blob_oid = Self::get_file_blob_oid(&commit, path.as_str())?;
            let blob =
                repo.find_blob(blob_oid)
                    .map_err(|e| GitOperationError::CommitAccessFailed {
                        sha: sha.to_string(),
                        message: e.message().to_owned(),
                    })?;

            let content = std::str::from_utf8(blob.content())
                .map_err(|_| GitOperationError::CommitAccessFailed {
                    sha: sha.to_string(),
                    message: "file content is not valid UTF-8".to_owned(),
                })?
                .to_owned();

            Ok(CommitSnapshot::with_file_content(
                metadata,
                path.to_string(),
                content,
            ))
        } else {
            Ok(CommitSnapshot::new(metadata))
        }
    }

    fn get_file_at_commit(
        &self,
        sha: &CommitSha,
        file_path: &RepoFilePath,
    ) -> Result<String, GitOperationError> {
        #[expect(
            clippy::expect_used,
            reason = "Mutex poisoning is an unrecoverable error"
        )]
        let repo = self.repo.lock().expect("Git repository mutex poisoned");
        let oid = Self::parse_sha_with_repo(&repo, sha.as_str())?;
        let commit = repo
            .find_commit(oid)
            .map_err(|_| GitOperationError::CommitNotFound {
                sha: sha.to_string(),
            })?;

        let blob_oid = Self::get_file_blob_oid(&commit, file_path.as_str())?;
        let blob = repo
            .find_blob(blob_oid)
            .map_err(|e| GitOperationError::CommitAccessFailed {
                sha: sha.to_string(),
                message: e.message().to_owned(),
            })?;

        let content = std::str::from_utf8(blob.content()).map_err(|_| {
            GitOperationError::CommitAccessFailed {
                sha: sha.to_string(),
                message: "file content is not valid UTF-8".to_owned(),
            }
        })?;

        Ok(content.to_owned())
    }

    fn verify_line_mapping(
        &self,
        request: &LineMappingRequest,
    ) -> Result<LineMappingVerification, GitOperationError> {
        #[expect(
            clippy::expect_used,
            reason = "Mutex poisoning is an unrecoverable error"
        )]
        let repo = self.repo.lock().expect("Git repository mutex poisoned");
        let old_oid = Self::parse_sha_with_repo(&repo, &request.old_sha)?;
        let new_oid = Self::parse_sha_with_repo(&repo, &request.new_sha)?;

        // Early return if commits are the same
        if Self::are_commits_same(old_oid, new_oid) {
            return Ok(LineMappingVerification::exact(request.line));
        }

        let old_commit =
            repo.find_commit(old_oid)
                .map_err(|_| GitOperationError::CommitNotFound {
                    sha: request.old_sha.clone(),
                })?;
        let new_commit =
            repo.find_commit(new_oid)
                .map_err(|_| GitOperationError::CommitNotFound {
                    sha: request.new_sha.clone(),
                })?;

        let old_tree = old_commit.tree()?;
        let new_tree = new_commit.tree()?;

        // Check if the file was deleted in the new commit
        if Self::is_file_deleted(&new_tree, &request.file_path) {
            return Ok(LineMappingVerification::deleted(request.line));
        }

        let diff = Self::create_file_diff(&repo, &old_tree, &new_tree, &request.file_path)?;

        // If no changes to the file, line is exact match
        if Self::has_no_changes(&diff) {
            return Ok(LineMappingVerification::exact(request.line));
        }

        let (line_offset, line_deleted) =
            Self::compute_line_offset_from_hunks(&diff, request.line)?;

        Ok(Self::create_line_mapping_result(
            request.line,
            line_offset,
            line_deleted,
        ))
    }

    fn get_parent_commits(
        &self,
        sha: &CommitSha,
        limit: usize,
    ) -> Result<Vec<CommitSha>, GitOperationError> {
        #[expect(
            clippy::expect_used,
            reason = "Mutex poisoning is an unrecoverable error"
        )]
        let repo = self.repo.lock().expect("Git repository mutex poisoned");
        let oid = Self::parse_sha_with_repo(&repo, sha.as_str())?;
        let mut revwalk = repo.revwalk()?;
        revwalk.push(oid)?;
        // Topological sorting ensures parents come after children in the commit graph,
        // which is more appropriate for history traversal than pure chronological order.
        // TIME adds a secondary sort by commit timestamp for commits at the same depth.
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

        let commits: Vec<CommitSha> = revwalk
            .filter_map(Result::ok)
            .take(limit)
            .map(|commit_oid| CommitSha::new(commit_oid.to_string()))
            .collect();

        Ok(commits)
    }

    fn commit_exists(&self, sha: &CommitSha) -> bool {
        #[expect(
            clippy::expect_used,
            reason = "Mutex poisoning is an unrecoverable error"
        )]
        let repo = self.repo.lock().expect("Git repository mutex poisoned");
        Self::parse_sha_with_repo(&repo, sha.as_str())
            .and_then(|oid| {
                repo.find_commit(oid)
                    .map_err(|_| GitOperationError::CommitNotFound {
                        sha: sha.to_string(),
                    })
            })
            .is_ok()
    }
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
#[path = "git_ops/tests.rs"]
mod tests;
