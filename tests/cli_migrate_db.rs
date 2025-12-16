//! CLI integration tests for the `--migrate-db` flag.
//!
//! These tests spawn the Frankie binary as a subprocess to verify process exit
//! behaviour and ensure no GitHub operations occur during migration-only runs.
#![allow(
    clippy::expect_used,
    clippy::missing_panics_doc,
    reason = "test code; panics are acceptable in test fixtures and assertions"
)]

use std::process::Command;

use tempfile::TempDir;

/// Returns the path to the built binary.
fn binary_path() -> std::path::PathBuf {
    // cargo test builds binaries in target/debug
    let mut path = std::env::current_exe().expect("failed to get current exe path");
    path.pop(); // remove test binary name
    path.pop(); // remove deps
    path.push("frankie");
    path
}

/// Creates a temporary directory for database tests.
fn create_temp_dir() -> TempDir {
    TempDir::new().expect("failed to create temporary directory")
}

#[test]
fn migrate_db_succeeds_with_in_memory_database() {
    let output = Command::new(binary_path())
        .args(["--migrate-db", "--database-url", ":memory:"])
        .output()
        .expect("failed to execute binary");

    assert!(
        output.status.success(),
        "expected successful exit, got: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn migrate_db_succeeds_with_file_database() {
    let temp_dir = create_temp_dir();
    let db_path = temp_dir.path().join("frankie.sqlite");
    let db_url = db_path.to_string_lossy();

    let output = Command::new(binary_path())
        .args(["--migrate-db", "--database-url", &db_url])
        .output()
        .expect("failed to execute binary");

    assert!(
        output.status.success(),
        "expected successful exit, got: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        db_path.exists(),
        "database file should be created at {}",
        db_path.display()
    );
}

#[test]
fn migrate_db_emits_telemetry_to_stderr() {
    let output = Command::new(binary_path())
        .args(["--migrate-db", "--database-url", ":memory:"])
        .output()
        .expect("failed to execute binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("schema_version_recorded"),
        "expected telemetry on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("20251214000000"),
        "expected schema version in telemetry, got: {stderr}"
    );
}

#[test]
fn migrate_db_does_not_perform_github_operations() {
    // Running with --migrate-db and no GitHub args should succeed without
    // attempting any network calls. Providing an invalid token or no token
    // should not matter since GitHub operations are skipped.
    let output = Command::new(binary_path())
        .args(["--migrate-db", "--database-url", ":memory:"])
        .env_remove("GITHUB_TOKEN")
        .env_remove("FRANKIE_TOKEN")
        .output()
        .expect("failed to execute binary");

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

#[test]
fn migrate_db_fails_without_database_url() {
    let output = Command::new(binary_path())
        .args(["--migrate-db"])
        .env_remove("FRANKIE_DATABASE_URL")
        .output()
        .expect("failed to execute binary");

    assert!(
        !output.status.success(),
        "should fail when database URL is missing"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("database URL is required"),
        "should report missing database URL error, got: {stderr}"
    );
}

#[test]
fn migrate_db_fails_with_blank_database_url() {
    let output = Command::new(binary_path())
        .args(["--migrate-db", "--database-url", "   "])
        .output()
        .expect("failed to execute binary");

    assert!(
        !output.status.success(),
        "should fail when database URL is blank"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("database URL must not be blank"),
        "should report blank database URL error, got: {stderr}"
    );
}

#[test]
fn migrate_db_fails_with_directory_path() {
    let temp_dir = create_temp_dir();
    let dir_path = temp_dir.path().to_string_lossy();

    let output = Command::new(binary_path())
        .args(["--migrate-db", "--database-url", &dir_path])
        .output()
        .expect("failed to execute binary");

    assert!(
        !output.status.success(),
        "should fail when database URL is a directory"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to connect to SQLite database"),
        "should report connection error, got: {stderr}"
    );
}

#[test]
fn migrate_db_exits_with_success_code_on_success() {
    let output = Command::new(binary_path())
        .args(["--migrate-db", "--database-url", ":memory:"])
        .output()
        .expect("failed to execute binary");

    assert_eq!(
        output.status.code(),
        Some(0),
        "should exit with code 0 on success"
    );
}

#[test]
fn migrate_db_exits_with_failure_code_on_error() {
    let output = Command::new(binary_path())
        .args(["--migrate-db"])
        .env_remove("FRANKIE_DATABASE_URL")
        .output()
        .expect("failed to execute binary");

    assert_eq!(
        output.status.code(),
        Some(1),
        "should exit with code 1 on failure"
    );
}

#[test]
fn migrate_db_is_idempotent() {
    let temp_dir = create_temp_dir();
    let db_path = temp_dir.path().join("frankie.sqlite");
    let db_url = db_path.to_string_lossy();

    // First migration
    let first = Command::new(binary_path())
        .args(["--migrate-db", "--database-url", &db_url])
        .output()
        .expect("failed to execute first migration");

    assert!(first.status.success(), "first migration should succeed");

    // Second migration
    let second = Command::new(binary_path())
        .args(["--migrate-db", "--database-url", &db_url])
        .output()
        .expect("failed to execute second migration");

    assert!(
        second.status.success(),
        "second migration should succeed (idempotent)"
    );

    // Both should emit the same schema version
    let first_stderr = String::from_utf8_lossy(&first.stderr);
    let second_stderr = String::from_utf8_lossy(&second.stderr);

    assert!(
        first_stderr.contains("20251214000000"),
        "first run should emit schema version"
    );
    assert!(
        second_stderr.contains("20251214000000"),
        "second run should emit same schema version"
    );
}
