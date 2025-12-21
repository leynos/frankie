//! Behavioural tests for local repository discovery.

use frankie::local::{LocalDiscoveryError, LocalRepository, discover_repository};
use git2::Repository;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;

/// State for local discovery BDD scenarios.
#[derive(ScenarioState, Default)]
struct DiscoveryState {
    temp_dir: Slot<TempDir>,
    result: Slot<LocalRepository>,
    error: Slot<LocalDiscoveryError>,
}

#[fixture]
fn discovery_state() -> DiscoveryState {
    DiscoveryState::default()
}

#[given("a Git repository with origin {origin_url}")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn create_repo_with_origin(discovery_state: &DiscoveryState, origin_url: String) {
    let origin_url_clean = origin_url.trim_matches('"');
    let temp_dir = TempDir::new().expect("should create temp directory");
    let repo = Repository::init(temp_dir.path()).expect("should init repository");
    repo.remote("origin", origin_url_clean)
        .expect("should add origin remote");
    discovery_state.temp_dir.set(temp_dir);
}

#[given("a Git repository with no remotes")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn create_repo_no_remotes(discovery_state: &DiscoveryState) {
    let temp_dir = TempDir::new().expect("should create temp directory");
    let _repo = Repository::init(temp_dir.path()).expect("should init repository");
    discovery_state.temp_dir.set(temp_dir);
}

#[when("the discovery is performed")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn perform_discovery(discovery_state: &DiscoveryState) {
    let path = discovery_state
        .temp_dir
        .with_ref(|t: &TempDir| t.path().to_owned())
        .expect("temp directory not initialised");

    match discover_repository(&path) {
        Ok(local_repo) => {
            discovery_state.result.set(local_repo);
        }
        Err(error) => {
            discovery_state.error.set(error);
        }
    }
}

#[then("the owner is detected as {expected_owner}")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_owner(discovery_state: &DiscoveryState, expected_owner: String) {
    let expected_owner_clean = expected_owner.trim_matches('"');
    let actual = discovery_state
        .result
        .with_ref(|r: &LocalRepository| r.owner().to_owned())
        .expect("discovery result missing");

    assert_eq!(actual, expected_owner_clean, "owner mismatch");
}

#[then("the repository is detected as {expected_repo}")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_repository(discovery_state: &DiscoveryState, expected_repo: String) {
    let expected_repo_clean = expected_repo.trim_matches('"');
    let actual = discovery_state
        .result
        .with_ref(|r: &LocalRepository| r.repository().to_owned())
        .expect("discovery result missing");

    assert_eq!(actual, expected_repo_clean, "repository mismatch");
}

#[then("the API base is {expected_api_base}")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_api_base(discovery_state: &DiscoveryState, expected_api_base: String) {
    let expected_api_base_clean = expected_api_base.trim_matches('"');
    let origin = discovery_state
        .result
        .with_ref(|r: &LocalRepository| r.github_origin().clone())
        .expect("discovery result missing");

    let locator = frankie::RepositoryLocator::from_github_origin(&origin)
        .expect("should create locator from origin");

    assert_eq!(
        locator.api_base().as_str(),
        expected_api_base_clean,
        "API base mismatch"
    );
}

#[then("the discovery fails with no remotes error")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_no_remotes_error(discovery_state: &DiscoveryState) {
    let error = discovery_state
        .error
        .with_ref(Clone::clone)
        .expect("expected error but got success");

    assert!(
        matches!(error, LocalDiscoveryError::NoRemotes),
        "expected NoRemotes error, got {error:?}"
    );
}

#[then("the discovery fails with not GitHub origin error")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_not_github_origin_error(discovery_state: &DiscoveryState) {
    let error = discovery_state
        .error
        .with_ref(Clone::clone)
        .expect("expected error but got success");

    assert!(
        matches!(error, LocalDiscoveryError::NotGitHubOrigin { .. }),
        "expected NotGitHubOrigin error, got {error:?}"
    );
}

#[scenario(path = "tests/features/local_discovery.feature", index = 0)]
fn discover_ssh_origin(discovery_state: DiscoveryState) {
    let _ = discovery_state;
}

#[scenario(path = "tests/features/local_discovery.feature", index = 1)]
fn discover_https_origin(discovery_state: DiscoveryState) {
    let _ = discovery_state;
}

#[scenario(path = "tests/features/local_discovery.feature", index = 2)]
fn warn_origin_missing(discovery_state: DiscoveryState) {
    let _ = discovery_state;
}

#[scenario(path = "tests/features/local_discovery.feature", index = 3)]
fn warn_origin_not_parseable(discovery_state: DiscoveryState) {
    let _ = discovery_state;
}

#[scenario(path = "tests/features/local_discovery.feature", index = 4)]
fn discover_enterprise_origin(discovery_state: DiscoveryState) {
    let _ = discovery_state;
}
