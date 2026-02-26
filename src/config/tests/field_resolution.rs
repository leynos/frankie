//! Tests for field resolution methods (`resolve_token`, `require_pr_url`,
//! `require_repository_info`, `set_pr_identifier`, `pr_identifier`).

use rstest::rstest;

use crate::FrankieConfig;

#[rstest]
fn resolve_token_returns_value_when_present() {
    let config = FrankieConfig {
        token: Some("my-token".to_owned()),
        ..Default::default()
    };

    let result = config.resolve_token();
    assert_eq!(
        result.ok(),
        Some("my-token".to_owned()),
        "should return the token"
    );
}

#[rstest]
fn require_pr_url_returns_value_when_present() {
    let config = FrankieConfig {
        pr_url: Some("https://github.com/owner/repo/pull/1".to_owned()),
        ..Default::default()
    };

    let result = config.require_pr_url();
    assert_eq!(
        result.ok(),
        Some("https://github.com/owner/repo/pull/1"),
        "should return the URL"
    );
}

#[rstest]
fn resolve_token_returns_error_when_none() {
    // Lock and clear GITHUB_TOKEN to ensure test isolation
    let _guard = env_lock::lock_env([("GITHUB_TOKEN", None::<&str>)]);
    let config = FrankieConfig::default();

    let result = config.resolve_token();
    assert!(result.is_err(), "should return error when token is None");
}

#[rstest]
fn require_pr_url_returns_error_when_none() {
    let config = FrankieConfig::default();

    let result = config.require_pr_url();
    assert!(result.is_err(), "should return error when pr_url is None");
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
fn default_pr_identifier_is_none() {
    let config = FrankieConfig::default();

    assert_eq!(
        config.pr_identifier(),
        None,
        "default config should have no pr_identifier"
    );
}

#[rstest]
fn set_pr_identifier_populates_accessor() {
    let mut config = FrankieConfig::default();
    config.set_pr_identifier("42".to_owned());

    assert_eq!(
        config.pr_identifier(),
        Some("42"),
        "pr_identifier should return the value set via set_pr_identifier"
    );
}

#[rstest]
fn default_repo_path_is_none() {
    let config = FrankieConfig::default();

    assert_eq!(
        config.repo_path, None,
        "default config should have no repo_path"
    );
}

#[rstest]
fn repo_path_can_be_set() {
    let config = FrankieConfig {
        repo_path: Some("/path/to/repo".to_owned()),
        ..Default::default()
    };

    assert_eq!(
        config.repo_path.as_deref(),
        Some("/path/to/repo"),
        "repo_path should return the configured value"
    );
}

#[rstest]
fn value_flags_includes_repo_path() {
    assert!(
        FrankieConfig::VALUE_FLAGS.contains(&"--repo-path"),
        "VALUE_FLAGS should include --repo-path"
    );
}

#[rstest]
fn value_flags_include_reply_drafting_flags() {
    assert!(
        FrankieConfig::VALUE_FLAGS.contains(&"--reply-max-length"),
        "VALUE_FLAGS should include --reply-max-length"
    );
    assert!(
        FrankieConfig::VALUE_FLAGS.contains(&"--reply-templates"),
        "VALUE_FLAGS should include --reply-templates"
    );
}

#[rstest]
fn value_flags_include_ai_rewrite_flags() {
    assert!(
        FrankieConfig::VALUE_FLAGS.contains(&"--ai-rewrite-mode"),
        "VALUE_FLAGS should include --ai-rewrite-mode"
    );
    assert!(
        FrankieConfig::VALUE_FLAGS.contains(&"--ai-rewrite-text"),
        "VALUE_FLAGS should include --ai-rewrite-text"
    );
    assert!(
        FrankieConfig::VALUE_FLAGS.contains(&"--ai-base-url"),
        "VALUE_FLAGS should include --ai-base-url"
    );
    assert!(
        FrankieConfig::VALUE_FLAGS.contains(&"--ai-model"),
        "VALUE_FLAGS should include --ai-model"
    );
    assert!(
        FrankieConfig::VALUE_FLAGS.contains(&"--ai-api-key"),
        "VALUE_FLAGS should include --ai-api-key"
    );
    assert!(
        FrankieConfig::VALUE_FLAGS.contains(&"--ai-timeout-seconds"),
        "VALUE_FLAGS should include --ai-timeout-seconds"
    );
}

#[rstest]
fn resolve_ai_api_key_prefers_config_value() {
    let _guard = env_lock::lock_env([("OPENAI_API_KEY", Some("env-key"))]);
    let config = FrankieConfig {
        ai_api_key: Some("config-key".to_owned()),
        ..Default::default()
    };

    assert_eq!(config.resolve_ai_api_key(), Some("config-key".to_owned()));
}

#[rstest]
fn resolve_ai_api_key_falls_back_to_environment() {
    let _guard = env_lock::lock_env([("OPENAI_API_KEY", Some("env-key"))]);
    let config = FrankieConfig::default();

    assert_eq!(config.resolve_ai_api_key(), Some("env-key".to_owned()));
}

#[rstest]
fn reply_drafting_defaults_are_present() {
    let config = FrankieConfig::default();

    assert_eq!(
        config.reply_max_length, 500,
        "reply_max_length should default to 500"
    );
    assert!(
        !config.reply_templates.is_empty(),
        "reply_templates should include defaults"
    );
}
