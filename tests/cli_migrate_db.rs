//! CLI integration tests for the `--migrate-db` flag.
//!
//! These tests spawn the Frankie binary as a subprocess to verify process exit
//! behaviour and ensure no GitHub operations occur during migration-only runs.

mod support;

use std::process::{Command, Output};

use rstest::rstest;

use support::create_temp_dir;

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

fn run_frankie(args: &[&str], env: &[(&str, Option<&str>)]) -> Output {
    let mut command = Command::new(binary_path());
    command.args(args);

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

fn run_migrate_db(database_url: Option<&str>, env: &[(&str, Option<&str>)]) -> Output {
    let mut args = vec!["--migrate-db"];
    if let Some(database_url_value) = database_url {
        args.extend(["--database-url", database_url_value]);
    }

    run_frankie(&args, env)
}

fn assert_migrate_db_succeeds(database_url: &str) {
    let output = run_migrate_db(Some(database_url), &[]);
    assert!(
        output.status.success(),
        "expected successful exit, got: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_migrate_db_fails(
    database_url: Option<&str>,
    env: &[(&str, Option<&str>)],
    expected_stderr_substring: &str,
) {
    let output = run_migrate_db(database_url, env);
    assert!(!output.status.success(), "expected failure exit status");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected_stderr_substring),
        "expected stderr to contain {expected_stderr_substring:?}, got: {stderr}"
    );
}

fn assert_migrate_db_exit_code(database_url: Option<&str>, expected_code: i32) {
    let output = run_migrate_db(database_url, &[("FRANKIE_DATABASE_URL", None)]);
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "unexpected exit code: {:?}",
        output.status
    );
}

#[test]
fn migrate_db_succeeds_with_in_memory_database() {
    assert_migrate_db_succeeds(":memory:");
}

#[test]
fn migrate_db_succeeds_with_file_database() {
    let temp_dir = create_temp_dir();
    let db_path = temp_dir.path().join("frankie.sqlite");
    let db_url = db_path.to_string_lossy().to_string();

    assert_migrate_db_succeeds(&db_url);

    assert!(
        db_path.exists(),
        "database file should be created at {}",
        db_path.display()
    );
}

#[test]
fn migrate_db_emits_telemetry_to_stderr() {
    let output = run_migrate_db(Some(":memory:"), &[]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("schema_version_recorded"),
        "expected telemetry on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("20251220000000"),
        "expected schema version in telemetry, got: {stderr}"
    );
}

#[test]
fn migrate_db_does_not_perform_github_operations() {
    // Running with --migrate-db and no GitHub args should succeed without
    // attempting any network calls. Providing an invalid token or no token
    // should not matter since GitHub operations are skipped.
    let output = run_migrate_db(
        Some(":memory:"),
        &[("GITHUB_TOKEN", None), ("FRANKIE_TOKEN", None)],
    );

    assert!(
        output.status.success(),
        "should succeed without GitHub token when only migrating"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should not contain any GitHub-related output
    assert!(
        !stdout.contains("Pull request") && !stdout.contains("PR #"),
        "should not perform PR operations"
    );
    assert!(
        !stderr.contains("GitHub") && !stderr.contains("rate limit"),
        "should not interact with GitHub API"
    );
}

#[rstest]
#[case::missing_database_url(None, "database URL is required")]
#[case::blank_database_url(Some("   "), "database URL must not be blank")]
fn migrate_db_fails_with_invalid_database_url(
    #[case] database_url: Option<&str>,
    #[case] expected_stderr_substring: &str,
) {
    assert_migrate_db_fails(
        database_url,
        &[("FRANKIE_DATABASE_URL", None)],
        expected_stderr_substring,
    );
}

#[test]
fn migrate_db_fails_with_directory_path() {
    let temp_dir = create_temp_dir();
    let dir_path = temp_dir.path().to_string_lossy().to_string();

    assert_migrate_db_fails(
        Some(&dir_path),
        &[("FRANKIE_DATABASE_URL", None)],
        "failed to connect to SQLite database",
    );
}

#[rstest]
#[case::success_with_in_memory_database(Some(":memory:"), 0)]
#[case::failure_without_database_url(None, 1)]
fn migrate_db_exits_with_expected_code(
    #[case] database_url: Option<&str>,
    #[case] expected_code: i32,
) {
    assert_migrate_db_exit_code(database_url, expected_code);
}

#[test]
fn migrate_db_succeeds_with_database_url_from_environment() {
    let output = run_migrate_db(None, &[("FRANKIE_DATABASE_URL", Some(":memory:"))]);

    assert!(
        output.status.success(),
        "expected migration to succeed when FRANKIE_DATABASE_URL is set\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn migrate_db_cli_database_url_overrides_environment() {
    let output = run_migrate_db(Some(":memory:"), &[("FRANKIE_DATABASE_URL", Some("   "))]);

    assert!(
        output.status.success(),
        "expected CLI --database-url to override FRANKIE_DATABASE_URL\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn migrate_db_fails_with_blank_database_url_from_environment() {
    assert_migrate_db_fails(
        None,
        &[("FRANKIE_DATABASE_URL", Some("   "))],
        "database URL must not be blank",
    );
}

#[test]
fn migrate_db_is_idempotent() {
    let temp_dir = create_temp_dir();
    let db_path = temp_dir.path().join("frankie.sqlite");
    let db_url = db_path.to_string_lossy().to_string();

    // First migration
    let first = run_migrate_db(Some(&db_url), &[]);

    assert!(first.status.success(), "first migration should succeed");

    // Second migration
    let second = run_migrate_db(Some(&db_url), &[]);

    assert!(
        second.status.success(),
        "second migration should succeed (idempotent)"
    );

    // Both should emit the same schema version
    let first_stderr = String::from_utf8_lossy(&first.stderr);
    let second_stderr = String::from_utf8_lossy(&second.stderr);

    assert!(
        first_stderr.contains("20251220000000"),
        "first run should emit schema version"
    );
    assert!(
        second_stderr.contains("20251220000000"),
        "second run should emit same schema version"
    );
}
