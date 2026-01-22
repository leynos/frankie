//! Git2-based implementation of `GitOperations`.

use std::path::Path;
use std::sync::Mutex;

use chrono::{TimeZone, Utc};
use git2::Repository;

use super::GitOperations;
use super::helpers;
use crate::local::commit::{
    CommitMetadata, CommitSnapshot, LineMappingRequest, LineMappingVerification,
};
use crate::local::error::GitOperationError;
use crate::local::types::{CommitSha, RepoFilePath};

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
        let oid = helpers::parse_sha_with_repo(&repo, sha.as_str())?;
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
            let blob_oid = helpers::get_file_blob_oid(&commit, path.as_str())?;
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
        let oid = helpers::parse_sha_with_repo(&repo, sha.as_str())?;
        let commit = repo
            .find_commit(oid)
            .map_err(|_| GitOperationError::CommitNotFound {
                sha: sha.to_string(),
            })?;

        let blob_oid = helpers::get_file_blob_oid(&commit, file_path.as_str())?;
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
        let old_oid = helpers::parse_sha_with_repo(&repo, &request.old_sha)?;
        let new_oid = helpers::parse_sha_with_repo(&repo, &request.new_sha)?;

        // Early return if commits are the same
        if helpers::are_commits_same(old_oid, new_oid) {
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
        if helpers::is_file_deleted(&new_tree, &request.file_path) {
            return Ok(LineMappingVerification::deleted(request.line));
        }

        let diff = helpers::create_file_diff(&repo, &old_tree, &new_tree, &request.file_path)?;

        // If no changes to the file, line is exact match
        if helpers::has_no_changes(&diff) {
            return Ok(LineMappingVerification::exact(request.line));
        }

        let (line_offset, line_deleted) =
            helpers::compute_line_offset_from_hunks(&diff, request.line)?;

        Ok(helpers::create_line_mapping_result(
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
        let oid = helpers::parse_sha_with_repo(&repo, sha.as_str())?;
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
        helpers::parse_sha_with_repo(&repo, sha.as_str())
            .and_then(|oid| {
                repo.find_commit(oid)
                    .map_err(|_| GitOperationError::CommitNotFound {
                        sha: sha.to_string(),
                    })
            })
            .is_ok()
    }
}
