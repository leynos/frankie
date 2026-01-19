//! Git operations for time-travel navigation.
//!
//! This module provides a trait-based abstraction for Git operations needed
//! by the time-travel feature, along with a git2-based implementation. The
//! trait enables dependency injection for testing without real repositories.

// Mutex unwrap is acceptable - a poisoned mutex indicates a bug
#![expect(clippy::unwrap_used, reason = "Mutex poisoning is a fatal error")]
// Shadow warnings for local iterator vars
#![expect(clippy::shadow_unrelated, reason = "Iterator variable reuse in map")]

use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::{TimeZone, Utc};
use git2::{DiffOptions, Oid, Repository};

use super::commit::{CommitSnapshot, LineMappingVerification};
use super::error::GitOperationError;

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
        sha: &str,
        file_path: Option<&str>,
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
    fn get_file_at_commit(&self, sha: &str, file_path: &str) -> Result<String, GitOperationError>;

    /// Verifies line mapping between two commits.
    ///
    /// Determines whether a line from the old commit exists at the same
    /// position, has moved, or has been deleted in the new commit.
    ///
    /// # Arguments
    ///
    /// * `old_sha` - The source commit SHA (where the comment was made).
    /// * `new_sha` - The target commit SHA (typically HEAD).
    /// * `file_path` - Path to the file.
    /// * `line` - The line number in the old commit.
    ///
    /// # Errors
    ///
    /// Returns an error if the diff cannot be computed.
    #[expect(
        clippy::too_many_arguments,
        reason = "All parameters needed for line mapping verification"
    )]
    fn verify_line_mapping(
        &self,
        old_sha: &str,
        new_sha: &str,
        file_path: &str,
        line: u32,
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
    fn get_parent_commits(&self, sha: &str, limit: usize)
    -> Result<Vec<String>, GitOperationError>;

    /// Checks if a commit exists in the repository.
    fn commit_exists(&self, sha: &str) -> bool;
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
    /// Opens a repository at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not a valid Git repository.
    pub fn open(repo_path: &Path) -> Result<Self, GitOperationError> {
        let repo =
            Repository::open(repo_path).map_err(|e| GitOperationError::RepositoryNotAvailable {
                message: e.message().to_owned(),
            })?;
        Ok(Self {
            repo: Mutex::new(repo),
        })
    }

    /// Discovers and opens a repository containing the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if no Git repository is found.
    pub fn discover(start_path: &Path) -> Result<Self, GitOperationError> {
        let repo = Repository::discover(start_path).map_err(|e| {
            GitOperationError::RepositoryNotAvailable {
                message: e.message().to_owned(),
            }
        })?;
        Ok(Self {
            repo: Mutex::new(repo),
        })
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
}

impl GitOperations for Git2Operations {
    fn get_commit_snapshot(
        &self,
        sha: &str,
        file_path: Option<&str>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        let repo = self.repo.lock().unwrap();
        let oid = Self::parse_sha_with_repo(&repo, sha)?;
        let commit = repo
            .find_commit(oid)
            .map_err(|_| GitOperationError::CommitNotFound {
                sha: sha.to_owned(),
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

        if let Some(path) = file_path {
            let blob_oid = Self::get_file_blob_oid(&commit, path)?;
            let blob =
                repo.find_blob(blob_oid)
                    .map_err(|e| GitOperationError::CommitAccessFailed {
                        sha: sha.to_owned(),
                        message: e.message().to_owned(),
                    })?;

            let content = std::str::from_utf8(blob.content())
                .map_err(|_| GitOperationError::CommitAccessFailed {
                    sha: sha.to_owned(),
                    message: "file content is not valid UTF-8".to_owned(),
                })?
                .to_owned();

            Ok(CommitSnapshot::with_file_content(
                oid.to_string(),
                message,
                author_name,
                timestamp,
                path.to_owned(),
                content,
            ))
        } else {
            Ok(CommitSnapshot::new(
                oid.to_string(),
                message,
                author_name,
                timestamp,
            ))
        }
    }

    fn get_file_at_commit(&self, sha: &str, file_path: &str) -> Result<String, GitOperationError> {
        let repo = self.repo.lock().unwrap();
        let oid = Self::parse_sha_with_repo(&repo, sha)?;
        let commit = repo
            .find_commit(oid)
            .map_err(|_| GitOperationError::CommitNotFound {
                sha: sha.to_owned(),
            })?;

        let blob_oid = Self::get_file_blob_oid(&commit, file_path)?;
        let blob = repo
            .find_blob(blob_oid)
            .map_err(|e| GitOperationError::CommitAccessFailed {
                sha: sha.to_owned(),
                message: e.message().to_owned(),
            })?;

        let content = std::str::from_utf8(blob.content()).map_err(|_| {
            GitOperationError::CommitAccessFailed {
                sha: sha.to_owned(),
                message: "file content is not valid UTF-8".to_owned(),
            }
        })?;

        Ok(content.to_owned())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Diff traversal logic is inherently complex"
    )]
    #[expect(
        clippy::excessive_nesting,
        reason = "Git diff callback requires nested conditions"
    )]
    fn verify_line_mapping(
        &self,
        old_sha: &str,
        new_sha: &str,
        file_path: &str,
        line: u32,
    ) -> Result<LineMappingVerification, GitOperationError> {
        let repo = self.repo.lock().unwrap();
        let old_oid = Self::parse_sha_with_repo(&repo, old_sha)?;
        let new_oid = Self::parse_sha_with_repo(&repo, new_sha)?;

        let old_commit =
            repo.find_commit(old_oid)
                .map_err(|_| GitOperationError::CommitNotFound {
                    sha: old_sha.to_owned(),
                })?;
        let new_commit =
            repo.find_commit(new_oid)
                .map_err(|_| GitOperationError::CommitNotFound {
                    sha: new_sha.to_owned(),
                })?;

        let old_tree = old_commit.tree()?;
        let new_tree = new_commit.tree()?;

        // Check if the file exists in the new commit
        if new_tree.get_path(Path::new(file_path)).is_err() {
            return Ok(LineMappingVerification::deleted(line));
        }

        // If commits are the same, line is exact match
        if old_oid == new_oid {
            return Ok(LineMappingVerification::exact(line));
        }

        let mut diff_opts = DiffOptions::new();
        diff_opts.pathspec(file_path);

        let diff = repo
            .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut diff_opts))
            .map_err(|e| GitOperationError::DiffComputationFailed {
                message: e.message().to_owned(),
            })?;

        // If no changes to the file, line is exact match
        if diff.deltas().len() == 0 {
            return Ok(LineMappingVerification::exact(line));
        }

        // Compute line offset from hunks
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

                // If we've already passed the target line, don't process
                if passed_line {
                    return true;
                }

                // Check if the target line is within this hunk's old range
                if line >= old_start && line < old_start + old_lines {
                    // Line is in a changed region - could be deleted or modified
                    // For simplicity, we'll mark it as deleted if old_lines > new_lines
                    // and the line is in the removed portion
                    if old_lines > new_lines {
                        let removed_start = old_start + new_lines;
                        if line >= removed_start {
                            line_deleted = true;
                        }
                    }
                    passed_line = true;
                } else if line >= old_start + old_lines {
                    // Line is after this hunk, accumulate offset
                    line_offset += i32::try_from(new_lines).unwrap_or(0)
                        - i32::try_from(old_lines).unwrap_or(0);
                } else {
                    // Line is before this hunk, we're done
                    passed_line = true;
                }

                true
            }),
            None,
        )
        .map_err(|e| GitOperationError::DiffComputationFailed {
            message: e.message().to_owned(),
        })?;

        if line_deleted {
            return Ok(LineMappingVerification::deleted(line));
        }

        let new_line =
            u32::try_from(i32::try_from(line).unwrap_or(0) + line_offset).unwrap_or(line);

        if new_line == line {
            Ok(LineMappingVerification::exact(line))
        } else {
            Ok(LineMappingVerification::moved(line, new_line))
        }
    }

    fn get_parent_commits(
        &self,
        sha: &str,
        limit: usize,
    ) -> Result<Vec<String>, GitOperationError> {
        let repo = self.repo.lock().unwrap();
        let oid = Self::parse_sha_with_repo(&repo, sha)?;
        let mut revwalk = repo.revwalk()?;
        revwalk.push(oid)?;
        // Topological sorting ensures parents come after children in the commit graph,
        // which is more appropriate for history traversal than pure chronological order.
        // TIME adds a secondary sort by commit timestamp for commits at the same depth.
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

        let commits: Vec<String> = revwalk
            .filter_map(Result::ok)
            .take(limit)
            .map(|oid| oid.to_string())
            .collect();

        Ok(commits)
    }

    fn commit_exists(&self, sha: &str) -> bool {
        let repo = self.repo.lock().unwrap();
        Self::parse_sha_with_repo(&repo, sha)
            .and_then(|oid| {
                repo.find_commit(oid)
                    .map_err(|_| GitOperationError::CommitNotFound {
                        sha: sha.to_owned(),
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
#[expect(clippy::indexing_slicing, reason = "Test assertions")]
#[expect(clippy::unwrap_used, reason = "Tests panic on failure")]
mod tests {
    use super::*;
    use crate::local::LineMappingStatus;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Configure user for commits
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        (dir, repo)
    }

    fn create_commit(repo: &Repository, message: &str, files: &[(&str, &str)]) -> Oid {
        let sig = repo.signature().unwrap();
        let mut index = repo.index().unwrap();

        for (path, content) in files {
            let full_path = repo.workdir().unwrap().join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&full_path, content).unwrap();
            index.add_path(Path::new(path)).unwrap();
        }

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

        let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();

        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap()
    }

    #[test]
    fn test_commit_snapshot() {
        let (dir, repo) = create_test_repo();
        let oid = create_commit(&repo, "Initial commit", &[("test.txt", "hello")]);

        let ops = Git2Operations::from_repository(repo);
        let snapshot = ops.get_commit_snapshot(&oid.to_string(), None).unwrap();

        assert_eq!(snapshot.message(), "Initial commit");
        assert_eq!(snapshot.author(), "Test User");
        assert!(snapshot.file_content().is_none());

        drop(dir);
    }

    #[test]
    fn test_commit_snapshot_with_file() {
        let (dir, repo) = create_test_repo();
        let oid = create_commit(&repo, "Add file", &[("src/main.rs", "fn main() {}")]);

        let ops = Git2Operations::from_repository(repo);
        let snapshot = ops
            .get_commit_snapshot(&oid.to_string(), Some("src/main.rs"))
            .unwrap();

        assert_eq!(snapshot.file_content(), Some("fn main() {}"));
        assert_eq!(snapshot.file_path(), Some("src/main.rs"));

        drop(dir);
    }

    #[test]
    fn test_get_file_at_commit() {
        let (dir, repo) = create_test_repo();
        let oid = create_commit(&repo, "Add file", &[("test.txt", "content here")]);

        let ops = Git2Operations::from_repository(repo);
        let content = ops
            .get_file_at_commit(&oid.to_string(), "test.txt")
            .unwrap();

        assert_eq!(content, "content here");

        drop(dir);
    }

    #[test]
    fn test_file_not_found() {
        let (dir, repo) = create_test_repo();
        let oid = create_commit(&repo, "Add file", &[("test.txt", "content")]);

        let ops = Git2Operations::from_repository(repo);
        let result = ops.get_file_at_commit(&oid.to_string(), "nonexistent.txt");

        assert!(matches!(
            result,
            Err(GitOperationError::FileNotFound { .. })
        ));

        drop(dir);
    }

    #[test]
    fn test_commit_not_found() {
        let (dir, repo) = create_test_repo();
        create_commit(&repo, "Initial", &[("test.txt", "content")]);

        let ops = Git2Operations::from_repository(repo);
        let result = ops.get_commit_snapshot("0000000000000000000000000000000000000000", None);

        assert!(matches!(
            result,
            Err(GitOperationError::CommitNotFound { .. })
        ));

        drop(dir);
    }

    #[test]
    fn test_commit_exists() {
        let (dir, repo) = create_test_repo();
        let oid = create_commit(&repo, "Initial", &[("test.txt", "content")]);

        let ops = Git2Operations::from_repository(repo);

        assert!(ops.commit_exists(&oid.to_string()));
        assert!(!ops.commit_exists("0000000000000000000000000000000000000000"));

        drop(dir);
    }

    #[test]
    fn test_get_parent_commits() {
        let (dir, repo) = create_test_repo();
        let oid1 = create_commit(&repo, "First", &[("test.txt", "v1")]);
        let oid2 = create_commit(&repo, "Second", &[("test.txt", "v2")]);
        let oid3 = create_commit(&repo, "Third", &[("test.txt", "v3")]);

        let ops = Git2Operations::from_repository(repo);
        let commits = ops.get_parent_commits(&oid3.to_string(), 10).unwrap();

        assert_eq!(commits.len(), 3);
        assert_eq!(commits[0], oid3.to_string());
        assert_eq!(commits[1], oid2.to_string());
        assert_eq!(commits[2], oid1.to_string());

        drop(dir);
    }

    #[test]
    fn test_line_mapping_exact() {
        let (dir, repo) = create_test_repo();
        let oid = create_commit(&repo, "Add file", &[("test.txt", "line1\nline2\nline3")]);

        let ops = Git2Operations::from_repository(repo);
        let verification = ops
            .verify_line_mapping(&oid.to_string(), &oid.to_string(), "test.txt", 2)
            .unwrap();

        assert_eq!(verification.status(), LineMappingStatus::Exact);
        assert_eq!(verification.original_line(), 2);
        assert_eq!(verification.current_line(), Some(2));

        drop(dir);
    }

    #[test]
    fn test_line_mapping_no_change() {
        let (dir, repo) = create_test_repo();
        let oid1 = create_commit(&repo, "Add file", &[("test.txt", "line1\nline2\nline3")]);
        let oid2 = create_commit(&repo, "Other file", &[("other.txt", "other content")]);

        let ops = Git2Operations::from_repository(repo);
        let verification = ops
            .verify_line_mapping(&oid1.to_string(), &oid2.to_string(), "test.txt", 2)
            .unwrap();

        assert_eq!(verification.status(), LineMappingStatus::Exact);

        drop(dir);
    }
}
