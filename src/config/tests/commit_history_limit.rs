//! Tests for `commit_history_limit` loading and precedence.

use ortho_config::OrthoConfig;
use rstest::rstest;
use serde_json::json;

use super::helpers::build_config_from_layers;
use crate::FrankieConfig;
use crate::config::DEFAULT_COMMIT_HISTORY_LIMIT;

/// Infrastructure helper: locks the environment, constructs argv, and loads
/// a `FrankieConfig`.  Callers own all assertions.
fn load_config_under_test(env_limit: Option<&str>, cli_args: &[&str]) -> FrankieConfig {
    let temp_dir = tempfile::TempDir::new().expect("temp dir should be created");
    let home = temp_dir.path().to_string_lossy().to_string();

    let _guard = env_lock::lock_env([
        ("FRANKIE_COMMIT_HISTORY_LIMIT", env_limit),
        ("HOME", Some(home.as_str())),
        ("XDG_CONFIG_HOME", Some(home.as_str())),
    ]);

    let mut args: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("frankie")];
    args.extend(cli_args.iter().map(std::ffi::OsString::from));

    FrankieConfig::load_from_iter(args).expect("config should load")
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
    let config = load_config_under_test(Some("10"), &[]);
    assert_eq!(
        config.commit_history_limit, 10,
        "expected FRANKIE_COMMIT_HISTORY_LIMIT to set limit"
    );
}

#[rstest]
fn commit_history_limit_loads_from_cli_flag() {
    let config = load_config_under_test(None, &["--commit-history-limit", "25"]);
    assert_eq!(
        config.commit_history_limit, 25,
        "expected --commit-history-limit to set limit"
    );
}

#[rstest]
fn commit_history_limit_cli_overrides_environment() {
    let config = load_config_under_test(Some("10"), &["--commit-history-limit", "25"]);
    assert_eq!(
        config.commit_history_limit, 25,
        "CLI should override environment for commit_history_limit"
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

#[rstest]
fn commit_history_limit_zero_from_config_is_clamped_to_one() {
    let layers = vec![("file", json!({"commit_history_limit": 0}))];
    let mut config = build_config_from_layers(&layers);

    // normalize() should clamp 0 to 1
    config.normalize();

    assert_eq!(
        config.commit_history_limit, 1,
        "commit_history_limit of 0 should be clamped to 1"
    );
}

#[rstest]
#[case::from_env(
    Some("0"),
    &[],
    "commit_history_limit of 0 from env should be clamped to 1"
)]
#[case::from_cli(
    None,
    &["--commit-history-limit", "0"],
    "commit_history_limit of 0 from CLI should be clamped to 1"
)]
fn commit_history_limit_zero_is_clamped_to_one(
    #[case] env_limit: Option<&str>,
    #[case] cli_args: &[&str],
    #[case] description: &str,
) {
    let mut config = load_config_under_test(env_limit, cli_args);
    config.normalize();
    assert_eq!(config.commit_history_limit, 1, "{description}");
}

#[rstest]
fn commit_history_limit_large_values_are_accepted() {
    let large_limit = 10_000usize;
    let large_limit_str = large_limit.to_string();

    // Config layer
    let layers_config = vec![("file", json!({"commit_history_limit": large_limit}))];
    let mut config_from_config = build_config_from_layers(&layers_config);
    config_from_config.normalize();
    assert_eq!(
        config_from_config.commit_history_limit, large_limit,
        "large commit_history_limit from config should be accepted unchanged"
    );

    // Env layer
    let temp_dir = tempfile::TempDir::new().expect("temp dir should be created");
    let home = temp_dir.path().to_string_lossy().to_string();

    let _guard = env_lock::lock_env([
        (
            "FRANKIE_COMMIT_HISTORY_LIMIT",
            Some(large_limit_str.as_str()),
        ),
        ("HOME", Some(home.as_str())),
        ("XDG_CONFIG_HOME", Some(home.as_str())),
    ]);

    let args_env: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("frankie")];
    let mut config_from_env = FrankieConfig::load_from_iter(args_env).expect("config should load");
    config_from_env.normalize();
    assert_eq!(
        config_from_env.commit_history_limit, large_limit,
        "large commit_history_limit from env should be accepted unchanged"
    );

    // CLI layer
    let args_cli: Vec<std::ffi::OsString> = vec![
        std::ffi::OsString::from("frankie"),
        std::ffi::OsString::from("--commit-history-limit"),
        std::ffi::OsString::from(large_limit.to_string()),
    ];
    let mut config_from_cli = FrankieConfig::load_from_iter(args_cli).expect("config should load");
    config_from_cli.normalize();
    assert_eq!(
        config_from_cli.commit_history_limit, large_limit,
        "large commit_history_limit from CLI should be accepted unchanged"
    );
}
