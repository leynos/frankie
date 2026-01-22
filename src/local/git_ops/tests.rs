#![expect(clippy::indexing_slicing, reason = "Test assertions")]
#![expect(clippy::unwrap_used, reason = "Tests panic on failure")]

use std::fs;
use std::path::Path;

use git2::{Oid, Repository};
use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::*;
use crate::local::LineMappingStatus;
use crate::local::commit::LineMappingRequest;
use crate::local::error::GitOperationError;
use crate::local::types::{CommitSha, RepoFilePath};

#[fixture]
fn test_repo() -> (TempDir, Repository) {
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

/// Helper to test Git operation error handling with custom setup.
fn test_git_error<F, T>(
    test_repo: (TempDir, Repository),
    setup: impl FnOnce(&Repository) -> String,
    operation: F,
) -> GitOperationError
where
    F: FnOnce(&Git2Operations, &str) -> Result<T, GitOperationError>,
    T: std::fmt::Debug,
{
    let (dir, repo) = test_repo;
    let param = setup(&repo);
    let ops = Git2Operations::from_repository(repo);

    let result = operation(&ops, &param);

    let err = result.unwrap_err();
    drop(dir);
    err
}

#[rstest]
fn test_commit_snapshot(test_repo: (TempDir, Repository)) {
    let (dir, repo) = test_repo;
    let oid = create_commit(&repo, "Initial commit", &[("test.txt", "hello")]);

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid.to_string());
    let snapshot = ops.get_commit_snapshot(&sha, None).unwrap();

    assert_eq!(snapshot.message(), "Initial commit");
    assert_eq!(snapshot.author(), "Test User");
    assert!(snapshot.file_content().is_none());

    drop(dir);
}

#[rstest]
fn test_commit_snapshot_with_file(test_repo: (TempDir, Repository)) {
    let (dir, repo) = test_repo;
    let oid = create_commit(&repo, "Add file", &[("src/main.rs", "fn main() {}")]);

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid.to_string());
    let path = RepoFilePath::new("src/main.rs".to_owned());
    let snapshot = ops.get_commit_snapshot(&sha, Some(&path)).unwrap();

    assert_eq!(snapshot.file_content(), Some("fn main() {}"));
    assert_eq!(snapshot.file_path(), Some("src/main.rs"));

    drop(dir);
}

#[rstest]
fn test_get_file_at_commit(test_repo: (TempDir, Repository)) {
    let (dir, repo) = test_repo;
    let oid = create_commit(&repo, "Add file", &[("test.txt", "content here")]);

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid.to_string());
    let path = RepoFilePath::new("test.txt".to_owned());
    let content = ops.get_file_at_commit(&sha, &path).unwrap();

    assert_eq!(content, "content here");

    drop(dir);
}

#[rstest]
fn test_file_not_found(test_repo: (TempDir, Repository)) {
    let err = test_git_error(
        test_repo,
        |repo| {
            let oid = create_commit(repo, "Add file", &[("test.txt", "content")]);
            oid.to_string()
        },
        |ops, sha| {
            let commit_sha = CommitSha::new(sha.to_owned());
            let path = RepoFilePath::new("nonexistent.txt".to_owned());
            ops.get_file_at_commit(&commit_sha, &path)
        },
    );

    assert!(matches!(err, GitOperationError::FileNotFound { .. }));
}

#[rstest]
fn test_commit_not_found(test_repo: (TempDir, Repository)) {
    let err = test_git_error(
        test_repo,
        |repo| {
            create_commit(repo, "Initial", &[("test.txt", "content")]);
            "0000000000000000000000000000000000000000".to_owned()
        },
        |ops, sha| {
            let commit_sha = CommitSha::new(sha.to_owned());
            ops.get_commit_snapshot(&commit_sha, None)
        },
    );

    assert!(matches!(err, GitOperationError::CommitNotFound { .. }));
}

#[rstest]
fn test_commit_exists(test_repo: (TempDir, Repository)) {
    let (dir, repo) = test_repo;
    let oid = create_commit(&repo, "Initial", &[("test.txt", "content")]);

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid.to_string());
    let nonexistent = CommitSha::new("0000000000000000000000000000000000000000".to_owned());

    assert!(ops.commit_exists(&sha));
    assert!(!ops.commit_exists(&nonexistent));

    drop(dir);
}

#[rstest]
fn test_get_parent_commits(test_repo: (TempDir, Repository)) {
    let (dir, repo) = test_repo;
    let oid1 = create_commit(&repo, "First", &[("test.txt", "v1")]);
    let oid2 = create_commit(&repo, "Second", &[("test.txt", "v2")]);
    let oid3 = create_commit(&repo, "Third", &[("test.txt", "v3")]);

    let ops = Git2Operations::from_repository(repo);
    let sha = CommitSha::new(oid3.to_string());
    let commits = ops.get_parent_commits(&sha, 10).unwrap();

    assert_eq!(commits.len(), 3);
    assert_eq!(commits[0].as_str(), oid3.to_string());
    assert_eq!(commits[1].as_str(), oid2.to_string());
    assert_eq!(commits[2].as_str(), oid1.to_string());

    drop(dir);
}

#[rstest]
fn test_line_mapping_exact(test_repo: (TempDir, Repository)) {
    let (dir, repo) = test_repo;
    let oid = create_commit(&repo, "Add file", &[("test.txt", "line1\nline2\nline3")]);

    let ops = Git2Operations::from_repository(repo);
    let request =
        LineMappingRequest::new(oid.to_string(), oid.to_string(), "test.txt".to_owned(), 2);
    let verification = ops.verify_line_mapping(&request).unwrap();

    assert_eq!(verification.status(), LineMappingStatus::Exact);
    assert_eq!(verification.original_line(), 2);
    assert_eq!(verification.current_line(), Some(2));

    drop(dir);
}

#[rstest]
fn test_line_mapping_no_change(test_repo: (TempDir, Repository)) {
    let (dir, repo) = test_repo;
    let oid1 = create_commit(&repo, "Add file", &[("test.txt", "line1\nline2\nline3")]);
    let oid2 = create_commit(&repo, "Other file", &[("other.txt", "other content")]);

    let ops = Git2Operations::from_repository(repo);
    let request =
        LineMappingRequest::new(oid1.to_string(), oid2.to_string(), "test.txt".to_owned(), 2);
    let verification = ops.verify_line_mapping(&request).unwrap();

    assert_eq!(verification.status(), LineMappingStatus::Exact);

    drop(dir);
}
