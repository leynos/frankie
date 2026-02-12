//! Integration tests for Git operations.
//!
//! These tests use real Git repositories created via `tempfile` to verify
//! commit snapshot retrieval, file content access, and line mapping logic.

#![expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions are expected to panic on failure"
)]

use camino::Utf8Path;
use cap_std::fs_utf8 as fs;

use git2::{ErrorCode, Oid, Repository};
use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::*;
use crate::local::LineMappingStatus;
use crate::local::commit::LineMappingRequest;
use crate::local::error::GitOperationError;
use crate::local::types::{CommitSha, RepoFilePath};

/// Error type for test fixtures and helpers.
type TestError = Box<dyn std::error::Error>;

#[fixture]
fn test_repo() -> Result<(TempDir, Repository), TestError> {
    let dir = TempDir::new()?;
    let repo = Repository::init(dir.path())?;

    // Configure user for commits
    let mut config = repo.config()?;
    config.set_str("user.name", "Test User")?;
    config.set_str("user.email", "test@example.com")?;

    Ok((dir, repo))
}

fn create_commit(
    repo: &Repository,
    message: &str,
    files: &[(&str, &str)],
) -> Result<Oid, TestError> {
    let sig = repo.signature()?;
    let mut index = repo.index()?;

    let workdir = repo
        .workdir()
        .ok_or("repository has no working directory")?;
    let workdir_str = workdir.to_str().ok_or("workdir path is not valid UTF-8")?;
    let dir = fs::Dir::open_ambient_dir(workdir_str, cap_std::ambient_authority())?;

    for (path, content) in files {
        let utf8_path = Utf8Path::new(path);
        if let Some(parent) = utf8_path.parent()
            && !parent.as_str().is_empty()
        {
            dir.create_dir_all(parent)?;
        }
        dir.write(utf8_path, content)?;
        index.add_path(utf8_path.as_std_path())?;
    }

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    // Determine the parent commit, treating UnbornBranch as "no parent"
    // while propagating other errors upward.
    let parent: Option<git2::Commit<'_>> = match repo.head() {
        Ok(head_ref) => Some(head_ref.peel_to_commit()?),
        Err(e) if e.code() == ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();

    Ok(repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?)
}

/// Helper to test Git operation error handling with custom setup.
fn test_git_error<F, T, S>(
    test_repo: Result<(TempDir, Repository), TestError>,
    setup: S,
    operation: F,
) -> Result<GitOperationError, TestError>
where
    F: FnOnce(&Git2Operations, &str) -> Result<T, GitOperationError>,
    S: FnOnce(&Repository) -> Result<String, TestError>,
    T: std::fmt::Debug,
{
    let (dir, repo) = test_repo?;
    let param = setup(&repo)?;
    let ops = Git2Operations::from_repository(repo);

    let result = operation(&ops, &param);

    let Err(err) = result else {
        drop(dir);
        return Err("operation must fail but succeeded".into());
    };
    drop(dir);
    Ok(err)
}

#[rstest]
fn test_commit_snapshot(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let oid = create_commit(&repo, "Initial commit", &[("test.txt", "hello")])?;

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid.to_string());
    let snapshot = ops.get_commit_snapshot(&sha, None)?;

    assert_eq!(snapshot.message(), "Initial commit");
    assert_eq!(snapshot.author(), "Test User");
    assert!(snapshot.file_content().is_none());

    drop(dir);
    Ok(())
}

#[rstest]
fn test_commit_snapshot_with_file(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let oid = create_commit(&repo, "Add file", &[("src/main.rs", "fn main() {}")])?;

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid.to_string());
    let path = RepoFilePath::new("src/main.rs".to_owned());
    let snapshot = ops.get_commit_snapshot(&sha, Some(&path))?;

    assert_eq!(snapshot.file_content(), Some("fn main() {}"));
    assert_eq!(snapshot.file_path(), Some("src/main.rs"));

    drop(dir);
    Ok(())
}

#[rstest]
fn test_get_file_at_commit(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let oid = create_commit(&repo, "Add file", &[("test.txt", "content here")])?;

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid.to_string());
    let path = RepoFilePath::new("test.txt".to_owned());
    let content = ops.get_file_at_commit(&sha, &path)?;

    assert_eq!(content, "content here");

    drop(dir);
    Ok(())
}

#[rstest]
fn test_file_not_found(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let err = test_git_error(
        test_repo,
        |repo| {
            let oid = create_commit(repo, "Add file", &[("test.txt", "content")])?;
            Ok(oid.to_string())
        },
        |ops, sha| {
            let commit_sha = CommitSha::new(sha.to_owned());
            let path = RepoFilePath::new("nonexistent.txt".to_owned());
            ops.get_file_at_commit(&commit_sha, &path)
        },
    )?;

    assert!(matches!(err, GitOperationError::FileNotFound { .. }));
    Ok(())
}

#[rstest]
fn test_commit_not_found(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let err = test_git_error(
        test_repo,
        |repo| {
            create_commit(repo, "Initial", &[("test.txt", "content")])?;
            Ok("0000000000000000000000000000000000000000".to_owned())
        },
        |ops, sha| {
            let commit_sha = CommitSha::new(sha.to_owned());
            ops.get_commit_snapshot(&commit_sha, None)
        },
    )?;

    assert!(matches!(err, GitOperationError::CommitNotFound { .. }));
    Ok(())
}

#[rstest]
fn test_commit_exists(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let oid = create_commit(&repo, "Initial", &[("test.txt", "content")])?;

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid.to_string());
    let nonexistent = CommitSha::new("0000000000000000000000000000000000000000".to_owned());

    assert!(ops.commit_exists(&sha));
    assert!(!ops.commit_exists(&nonexistent));

    drop(dir);
    Ok(())
}

#[rstest]
fn test_get_parent_commits(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let oid1 = create_commit(&repo, "First", &[("test.txt", "v1")])?;
    let oid2 = create_commit(&repo, "Second", &[("test.txt", "v2")])?;
    let oid3 = create_commit(&repo, "Third", &[("test.txt", "v3")])?;

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid3.to_string());
    let commits = ops.get_parent_commits(&sha, 10)?;

    assert_eq!(commits.len(), 3);
    assert_eq!(
        commits.first().ok_or("missing commit 0")?.as_str(),
        oid3.to_string()
    );
    assert_eq!(
        commits.get(1).ok_or("missing commit 1")?.as_str(),
        oid2.to_string()
    );
    assert_eq!(
        commits.get(2).ok_or("missing commit 2")?.as_str(),
        oid1.to_string()
    );

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_exact(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let oid = create_commit(&repo, "Add file", &[("test.txt", "line1\nline2\nline3")])?;

    let ops = Git2Operations::from_repository(repo);
    let request =
        LineMappingRequest::new(oid.to_string(), oid.to_string(), "test.txt".to_owned(), 2);
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Exact);
    assert_eq!(verification.original_line(), 2);
    assert_eq!(verification.current_line(), Some(2));

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_no_change(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let oid1 = create_commit(&repo, "Add file", &[("test.txt", "line1\nline2\nline3")])?;
    let oid2 = create_commit(&repo, "Other file", &[("other.txt", "other content")])?;

    let ops = Git2Operations::from_repository(repo);
    let request =
        LineMappingRequest::new(oid1.to_string(), oid2.to_string(), "test.txt".to_owned(), 2);
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Exact);

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_shifts_when_line_moves_within_hunk(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let old_oid = create_commit(
        &repo,
        "Add initial file",
        &[("test.txt", "alpha\nbeta\ngamma\n")],
    )?;
    let new_oid = create_commit(
        &repo,
        "Insert line at top",
        &[("test.txt", "inserted\nalpha\nbeta\ngamma\n")],
    )?;

    let ops = Git2Operations::from_repository(repo);
    let request = LineMappingRequest::new(
        old_oid.to_string(),
        new_oid.to_string(),
        "test.txt".to_owned(),
        2,
    );
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Moved);
    assert_eq!(verification.original_line(), 2);
    assert_eq!(verification.current_line(), Some(3));
    assert_eq!(verification.offset(), Some(1));

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_shifts_after_deletion_within_hunk(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let old_oid = create_commit(
        &repo,
        "Add initial file",
        &[("test.txt", "drop\nkeep-one\nkeep-two\n")],
    )?;
    let new_oid = create_commit(
        &repo,
        "Delete first line",
        &[("test.txt", "keep-one\nkeep-two\n")],
    )?;

    let ops = Git2Operations::from_repository(repo);
    let request = LineMappingRequest::new(
        old_oid.to_string(),
        new_oid.to_string(),
        "test.txt".to_owned(),
        3,
    );
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Moved);
    assert_eq!(verification.original_line(), 3);
    assert_eq!(verification.current_line(), Some(2));
    assert_eq!(verification.offset(), Some(-1));

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_marks_deleted_line_inside_hunk(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let old_oid = create_commit(
        &repo,
        "Add initial file",
        &[("test.txt", "line-1\nline-2\nline-3\n")],
    )?;
    let new_oid = create_commit(
        &repo,
        "Delete middle line",
        &[("test.txt", "line-1\nline-3\n")],
    )?;

    let ops = Git2Operations::from_repository(repo);
    let request = LineMappingRequest::new(
        old_oid.to_string(),
        new_oid.to_string(),
        "test.txt".to_owned(),
        2,
    );
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Deleted);
    assert_eq!(verification.original_line(), 2);
    assert_eq!(verification.current_line(), None);
    assert_eq!(verification.offset(), None);

    drop(dir);
    Ok(())
}
