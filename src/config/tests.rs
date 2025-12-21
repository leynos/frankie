//! Unit tests for configuration loading and precedence.

use ortho_config::{MergeComposer, OrthoConfig};
use rstest::rstest;
use serde_json::{Value, json};

use super::{FrankieConfig, OperationMode};

/// Applies a configuration layer to the composer based on the layer type.
fn apply_layer(composer: &mut MergeComposer, layer_type: &str, value: Value) {
    match layer_type {
        "defaults" => composer.push_defaults(value),
        "file" => composer.push_file(value, None),
        "environment" => composer.push_environment(value),
        "cli" => composer.push_cli(value),
        _ => panic!("unknown layer type: {layer_type}"),
    }
}

/// Helper to compose a [`FrankieConfig`] from a sequence of `(layer_type, value)` pairs.
fn build_config_from_layers(layers: &[(&str, Value)]) -> FrankieConfig {
    let mut composer = MergeComposer::new();

    for (layer_type, value) in layers {
        apply_layer(&mut composer, layer_type, value.clone());
    }

    FrankieConfig::merge_from_layers(composer.layers()).expect("merge should succeed")
}

/// Helper to test `pr_metadata_cache_ttl_seconds` loading from environment and/or CLI.
fn test_pr_metadata_cache_ttl_seconds_loading(
    env_ttl: Option<&str>,
    cli_args: &[&str],
    expected_ttl: u64,
    description: &str,
) {
    let temp_dir = tempfile::TempDir::new().expect("temp dir should be created");
    let home = temp_dir.path().to_string_lossy().to_string();

    let _guard = env_lock::lock_env([
        ("FRANKIE_PR_METADATA_CACHE_TTL_SECONDS", env_ttl),
        ("HOME", Some(home.as_str())),
        ("XDG_CONFIG_HOME", Some(home.as_str())),
    ]);

    let mut args: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("frankie")];
    args.extend(cli_args.iter().map(std::ffi::OsString::from));

    let config = FrankieConfig::load_from_iter(args).expect("config should load");

    assert_eq!(
        config.pr_metadata_cache_ttl_seconds, expected_ttl,
        "{description}"
    );
}

#[rstest]
#[case::file_overrides_defaults(
    vec![("defaults", json!({"pr_url": "default-url"})), ("file", json!({"pr_url": "file-url"}))],
    "pr_url",
    "file-url",
    "file should override default"
)]
#[case::environment_overrides_file(
    vec![("file", json!({"token": "file-token"})), ("environment", json!({"token": "env-token"}))],
    "token",
    "env-token",
    "environment should override file"
)]
#[case::cli_overrides_environment(
    vec![("environment", json!({"pr_url": "env-url"})), ("cli", json!({"pr_url": "cli-url"}))],
    "pr_url",
    "cli-url",
    "CLI should override environment"
)]
#[case::database_url_defaults_file_env_cli(
    vec![
        ("defaults", json!({"database_url": "default-db"})),
        ("file", json!({"database_url": "file-db"})),
        ("environment", json!({"database_url": "env-db"})),
        ("cli", json!({"database_url": "cli-db"}))
    ],
    "database_url",
    "cli-db",
    "CLI should win for database_url"
)]
fn test_layer_precedence(
    #[case] layers: Vec<(&str, Value)>,
    #[case] field: &str,
    #[case] expected: &str,
    #[case] message: &str,
) {
    let mut composer = MergeComposer::new();

    for (layer_type, value) in layers {
        apply_layer(&mut composer, layer_type, value);
    }

    let config = FrankieConfig::merge_from_layers(composer.layers()).expect("merge should succeed");

    let actual = match field {
        "pr_url" => config.pr_url.as_deref(),
        "token" => config.token.as_deref(),
        "database_url" => config.database_url.as_deref(),
        _ => panic!("unknown field: {field}"),
    };

    assert_eq!(actual, Some(expected), "{message}");
}

#[rstest]
fn defaults_are_none_when_no_sources_provided() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({"pr_url": null, "token": null}));

    let config = FrankieConfig::merge_from_layers(composer.layers())
        .expect("merge should succeed with empty defaults");

    assert!(config.pr_url.is_none(), "pr_url should be None");
    assert!(config.token.is_none(), "token should be None");
    assert!(config.database_url.is_none(), "database_url should be None");
    assert!(
        !config.migrate_db,
        "migrate_db should default to false when unset"
    );
    assert_eq!(
        config.pr_metadata_cache_ttl_seconds, 86_400,
        "pr_metadata_cache_ttl_seconds should default to 24 hours when unset"
    );
}

#[rstest]
fn full_precedence_chain() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(
        json!({"pr_url": "default", "token": "default-token", "database_url": "default-db"}),
    );
    composer.push_file(
        json!({"pr_url": "file", "token": "file-token", "database_url": "file-db"}),
        None,
    );
    composer.push_environment(json!({"pr_url": "env", "database_url": "env-db"}));
    composer.push_cli(json!({"pr_url": "cli", "database_url": "cli-db"}));

    let config = FrankieConfig::merge_from_layers(composer.layers()).expect("merge should succeed");

    assert_eq!(config.pr_url.as_deref(), Some("cli"), "CLI wins for pr_url");
    assert_eq!(
        config.token.as_deref(),
        Some("file-token"),
        "file wins for token (no env/cli override)"
    );
    assert_eq!(
        config.database_url.as_deref(),
        Some("cli-db"),
        "CLI wins for database_url"
    );
}

#[rstest]
fn partial_overrides_preserve_lower_values() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({"pr_url": "default-url", "token": "default-token"}));
    composer.push_cli(json!({"pr_url": "cli-url"}));

    let config = FrankieConfig::merge_from_layers(composer.layers()).expect("merge should succeed");

    assert_eq!(
        config.pr_url.as_deref(),
        Some("cli-url"),
        "CLI should override pr_url"
    );
    assert_eq!(
        config.token.as_deref(),
        Some("default-token"),
        "default token should be preserved"
    );
}

#[rstest]
fn database_url_cli_overrides_environment() {
    let config = build_config_from_layers(&[
        ("environment", json!({"database_url": "env-only"})),
        ("cli", json!({"database_url": "cli-only"})),
    ]);

    assert_eq!(
        config.database_url.as_deref(),
        Some("cli-only"),
        "CLI should override environment for database_url"
    );
}

#[rstest]
fn migrate_db_layer_precedence_defaults_file_environment_cli() {
    let config = build_config_from_layers(&[
        ("defaults", json!({"migrate_db": false})),
        ("file", json!({"migrate_db": true})),
        ("environment", json!({"migrate_db": false})),
        ("cli", json!({"migrate_db": true})),
    ]);

    assert!(config.migrate_db, "CLI layer should win for migrate_db");
}

#[rstest]
fn database_url_and_migrate_db_defaults_when_unset() {
    let config = build_config_from_layers(&[(
        "defaults",
        json!({
            "pr_url": null,
            "token": null,
            "owner": null,
            "repo": null,
            "database_url": null,
            "migrate_db": false
        }),
    )]);

    assert!(
        config.database_url.is_none(),
        "database_url should remain None when not provided by any layer"
    );
    assert!(
        !config.migrate_db,
        "migrate_db should default to false when unset"
    );
}

#[rstest]
#[case::pr_url(
    FrankieConfig { pr_url: Some("https://github.com/owner/repo/pull/1".to_owned()), ..Default::default() },
    "https://github.com/owner/repo/pull/1",
    false
)]
#[case::token(
    FrankieConfig { token: Some("my-token".to_owned()), ..Default::default() },
    "my-token",
    true
)]
fn returns_value_when_field_present(
    #[case] config: FrankieConfig,
    #[case] expected: &str,
    #[case] is_token: bool,
) {
    if is_token {
        let result = config.resolve_token();
        assert_eq!(
            result.ok(),
            Some(expected.to_owned()),
            "should return the token"
        );
    } else {
        let result = config.require_pr_url();
        assert_eq!(result.ok(), Some(expected), "should return the URL");
    }
}

#[rstest]
#[case::pr_url(false)]
#[case::token(true)]
fn returns_error_when_field_none(#[case] is_token: bool) {
    let config = FrankieConfig::default();

    if is_token {
        // Lock and clear GITHUB_TOKEN to ensure test isolation
        let _guard = env_lock::lock_env([("GITHUB_TOKEN", None::<&str>)]);
        let result = config.resolve_token();
        assert!(result.is_err(), "should return error when token is None");
    } else {
        let result = config.require_pr_url();
        assert!(result.is_err(), "should return error when pr_url is None");
    }
}

#[rstest]
fn operation_mode_single_pr_when_pr_url_present() {
    let config = FrankieConfig {
        pr_url: Some("https://github.com/owner/repo/pull/1".to_owned()),
        ..Default::default()
    };

    assert_eq!(
        config.operation_mode(),
        OperationMode::SinglePullRequest,
        "should be SinglePullRequest when pr_url is set"
    );
}

#[rstest]
fn operation_mode_repository_listing_when_owner_and_repo_present() {
    let config = FrankieConfig {
        owner: Some("octocat".to_owned()),
        repo: Some("hello-world".to_owned()),
        ..Default::default()
    };

    assert_eq!(
        config.operation_mode(),
        OperationMode::RepositoryListing,
        "should be RepositoryListing when owner and repo are set"
    );
}

#[rstest]
fn operation_mode_interactive_when_no_fields_set() {
    let config = FrankieConfig::default();

    assert_eq!(
        config.operation_mode(),
        OperationMode::Interactive,
        "should be Interactive when no fields are set"
    );
}

#[rstest]
fn operation_mode_ignores_database_fields() {
    let config = FrankieConfig {
        database_url: Some("frankie.sqlite".to_owned()),
        migrate_db: true,
        ..Default::default()
    };

    assert_eq!(
        config.operation_mode(),
        OperationMode::Interactive,
        "database fields should not affect operation mode"
    );
}

#[rstest]
fn pr_url_takes_precedence_over_owner_repo() {
    let config = FrankieConfig {
        pr_url: Some("https://github.com/owner/repo/pull/1".to_owned()),
        owner: Some("octocat".to_owned()),
        repo: Some("hello-world".to_owned()),
        ..Default::default()
    };

    assert_eq!(
        config.operation_mode(),
        OperationMode::SinglePullRequest,
        "pr_url should take precedence over owner/repo"
    );
}

#[rstest]
fn require_repository_info_returns_error_when_owner_missing() {
    let config = FrankieConfig {
        repo: Some("hello-world".to_owned()),
        ..Default::default()
    };

    let result = config.require_repository_info();
    assert!(result.is_err(), "should return error when owner is missing");
}

#[rstest]
fn require_repository_info_returns_error_when_repo_missing() {
    let config = FrankieConfig {
        owner: Some("octocat".to_owned()),
        ..Default::default()
    };

    let result = config.require_repository_info();
    assert!(result.is_err(), "should return error when repo is missing");
}

#[rstest]
fn require_repository_info_returns_values_when_present() {
    let config = FrankieConfig {
        owner: Some("octocat".to_owned()),
        repo: Some("hello-world".to_owned()),
        ..Default::default()
    };

    let result = config.require_repository_info();
    assert_eq!(
        result.ok(),
        Some(("octocat", "hello-world")),
        "should return owner and repo"
    );
}

#[rstest]
fn resolve_token_ignores_database_fields() {
    let _guard = env_lock::lock_env([("GITHUB_TOKEN", Some("legacy-token"))]);
    let config = FrankieConfig {
        database_url: Some("frankie.sqlite".to_owned()),
        migrate_db: true,
        ..Default::default()
    };

    assert_eq!(
        config.resolve_token().ok(),
        Some("legacy-token".to_owned()),
        "token resolution should not be affected by database fields"
    );
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
    );
}

#[rstest]
fn pr_metadata_cache_ttl_seconds_loads_from_cli_flag() {
    test_pr_metadata_cache_ttl_seconds_loading(
        None,
        &["--pr-metadata-cache-ttl-seconds", "123"],
        123,
        "expected --pr-metadata-cache-ttl-seconds to set TTL",
    );
}

#[rstest]
fn pr_metadata_cache_ttl_seconds_cli_overrides_environment() {
    test_pr_metadata_cache_ttl_seconds_loading(
        Some("3600"),
        &["--pr-metadata-cache-ttl-seconds", "123"],
        123,
        "CLI should override environment for pr_metadata_cache_ttl_seconds",
    );
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
    #[case] layers: Vec<(&str, Value)>,
    #[case] expected: u64,
) {
    let config = build_config_from_layers(&layers);
    assert_eq!(
        config.pr_metadata_cache_ttl_seconds, expected,
        "pr_metadata_cache_ttl_seconds should follow standard precedence rules"
    );
}
