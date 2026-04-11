//! Tests for `commit_history_limit` loading and precedence.

use rstest::rstest;
use serde_json::json;

use super::helpers::build_config_from_layers;
use crate::FrankieConfig;
use crate::config::DEFAULT_COMMIT_HISTORY_LIMIT;

/// Infrastructure helper: locks the environment, constructs argv, and loads
/// a `FrankieConfig`.  Callers own all assertions.
fn load_config_under_test(
    env_limit: Option<&str>,
    cli_args: &[&str],
) -> Result<FrankieConfig, Box<dyn std::error::Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let home = temp_dir.path().to_string_lossy().to_string();

    let _guard = env_lock::lock_env([
        ("FRANKIE_COMMIT_HISTORY_LIMIT", env_limit),
        ("HOME", Some(home.as_str())),
        ("XDG_CONFIG_HOME", Some(home.as_str())),
    ]);

    let mut args: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("frankie")];
    args.extend(cli_args.iter().map(std::ffi::OsString::from));

    FrankieConfig::load_from_iter(args)
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
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions via assert_eq! are expected to panic on failure"
)]
fn commit_history_limit_loads_from_environment_variable() -> Result<(), Box<dyn std::error::Error>>
{
    let config = load_config_under_test(Some("10"), &[])?;
    assert_eq!(
        config.commit_history_limit, 10,
        "expected FRANKIE_COMMIT_HISTORY_LIMIT to set limit"
    );
    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions via assert_eq! are expected to panic on failure"
)]
fn commit_history_limit_loads_from_cli_flag() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config_under_test(None, &["--commit-history-limit", "25"])?;
    assert_eq!(
        config.commit_history_limit, 25,
        "expected --commit-history-limit to set limit"
    );
    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions via assert_eq! are expected to panic on failure"
)]
fn commit_history_limit_cli_overrides_environment() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config_under_test(Some("10"), &["--commit-history-limit", "25"])?;
    assert_eq!(
        config.commit_history_limit, 25,
        "CLI should override environment for commit_history_limit"
    );
    Ok(())
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
#[case::from_env(Some("0"), &[])]
#[case::from_cli(None, &["--commit-history-limit", "0"])]
fn commit_history_limit_zero_is_clamped_to_one(
    #[case] env_limit: Option<&str>,
    #[case] cli_args: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config_under_test(env_limit, cli_args)?;

    if config.commit_history_limit != 1 {
        return Err(format!(
            "expected commit_history_limit to be clamped to 1, got {}",
            config.commit_history_limit
        )
        .into());
    }

    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions via assert_eq! are expected to panic on failure"
)]
fn commit_history_limit_large_values_are_accepted() -> Result<(), Box<dyn std::error::Error>> {
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
    let config_from_env = load_config_under_test(Some(&large_limit_str), &[])?;
    assert_eq!(
        config_from_env.commit_history_limit, large_limit,
        "large commit_history_limit from env should be accepted unchanged"
    );

    // CLI layer
    let config_from_cli =
        load_config_under_test(None, &["--commit-history-limit", &large_limit_str])?;
    assert_eq!(
        config_from_cli.commit_history_limit, large_limit,
        "large commit_history_limit from CLI should be accepted unchanged"
    );

    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions in Result-returning functions are expected to panic on failure"
)]
fn default_load_returns_normalised_config() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config_under_test(None, &["--commit-history-limit", "0"])?;
    assert_eq!(
        config.commit_history_limit, 1,
        "load_from_iter should auto-normalize commit_history_limit=0 to 1"
    );
    Ok(())
}
