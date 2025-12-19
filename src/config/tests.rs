//! Unit tests for configuration loading and precedence.

use ortho_config::MergeComposer;
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
}

#[rstest]
fn full_precedence_chain() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({"pr_url": "default", "token": "default-token"}));
    composer.push_file(json!({"pr_url": "file", "token": "file-token"}), None);
    composer.push_environment(json!({"pr_url": "env"}));
    composer.push_cli(json!({"pr_url": "cli"}));

    let config = FrankieConfig::merge_from_layers(composer.layers()).expect("merge should succeed");

    assert_eq!(config.pr_url.as_deref(), Some("cli"), "CLI wins for pr_url");
    assert_eq!(
        config.token.as_deref(),
        Some("file-token"),
        "file wins for token (no env/cli override)"
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
