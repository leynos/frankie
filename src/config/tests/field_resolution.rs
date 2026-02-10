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
