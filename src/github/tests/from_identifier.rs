//! Tests for [`PullRequestLocator::from_identifier`].

use rstest::rstest;

use crate::github::error::IntakeError;
use crate::github::locator::PullRequestLocator;
use crate::local::GitHubOrigin;

fn github_com_origin() -> GitHubOrigin {
    GitHubOrigin::GitHubCom {
        owner: "octo".to_owned(),
        repository: "repo".to_owned(),
    }
}

fn enterprise_origin() -> GitHubOrigin {
    GitHubOrigin::Enterprise {
        host: "ghe.example.com".to_owned(),
        port: None,
        owner: "corp".to_owned(),
        repository: "internal".to_owned(),
    }
}

fn enterprise_origin_with_port() -> GitHubOrigin {
    GitHubOrigin::Enterprise {
        host: "ghe.example.com".to_owned(),
        port: Some(8443),
        owner: "corp".to_owned(),
        repository: "internal".to_owned(),
    }
}

#[rstest]
fn from_identifier_resolves_pr_number_for_github_com() {
    let origin = github_com_origin();
    let locator =
        PullRequestLocator::from_identifier("42", &origin).expect("should resolve PR number");

    assert_eq!(locator.number().get(), 42, "number mismatch");
    assert_eq!(locator.owner().as_str(), "octo", "owner mismatch");
    assert_eq!(locator.repository().as_str(), "repo", "repo mismatch");
    assert_eq!(
        locator.api_base().as_str(),
        "https://api.github.com/",
        "api base mismatch"
    );
}

#[rstest]
fn from_identifier_resolves_pr_number_for_enterprise() {
    let origin = enterprise_origin();
    let locator = PullRequestLocator::from_identifier("7", &origin)
        .expect("should resolve PR number for enterprise");

    assert_eq!(locator.number().get(), 7, "number mismatch");
    assert_eq!(locator.owner().as_str(), "corp", "owner mismatch");
    assert_eq!(locator.repository().as_str(), "internal", "repo mismatch");
    assert_eq!(
        locator.api_base().as_str(),
        "https://ghe.example.com/api/v3",
        "enterprise api base mismatch"
    );
}

#[rstest]
fn from_identifier_resolves_pr_number_for_enterprise_with_port() {
    let origin = enterprise_origin_with_port();
    let locator = PullRequestLocator::from_identifier("3", &origin)
        .expect("should resolve PR number for enterprise with port");

    assert_eq!(locator.number().get(), 3, "number mismatch");
    assert_eq!(locator.owner().as_str(), "corp", "owner mismatch");
    assert_eq!(locator.repository().as_str(), "internal", "repo mismatch");
    assert_eq!(
        locator.api_base().as_str(),
        "https://ghe.example.com:8443/api/v3",
        "enterprise api base should preserve port"
    );
}

#[rstest]
fn from_identifier_delegates_url_to_parse() {
    let origin = github_com_origin();
    let url = "https://github.com/other/project/pull/99";

    let locator =
        PullRequestLocator::from_identifier(url, &origin).expect("should delegate URL to parse");

    assert_eq!(locator.number().get(), 99, "number mismatch");
    assert_eq!(locator.owner().as_str(), "other", "owner mismatch");
    assert_eq!(locator.repository().as_str(), "project", "repo mismatch");
}

#[rstest]
fn from_identifier_rejects_zero() {
    let origin = github_com_origin();
    let result = PullRequestLocator::from_identifier("0", &origin);

    assert!(
        matches!(result, Err(IntakeError::InvalidPullRequestNumber)),
        "should reject zero, got {result:?}"
    );
}

#[rstest]
fn from_identifier_rejects_non_numeric() {
    let origin = github_com_origin();
    let result = PullRequestLocator::from_identifier("abc", &origin);

    assert!(
        matches!(result, Err(IntakeError::Configuration { .. })),
        "should reject non-numeric, got {result:?}"
    );
}

#[rstest]
fn from_identifier_rejects_negative() {
    let origin = github_com_origin();
    let result = PullRequestLocator::from_identifier("-5", &origin);

    // Starts with '-' but contains no "://", so treated as a number parse
    assert!(
        matches!(result, Err(IntakeError::Configuration { .. })),
        "should reject negative, got {result:?}"
    );
}
