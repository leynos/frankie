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
    assert_interactive_mode_error(
        || create_temp_repo_with_origin("git@github.com:octo/cat.git"),
        &["--no-local-discovery", "--token", "ghp_test"],
        "--no-local-discovery is set without explicit arguments",
        |stderr| stderr.contains("--pr-url") || stderr.contains("--owner"),
        "error message should mention required arguments",
    );
}

#[rstest]
fn no_local_discovery_short_flag_requires_explicit_arguments() {
    assert_interactive_mode_error(
        || create_temp_repo_with_origin("git@github.com:octo/cat.git"),
        &["-n", "--token", "ghp_test"],
        "-n is set without explicit arguments",
        |stderr| stderr.contains("--pr-url") || stderr.contains("--owner"),
        "error message should mention required arguments",
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
    assert_interactive_mode_error(
        create_temp_repo_no_remotes,
        &["--token", "ghp_test"],
        "repository has no remotes",
        |stderr| stderr.contains("no remotes"),
        "should warn about missing remotes",
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

/// Asserts that running Frankie in interactive mode with the given setup produces an error.
///
/// # Parameters
/// - `setup`: A closure that creates and returns a temporary directory for the test.
/// - `args`: Command-line arguments to pass to Frankie.
/// - `failure_reason`: A human-readable description of why the command should fail.
/// - `stderr_predicate`: A closure that returns `true` if stderr contains the expected content.
/// - `stderr_context`: A description of what stderr should contain (for assertion messages).
#[expect(
    clippy::too_many_arguments,
    reason = "test helper with descriptive parameters; clarity outweighs argument count"
)]
fn assert_interactive_mode_error<F>(
    setup: F,
    args: &[&str],
    failure_reason: &str,
    stderr_predicate: impl Fn(&str) -> bool,
    stderr_context: &str,
) where
    F: FnOnce() -> TempDir,
{
    let temp_dir = setup();

    let output = run_frankie_in_dir(args, &[], temp_dir.path());

    assert!(
        !output.status.success(),
        "should fail when {failure_reason}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr_predicate(&stderr), "{stderr_context}: {stderr}");
}

#[rstest]
fn interactive_mode_warns_when_not_in_git_repo() {
    assert_interactive_mode_error(
        create_temp_non_git_dir,
        &["--token", "ghp_test"],
        "not in a Git repository",
        |stderr| stderr.contains("--pr-url") || stderr.contains("--owner"),
        "error message should mention required arguments",
    );
}

#[rstest]
fn interactive_mode_warns_when_origin_url_invalid() {
    assert_interactive_mode_error(
        || create_temp_repo_with_origin("not-a-valid-url"),
        &["--token", "ghp_test"],
        "origin URL is invalid",
        |stderr| {
            stderr.contains("could not parse remote URL") || stderr.contains("not-a-valid-url")
        },
        "should warn about invalid URL",
    );
}
