//! Tests for field resolution methods (`resolve_token`, `require_pr_url`,
//! `require_repository_info`).

use rstest::rstest;

use crate::FrankieConfig;

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
