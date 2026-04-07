//! Tests for `commit_history_limit` loading and precedence.

use ortho_config::OrthoConfig;
use rstest::rstest;
use serde_json::json;

use super::helpers::build_config_from_layers;
use crate::FrankieConfig;
use crate::config::DEFAULT_COMMIT_HISTORY_LIMIT;

/// Helper to test `commit_history_limit` loading from environment and/or CLI.
fn test_commit_history_limit_loading(
    env_limit: Option<&str>,
    cli_args: &[&str],
    expected_limit: usize,
    description: &str,
) {
    let temp_dir = tempfile::TempDir::new().expect("temp dir should be created");
    let home = temp_dir.path().to_string_lossy().to_string();

    let _guard = env_lock::lock_env([
        ("FRANKIE_COMMIT_HISTORY_LIMIT", env_limit),
        ("HOME", Some(home.as_str())),
        ("XDG_CONFIG_HOME", Some(home.as_str())),
    ]);

    let mut args: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("frankie")];
    args.extend(cli_args.iter().map(std::ffi::OsString::from));

    let config = FrankieConfig::load_from_iter(args).expect("config should load");

    assert_eq!(config.commit_history_limit, expected_limit, "{description}");
}

#[rstest]
fn commit_history_limit_defaults_to_50() {
    let config = FrankieConfig::default();
    assert_eq!(
        config.commit_history_limit, DEFAULT_COMMIT_HISTORY_LIMIT,
        "default commit_history_limit should be 50"
    );
}

#[rstest]
fn commit_history_limit_loads_from_environment_variable() {
    test_commit_history_limit_loading(
        Some("10"),
        &[],
        10,
        "expected FRANKIE_COMMIT_HISTORY_LIMIT to set limit",
    );
}

#[rstest]
fn commit_history_limit_loads_from_cli_flag() {
    test_commit_history_limit_loading(
        None,
        &["--commit-history-limit", "25"],
        25,
        "expected --commit-history-limit to set limit",
    );
}

#[rstest]
fn commit_history_limit_cli_overrides_environment() {
    test_commit_history_limit_loading(
        Some("10"),
        &["--commit-history-limit", "25"],
        25,
        "CLI should override environment for commit_history_limit",
    );
}

#[rstest]
#[case::file_overrides_defaults(
    vec![
        ("defaults", json!({"commit_history_limit": 10})),
        ("file", json!({"commit_history_limit": 20}))
    ],
    20
)]
#[case::environment_overrides_file(
    vec![
        ("file", json!({"commit_history_limit": 10})),
        ("environment", json!({"commit_history_limit": 20}))
    ],
    20
)]
#[case::cli_overrides_environment(
    vec![
        ("environment", json!({"commit_history_limit": 10})),
        ("cli", json!({"commit_history_limit": 20}))
    ],
    20
)]
fn commit_history_limit_layer_precedence(
    #[case] layers: Vec<(&str, serde_json::Value)>,
    #[case] expected: usize,
) {
    let config = build_config_from_layers(&layers);
    assert_eq!(
        config.commit_history_limit, expected,
        "commit_history_limit should follow standard precedence rules"
    );
}
