//! Tests for `pr_metadata_cache_ttl_seconds` loading and precedence.

use rstest::rstest;
use serde_json::json;

use super::helpers::build_config_from_layers;
use crate::FrankieConfig;

/// Helper to test `pr_metadata_cache_ttl_seconds` loading from environment and/or CLI.
///
/// # Errors
///
/// Returns an error when the temporary home cannot be created, the
/// configuration does not load, or the loaded TTL differs from `expected_ttl`.
fn test_pr_metadata_cache_ttl_seconds_loading(
    env_ttl: Option<&str>,
    cli_args: &[&str],
    expected_ttl: u64,
    description: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let home = temp_dir.path().to_string_lossy().to_string();

    let _guard = env_lock::lock_env([
        ("FRANKIE_PR_METADATA_CACHE_TTL_SECONDS", env_ttl),
        ("HOME", Some(home.as_str())),
        ("XDG_CONFIG_HOME", Some(home.as_str())),
    ]);

    let mut args: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("frankie")];
    args.extend(cli_args.iter().map(std::ffi::OsString::from));

    let config = FrankieConfig::load_from_iter(args)?;

    if config.pr_metadata_cache_ttl_seconds != expected_ttl {
        return Err(format!(
            "{description}: expected pr_metadata_cache_ttl_seconds = {expected_ttl}, got {}",
            config.pr_metadata_cache_ttl_seconds
        )
        .into());
    }
    Ok(())
}

#[rstest]
fn pr_metadata_cache_ttl_seconds_defaults_to_24_hours() {
    let config = FrankieConfig::default();
    assert_eq!(
        config.pr_metadata_cache_ttl_seconds, 86_400,
        "default pr_metadata_cache_ttl_seconds should be 24 hours"
    );
}

#[rstest]
fn pr_metadata_cache_ttl_seconds_loads_from_environment_variable() {
    test_pr_metadata_cache_ttl_seconds_loading(
        Some("3600"),
        &[],
        3600,
        "expected FRANKIE_PR_METADATA_CACHE_TTL_SECONDS to set TTL",
    )
    .expect("loading the configuration under a temporary home should succeed");
}

#[rstest]
fn pr_metadata_cache_ttl_seconds_loads_from_cli_flag() {
    test_pr_metadata_cache_ttl_seconds_loading(
        None,
        &["--pr-metadata-cache-ttl-seconds", "123"],
        123,
        "expected --pr-metadata-cache-ttl-seconds to set TTL",
    )
    .expect("loading the configuration under a temporary home should succeed");
}

#[rstest]
fn pr_metadata_cache_ttl_seconds_cli_overrides_environment() {
    test_pr_metadata_cache_ttl_seconds_loading(
        Some("3600"),
        &["--pr-metadata-cache-ttl-seconds", "123"],
        123,
        "CLI should override environment for pr_metadata_cache_ttl_seconds",
    )
    .expect("loading the configuration under a temporary home should succeed");
}

#[rstest]
#[case::file_overrides_defaults(
    vec![
        ("defaults", json!({"pr_metadata_cache_ttl_seconds": 10})),
        ("file", json!({"pr_metadata_cache_ttl_seconds": 20}))
    ],
    20
)]
#[case::environment_overrides_file(
    vec![
        ("file", json!({"pr_metadata_cache_ttl_seconds": 10})),
        ("environment", json!({"pr_metadata_cache_ttl_seconds": 20}))
    ],
    20
)]
#[case::cli_overrides_environment(
    vec![
        ("environment", json!({"pr_metadata_cache_ttl_seconds": 10})),
        ("cli", json!({"pr_metadata_cache_ttl_seconds": 20}))
    ],
    20
)]
fn pr_metadata_cache_ttl_seconds_layer_precedence(
    #[case] layers: Vec<(&str, serde_json::Value)>,
    #[case] expected: u64,
) {
    let config = build_config_from_layers(&layers).expect("configuration layers should merge");
    assert_eq!(
        config.pr_metadata_cache_ttl_seconds, expected,
        "pr_metadata_cache_ttl_seconds should follow standard precedence rules"
    );
}
