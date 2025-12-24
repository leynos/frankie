//! Unit tests for local repository discovery.

use git2::Repository;
use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::discovery::{discover_repository, discover_repository_with_remote};
use super::error::LocalDiscoveryError;
use super::remote::GitHubOrigin;

/// A temporary Git repository with an origin remote configured.
struct TempRepoWithOrigin {
    temp_dir: TempDir,
    #[expect(
        dead_code,
        reason = "kept for lifetime; Repository is owned by temp_dir"
    )]
    repo: Repository,
}

/// A temporary Git repository with no remotes configured.
struct TempRepoNoRemotes {
    temp_dir: TempDir,
    #[expect(
        dead_code,
        reason = "kept for lifetime; Repository is owned by temp_dir"
    )]
    repo: Repository,
}

/// Creates a temporary Git repository with the specified origin URL.
fn create_temp_repo_with_origin(origin_url: &str) -> TempRepoWithOrigin {
    let temp_dir = TempDir::new().expect("should create temp directory");
    let repo = Repository::init(temp_dir.path()).expect("should init repository");
    repo.remote("origin", origin_url)
        .expect("should add origin remote");
    TempRepoWithOrigin { temp_dir, repo }
}

/// Creates a temporary Git repository with no remotes.
#[fixture]
fn temp_repo_no_remotes() -> TempRepoNoRemotes {
    let temp_dir = TempDir::new().expect("should create temp directory");
    let repo = Repository::init(temp_dir.path()).expect("should init repository");
    TempRepoNoRemotes { temp_dir, repo }
}

/// Creates a temporary directory that is not a Git repository.
#[fixture]
fn non_repo_dir() -> TempDir {
    TempDir::new().expect("should create temp directory")
}

/// Expected result for a GitHub.com origin discovery test case.
struct GitHubComExpected {
    owner: &'static str,
    repository: &'static str,
}

/// Expected result for a GitHub Enterprise origin discovery test case.
struct EnterpriseExpected {
    host: &'static str,
    owner: &'static str,
    repository: &'static str,
}

#[rstest]
#[case::ssh_scp_style(
    "git@github.com:octo/cat.git",
    GitHubComExpected { owner: "octo", repository: "cat" }
)]
#[case::https_with_git_suffix(
    "https://github.com/owner/repo.git",
    GitHubComExpected { owner: "owner", repository: "repo" }
)]
#[case::https_no_git_suffix(
    "https://github.com/user/project",
    GitHubComExpected { owner: "user", repository: "project" }
)]
#[case::ssh_url_style(
    "ssh://git@github.com/org/lib.git",
    GitHubComExpected { owner: "org", repository: "lib" }
)]
fn discover_github_com_origins(#[case] origin_url: &str, #[case] expected: GitHubComExpected) {
    let temp_repo = create_temp_repo_with_origin(origin_url);
    let result = discover_repository(temp_repo.temp_dir.path());

    let local_repo = result.expect("should discover repository");
    assert_eq!(local_repo.owner(), expected.owner);
    assert_eq!(local_repo.repository(), expected.repository);
    assert_eq!(local_repo.remote_name(), "origin");
    assert!(local_repo.github_origin().is_github_com());
}

#[rstest]
#[case::ssh_scp_style(
    "git@ghe.example.com:org/project.git",
    EnterpriseExpected { host: "ghe.example.com", owner: "org", repository: "project" }
)]
#[case::https(
    "https://github.acme.corp/team/app",
    EnterpriseExpected { host: "github.acme.corp", owner: "team", repository: "app" }
)]
fn discover_enterprise_origins(#[case] origin_url: &str, #[case] expected: EnterpriseExpected) {
    let temp_repo = create_temp_repo_with_origin(origin_url);
    let result = discover_repository(temp_repo.temp_dir.path());

    let local_repo = result.expect("should discover repository");
    assert_eq!(local_repo.owner(), expected.owner);
    assert_eq!(local_repo.repository(), expected.repository);
    assert!(!local_repo.github_origin().is_github_com());

    match local_repo.github_origin() {
        GitHubOrigin::Enterprise { host, .. } => {
            assert_eq!(host, expected.host);
        }
        GitHubOrigin::GitHubCom { .. } => panic!("expected Enterprise variant"),
    }
}

#[rstest]
fn discover_repository_not_a_repo(non_repo_dir: TempDir) {
    let result = discover_repository(non_repo_dir.path());

    assert!(matches!(result, Err(LocalDiscoveryError::NotARepository)));
}

#[rstest]
fn discover_repository_no_remotes(temp_repo_no_remotes: TempRepoNoRemotes) {
    let result = discover_repository(temp_repo_no_remotes.temp_dir.path());

    assert!(matches!(result, Err(LocalDiscoveryError::NoRemotes)));
}

#[test]
fn discover_repository_remote_not_found() {
    let temp_repo = create_temp_repo_with_origin("git@github.com:owner/repo.git");

    let result = discover_repository_with_remote(temp_repo.temp_dir.path(), "upstream");

    assert!(matches!(
        result,
        Err(LocalDiscoveryError::RemoteNotFound { name }) if name == "upstream"
    ));
}

#[test]
fn discover_repository_treats_non_github_com_as_enterprise() {
    let temp_repo = create_temp_repo_with_origin("git@gitlab.com:owner/repo.git");

    let result = discover_repository(temp_repo.temp_dir.path());

    let local_repo = result.expect("should discover repository");
    assert_eq!(local_repo.owner(), "owner");
    assert_eq!(local_repo.repository(), "repo");
    assert!(
        !local_repo.github_origin().is_github_com(),
        "non-github.com hosts should be treated as Enterprise"
    );
}

#[test]
fn discover_repository_invalid_url() {
    let temp_repo = create_temp_repo_with_origin("not-a-valid-url");

    let result = discover_repository(temp_repo.temp_dir.path());

    assert!(matches!(
        result,
        Err(LocalDiscoveryError::InvalidRemoteUrl { .. })
    ));
}

#[test]
fn discover_repository_from_subdirectory() {
    let temp_repo = create_temp_repo_with_origin("git@github.com:owner/repo.git");

    // Create a subdirectory
    let subdir = temp_repo.temp_dir.path().join("src").join("lib");
    std::fs::create_dir_all(&subdir).expect("should create subdirectory");

    let result = discover_repository(&subdir);

    let local_repo = result.expect("should discover repository from subdirectory");
    assert_eq!(local_repo.owner(), "owner");
    assert_eq!(local_repo.repository(), "repo");
}

#[test]
fn workdir_is_correct() {
    let temp_repo = create_temp_repo_with_origin("git@github.com:owner/repo.git");

    let result = discover_repository(temp_repo.temp_dir.path());

    let local_repo = result.expect("should discover repository");
    // The workdir should be the temp directory
    assert_eq!(
        local_repo
            .workdir()
            .canonicalize()
            .expect("should canonicalize"),
        temp_repo
            .temp_dir
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
