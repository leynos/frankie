//! CLI integration tests for interactive mode (local repository discovery).
//!
//! These tests spawn the Frankie binary as a subprocess to verify process exit
//! behaviour when running in interactive mode with and without `--no-local-discovery`.

use std::process::{Command, Output};

use git2::Repository;
use rstest::rstest;
use tempfile::TempDir;

/// Returns the path to the built binary.
fn binary_path() -> std::path::PathBuf {
    // cargo test builds binaries in target/debug
    let mut path = std::env::current_exe()
        .unwrap_or_else(|error| panic!("failed to get current exe path: {error}"));
    path.pop(); // remove test binary name
    path.pop(); // remove deps
    path.push("frankie");
    path
}

fn run_frankie_in_dir(
    args: &[&str],
    env: &[(&str, Option<&str>)],
    working_dir: &std::path::Path,
) -> Output {
    let mut command = Command::new(binary_path());
    command.args(args);
    command.current_dir(working_dir);

    // Ensure tests are hermetic even if the developer has Frankie env vars set.
    command
        .env_remove("FRANKIE_DATABASE_URL")
        .env_remove("FRANKIE_MIGRATE_DB")
        .env_remove("FRANKIE_PR_URL")
        .env_remove("FRANKIE_TOKEN")
        .env_remove("FRANKIE_OWNER")
        .env_remove("FRANKIE_REPO")
        .env_remove("GITHUB_TOKEN");

    for (key, value) in env {
        match value {
            Some(env_value) => {
                command.env(key, env_value);
            }
            None => {
                command.env_remove(key);
            }
        }
    }

    command
        .output()
        .unwrap_or_else(|error| panic!("failed to execute binary: {error}"))
}

/// Creates a temporary Git repository with the given origin URL.
#[expect(
    clippy::expect_used,
    reason = "integration test setup; allow-expect-in-tests does not cover integration tests"
)]
fn create_temp_repo_with_origin(origin_url: &str) -> TempDir {
    let temp_dir = TempDir::new().expect("should create temp directory");
    let repo = Repository::init(temp_dir.path()).expect("should init repository");
    repo.remote("origin", origin_url)
        .expect("should add origin remote");
    temp_dir
}

/// Creates a temporary Git repository with no remotes.
#[expect(
    clippy::expect_used,
    reason = "integration test setup; allow-expect-in-tests does not cover integration tests"
)]
fn create_temp_repo_no_remotes() -> TempDir {
    let temp_dir = TempDir::new().expect("should create temp directory");
    let _repo = Repository::init(temp_dir.path()).expect("should init repository");
    temp_dir
}

#[rstest]
fn no_local_discovery_flag_requires_explicit_arguments() {
    let temp_dir = create_temp_repo_with_origin("git@github.com:octo/cat.git");

    let output = run_frankie_in_dir(
        &["--no-local-discovery", "--token", "ghp_test"],
        &[],
        temp_dir.path(),
    );

    assert!(
        !output.status.success(),
        "should fail when --no-local-discovery is set without explicit arguments"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--pr-url") || stderr.contains("--owner"),
        "error message should mention required arguments: {stderr}"
    );
}

#[rstest]
fn no_local_discovery_short_flag_requires_explicit_arguments() {
    let temp_dir = create_temp_repo_with_origin("git@github.com:octo/cat.git");

    let output = run_frankie_in_dir(&["-n", "--token", "ghp_test"], &[], temp_dir.path());

    assert!(
        !output.status.success(),
        "should fail when -n is set without explicit arguments"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--pr-url") || stderr.contains("--owner"),
        "error message should mention required arguments: {stderr}"
    );
}

#[rstest]
fn interactive_mode_discovers_repository_from_git_directory() {
    let temp_dir = create_temp_repo_with_origin("git@github.com:octo/cat.git");

    let output = run_frankie_in_dir(&["--token", "ghp_test"], &[], temp_dir.path());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Discovered repository from local Git: octo/cat"),
        "should print discovery message: {stderr}"
    );
}

#[rstest]
fn interactive_mode_warns_when_no_remotes() {
    let temp_dir = create_temp_repo_no_remotes();

    let output = run_frankie_in_dir(&["--token", "ghp_test"], &[], temp_dir.path());

    assert!(
        !output.status.success(),
        "should fail when repository has no remotes"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no remotes"),
        "should warn about missing remotes: {stderr}"
    );
}

/// Creates a temporary directory that is NOT a Git repository.
#[expect(
    clippy::expect_used,
    reason = "integration test setup; allow-expect-in-tests does not cover integration tests"
)]
fn create_temp_non_git_dir() -> TempDir {
    TempDir::new().expect("should create temp directory")
}

#[rstest]
fn interactive_mode_warns_when_not_in_git_repo() {
    let temp_dir = create_temp_non_git_dir();
    // Don't initialize git repo

    let output = run_frankie_in_dir(&["--token", "ghp_test"], &[], temp_dir.path());

    assert!(
        !output.status.success(),
        "should fail when not in a Git repository"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--pr-url") || stderr.contains("--owner"),
        "error message should mention required arguments: {stderr}"
    );
}

#[rstest]
fn interactive_mode_warns_when_origin_url_invalid() {
    let temp_dir = create_temp_repo_with_origin("not-a-valid-url");

    let output = run_frankie_in_dir(&["--token", "ghp_test"], &[], temp_dir.path());

    assert!(
        !output.status.success(),
        "should fail when origin URL is invalid"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not parse remote URL") || stderr.contains("not-a-valid-url"),
        "should warn about invalid URL: {stderr}"
    );
}
