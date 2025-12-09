//! Behavioural tests for CLI configuration loading.

use frankie::{FrankieConfig, IntakeError};
use ortho_config::MergeComposer;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use serde_json::{Value, json};

/// State for CLI configuration scenarios.
///
/// Uses JSON values to represent configuration layers since `MergeComposer`
/// doesn't implement Clone. The composer is built fresh in `build_config`.
#[derive(ScenarioState, Default)]
struct ConfigState {
    defaults_layer: Slot<Value>,
    env_layer: Slot<Value>,
    cli_layer: Slot<Value>,
    config: Slot<FrankieConfig>,
    pr_url_error: Slot<IntakeError>,
    token_error: Slot<IntakeError>,
    github_token_backup: Slot<Option<String>>,
}

#[fixture]
fn config_state() -> ConfigState {
    ConfigState::default()
}

/// Builds and stores the configuration from the accumulated layers.
fn build_config(state: &ConfigState) {
    let mut composer = MergeComposer::new();

    // Always push base defaults with explicit null values to ensure merge succeeds.
    // The struct needs at least one valid layer with its shape.
    let base_defaults = json!({"pr_url": null, "token": null});
    let defaults = state
        .defaults_layer
        .get()
        .unwrap_or_else(|| base_defaults.clone());
    let merged_defaults = merge_json(base_defaults, defaults);
    composer.push_defaults(merged_defaults);

    if let Some(env) = state.env_layer.get() {
        composer.push_environment(env);
    }

    if let Some(cli) = state.cli_layer.get() {
        composer.push_cli(cli);
    }

    match FrankieConfig::merge_from_layers(composer.layers()) {
        Ok(config) => {
            state.config.set(config);
        }
        Err(error) => {
            panic!("failed to merge configuration: {error}");
        }
    }
}

/// Merges two JSON values, with `overlay` values taking precedence over `base`.
fn merge_json(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                base_map.insert(key, value);
            }
            Value::Object(base_map)
        }
        (_, other) => other,
    }
}

// --- Given steps ---

#[given("a configuration with no pr_url set")]
fn no_pr_url_set(config_state: &ConfigState) {
    config_state.defaults_layer.set(json!({}));
}

#[given("a configuration with no token set")]
fn no_token_set(config_state: &ConfigState) {
    config_state.defaults_layer.set(json!({}));
}

#[given("a configuration with environment pr_url {url}")]
fn env_pr_url_set(config_state: &ConfigState, url: String) {
    let url_clean = url.trim_matches('"');
    config_state.env_layer.set(json!({"pr_url": url_clean}));
}

#[given("a configuration with environment token {token}")]
fn env_token_set(config_state: &ConfigState, token: String) {
    let token_clean = token.trim_matches('"');
    config_state.env_layer.set(json!({"token": token_clean}));
}

#[given("no GITHUB_TOKEN environment variable")]
fn no_github_token_env(config_state: &ConfigState) {
    // Backup and remove GITHUB_TOKEN
    let backup = std::env::var("GITHUB_TOKEN").ok();
    config_state.github_token_backup.set(backup);
    // SAFETY: This test runs in isolation and does not rely on this
    // environment variable being set for any concurrent thread.
    unsafe { std::env::remove_var("GITHUB_TOKEN") };
}

#[given("a GITHUB_TOKEN environment variable set to {token}")]
fn github_token_env_set(config_state: &ConfigState, token: String) {
    // Backup current value
    let backup = std::env::var("GITHUB_TOKEN").ok();
    config_state.github_token_backup.set(backup);

    let token_clean = token.trim_matches('"');
    // SAFETY: This test runs in isolation and does not rely on this
    // environment variable being set for any concurrent thread.
    unsafe { std::env::set_var("GITHUB_TOKEN", token_clean) };
}

// --- When steps ---

#[when("the CLI receives pr_url {url}")]
fn cli_receives_pr_url(config_state: &ConfigState, url: String) {
    let url_clean = url.trim_matches('"');
    config_state.cli_layer.set(json!({"pr_url": url_clean}));
    build_config(config_state);
}

#[when("the CLI receives token {token}")]
fn cli_receives_token(config_state: &ConfigState, token: String) {
    let token_clean = token.trim_matches('"');
    config_state.cli_layer.set(json!({"token": token_clean}));
    build_config(config_state);
}

#[when("the CLI receives no pr_url")]
fn cli_receives_no_pr_url(config_state: &ConfigState) {
    build_config(config_state);
}

#[when("the CLI receives no token")]
fn cli_receives_no_token(config_state: &ConfigState) {
    build_config(config_state);
}

// --- Then steps ---

#[then("the configuration pr_url is {expected}")]
fn assert_pr_url(config_state: &ConfigState, expected: String) {
    let expected_clean = expected.trim_matches('"');

    let config = config_state
        .config
        .get()
        .unwrap_or_else(|| panic!("configuration not built"));

    assert_eq!(
        config.pr_url.as_deref(),
        Some(expected_clean),
        "pr_url mismatch"
    );
}

#[then("the resolved token is {expected}")]
fn assert_resolved_token(config_state: &ConfigState, expected: String) {
    let expected_clean = expected.trim_matches('"');

    let config = config_state
        .config
        .get()
        .unwrap_or_else(|| panic!("configuration not built"));

    let resolved = config
        .resolve_token()
        .unwrap_or_else(|error| panic!("token resolution failed: {error}"));

    // Restore GITHUB_TOKEN if we backed it up
    if let Some(Some(value)) = config_state.github_token_backup.take() {
        // SAFETY: This test runs in isolation.
        unsafe { std::env::set_var("GITHUB_TOKEN", value) };
    }

    assert_eq!(resolved, expected_clean, "resolved token mismatch");
}

#[then("requiring pr_url returns an error")]
fn assert_pr_url_error(config_state: &ConfigState) {
    let config = config_state
        .config
        .get()
        .unwrap_or_else(|| panic!("configuration not built"));

    let result = config.require_pr_url();
    assert!(result.is_err(), "expected pr_url to return error");

    if let Err(error) = result {
        config_state.pr_url_error.set(error);
    }
}

#[then("resolving token returns an error")]
fn assert_token_error(config_state: &ConfigState) {
    let config = config_state
        .config
        .get()
        .unwrap_or_else(|| panic!("configuration not built"));

    let result = config.resolve_token();

    // Restore GITHUB_TOKEN if we backed it up
    if let Some(Some(value)) = config_state.github_token_backup.take() {
        // SAFETY: This test runs in isolation.
        unsafe { std::env::set_var("GITHUB_TOKEN", value) };
    }

    assert!(result.is_err(), "expected token resolution to return error");

    if let Err(error) = result {
        config_state.token_error.set(error);
    }
}

// --- Scenario bindings ---

#[scenario(path = "tests/features/cli_config.feature", index = 0)]
fn load_pr_url_from_cli(config_state: ConfigState) {
    let _ = config_state;
}

#[scenario(path = "tests/features/cli_config.feature", index = 1)]
fn load_token_from_cli(config_state: ConfigState) {
    let _ = config_state;
}

#[scenario(path = "tests/features/cli_config.feature", index = 2)]
fn cli_pr_url_overrides_env(config_state: ConfigState) {
    let _ = config_state;
}

#[scenario(path = "tests/features/cli_config.feature", index = 3)]
fn cli_token_overrides_env(config_state: ConfigState) {
    let _ = config_state;
}

#[scenario(path = "tests/features/cli_config.feature", index = 4)]
fn env_token_used_when_cli_not_provided(config_state: ConfigState) {
    let _ = config_state;
}

#[scenario(path = "tests/features/cli_config.feature", index = 5)]
fn missing_pr_url_error(config_state: ConfigState) {
    let _ = config_state;
}

#[scenario(path = "tests/features/cli_config.feature", index = 6)]
fn missing_token_error(config_state: ConfigState) {
    let _ = config_state;
}

#[scenario(path = "tests/features/cli_config.feature", index = 7)]
fn github_token_fallback(config_state: ConfigState) {
    let _ = config_state;
}

#[scenario(path = "tests/features/cli_config.feature", index = 8)]
fn frankie_token_precedence(config_state: ConfigState) {
    let _ = config_state;
}
