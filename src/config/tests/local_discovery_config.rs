//! Tests for `no_local_discovery` configuration loading and precedence.

use ortho_config::OrthoConfig;
use rstest::rstest;
use serde_json::json;

use super::helpers::build_config_from_layers;
use crate::FrankieConfig;

/// Helper to test `no_local_discovery` loading from CLI.
///
/// Note: `ortho_config` does not support loading boolean values from environment
/// variables, so only CLI flag and config file are tested for real loading.
fn test_no_local_discovery_loading(cli_args: &[&str], expected: bool, description: &str) {
    let temp_dir = tempfile::TempDir::new().expect("temp dir should be created");
    let home = temp_dir.path().to_string_lossy().to_string();

    let _guard = env_lock::lock_env([
        ("HOME", Some(home.as_str())),
        ("XDG_CONFIG_HOME", Some(home.as_str())),
    ]);

    let mut args: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("frankie")];
    args.extend(cli_args.iter().map(std::ffi::OsString::from));

    let config = FrankieConfig::load_from_iter(args).expect("config should load");

    assert_eq!(config.no_local_discovery, expected, "{description}");
}

#[rstest]
fn no_local_discovery_defaults_to_false() {
    let config = FrankieConfig::default();
    assert!(
        !config.no_local_discovery,
        "no_local_discovery should default to false"
    );
}

#[rstest]
#[case::file_overrides_defaults(
    vec![
        ("defaults", json!({"no_local_discovery": false})),
        ("file", json!({"no_local_discovery": true}))
    ],
    true
)]
#[case::environment_overrides_file(
    vec![
        ("file", json!({"no_local_discovery": true})),
        ("environment", json!({"no_local_discovery": false}))
    ],
    false
)]
#[case::cli_overrides_environment(
    vec![
        ("environment", json!({"no_local_discovery": false})),
        ("cli", json!({"no_local_discovery": true}))
    ],
    true
)]
fn no_local_discovery_layer_precedence(
    #[case] layers: Vec<(&str, serde_json::Value)>,
    #[case] expected: bool,
) {
    let config = build_config_from_layers(&layers);
    assert_eq!(
        config.no_local_discovery, expected,
        "no_local_discovery should follow standard precedence rules"
    );
}

#[rstest]
fn no_local_discovery_loads_from_cli_flag() {
    test_no_local_discovery_loading(
        &["--no-local-discovery"],
        true,
        "expected --no-local-discovery to set flag",
    );
}

#[rstest]
fn no_local_discovery_absent_flag_defaults_to_false() {
    test_no_local_discovery_loading(&[], false, "missing --no-local-discovery should be false");
}
