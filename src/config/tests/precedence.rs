//! Tests for configuration layer precedence.

use ortho_config::MergeComposer;
use rstest::rstest;
use serde_json::{Value, json};

use super::helpers::{apply_layer, build_config_from_layers};
use crate::FrankieConfig;

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
