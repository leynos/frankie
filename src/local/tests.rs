//! Unit tests for local repository discovery.

use git2::Repository;
use tempfile::TempDir;

use super::discovery::{discover_repository, discover_repository_with_remote};
use super::error::LocalDiscoveryError;
use super::remote::GitHubOrigin;

/// Creates a temporary Git repository with the specified origin URL.
fn create_temp_repo_with_origin(origin_url: &str) -> (TempDir, Repository) {
    let temp_dir = TempDir::new().expect("should create temp directory");
    let repo = Repository::init(temp_dir.path()).expect("should init repository");
    repo.remote("origin", origin_url)
        .expect("should add origin remote");
    (temp_dir, repo)
}

/// Creates a temporary Git repository with no remotes.
fn create_temp_repo_no_remotes() -> (TempDir, Repository) {
    let temp_dir = TempDir::new().expect("should create temp directory");
    let repo = Repository::init(temp_dir.path()).expect("should init repository");
    (temp_dir, repo)
}

/// Creates a temporary directory that is not a Git repository.
fn create_non_repo_dir() -> TempDir {
    TempDir::new().expect("should create temp directory")
}

#[test]
fn discover_repository_with_ssh_origin() {
    let (temp_dir, _repo) = create_temp_repo_with_origin("git@github.com:octo/cat.git");

    let result = discover_repository(temp_dir.path());

    let local_repo = result.expect("should discover repository");
    assert_eq!(local_repo.owner(), "octo");
    assert_eq!(local_repo.repository(), "cat");
    assert_eq!(local_repo.remote_name(), "origin");
    assert!(local_repo.github_origin().is_github_com());
}

#[test]
fn discover_repository_with_https_origin() {
    let (temp_dir, _repo) = create_temp_repo_with_origin("https://github.com/owner/repo.git");

    let result = discover_repository(temp_dir.path());

    let local_repo = result.expect("should discover repository");
    assert_eq!(local_repo.owner(), "owner");
    assert_eq!(local_repo.repository(), "repo");
}

#[test]
fn discover_repository_with_enterprise_origin() {
    let (temp_dir, _repo) = create_temp_repo_with_origin("git@ghe.example.com:org/project.git");

    let result = discover_repository(temp_dir.path());

    let local_repo = result.expect("should discover repository");
    assert_eq!(local_repo.owner(), "org");
    assert_eq!(local_repo.repository(), "project");
    assert!(!local_repo.github_origin().is_github_com());

    match local_repo.github_origin() {
        GitHubOrigin::Enterprise { host, .. } => {
            assert_eq!(host, "ghe.example.com");
        }
        GitHubOrigin::GitHubCom { .. } => panic!("expected Enterprise variant"),
    }
}

#[test]
fn discover_repository_not_a_repo() {
    let temp_dir = create_non_repo_dir();

    let result = discover_repository(temp_dir.path());

    assert!(matches!(result, Err(LocalDiscoveryError::NotARepository)));
}

#[test]
fn discover_repository_no_remotes() {
    let (temp_dir, _repo) = create_temp_repo_no_remotes();

    let result = discover_repository(temp_dir.path());

    assert!(matches!(result, Err(LocalDiscoveryError::NoRemotes)));
}

#[test]
fn discover_repository_remote_not_found() {
    let (temp_dir, _repo) = create_temp_repo_with_origin("git@github.com:owner/repo.git");

    let result = discover_repository_with_remote(temp_dir.path(), "upstream");

    assert!(matches!(
        result,
        Err(LocalDiscoveryError::RemoteNotFound { name }) if name == "upstream"
    ));
}

#[test]
fn discover_repository_non_github_origin() {
    let (temp_dir, _repo) = create_temp_repo_with_origin("git@gitlab.com:owner/repo.git");

    let result = discover_repository(temp_dir.path());

    // GitLab is parsed but as a non-github.com host, which is treated as Enterprise
    // But the test should still pass since GitLab format is similar
    let local_repo = result.expect("should discover repository");
    assert_eq!(local_repo.owner(), "owner");
    assert_eq!(local_repo.repository(), "repo");
}

#[test]
fn discover_repository_invalid_url() {
    let (temp_dir, _repo) = create_temp_repo_with_origin("not-a-valid-url");

    let result = discover_repository(temp_dir.path());

    assert!(matches!(
        result,
        Err(LocalDiscoveryError::NotGitHubOrigin { .. })
    ));
}

#[test]
fn discover_repository_from_subdirectory() {
    let (temp_dir, _repo) = create_temp_repo_with_origin("git@github.com:owner/repo.git");

    // Create a subdirectory
    let subdir = temp_dir.path().join("src").join("lib");
    std::fs::create_dir_all(&subdir).expect("should create subdirectory");

    let result = discover_repository(&subdir);

    let local_repo = result.expect("should discover repository from subdirectory");
    assert_eq!(local_repo.owner(), "owner");
    assert_eq!(local_repo.repository(), "repo");
}

#[test]
fn workdir_is_correct() {
    let (temp_dir, _repo) = create_temp_repo_with_origin("git@github.com:owner/repo.git");

    let result = discover_repository(temp_dir.path());

    let local_repo = result.expect("should discover repository");
    // The workdir should be the temp directory
    assert_eq!(
        local_repo
            .workdir()
            .canonicalize()
            .expect("should canonicalize"),
        temp_dir
            .path()
            .canonicalize()
            .expect("should canonicalize temp")
    );
}

#[test]
fn discover_with_custom_remote() {
    let temp_dir = TempDir::new().expect("should create temp directory");
    let repo = Repository::init(temp_dir.path()).expect("should init repository");
    repo.remote("upstream", "git@github.com:upstream/project.git")
        .expect("should add upstream remote");

    let result = discover_repository_with_remote(temp_dir.path(), "upstream");

    let local_repo = result.expect("should discover with custom remote");
    assert_eq!(local_repo.owner(), "upstream");
    assert_eq!(local_repo.repository(), "project");
    assert_eq!(local_repo.remote_name(), "upstream");
}
