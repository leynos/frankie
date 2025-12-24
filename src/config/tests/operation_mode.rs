//! Tests for operation mode determination.

use rstest::rstest;

use crate::FrankieConfig;
use crate::config::OperationMode;

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
