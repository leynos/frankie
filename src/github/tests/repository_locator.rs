//! Tests for [`RepositoryLocator`] construction and validation.

use rstest::rstest;

use crate::github::{IntakeError, RepositoryLocator};

#[rstest]
fn parses_repository_url() {
    let locator = RepositoryLocator::parse("https://github.com/octo/repo")
        .expect("should parse repository URL");
    assert_eq!(locator.owner().as_str(), "octo", "owner mismatch");
    assert_eq!(locator.repository().as_str(), "repo", "repository mismatch");
    assert_eq!(
        locator.api_base().as_str(),
        "https://api.github.com/",
        "api base mismatch"
    );
}

#[rstest]
fn parses_repository_url_with_trailing_path() {
    let locator = RepositoryLocator::parse("https://github.com/octo/repo/pulls")
        .expect("should parse repository URL with trailing path");
    assert_eq!(locator.owner().as_str(), "octo", "owner mismatch");
    assert_eq!(locator.repository().as_str(), "repo", "repository mismatch");
}

#[rstest]
fn parses_enterprise_repository_url() {
    let locator = RepositoryLocator::parse("https://ghe.example.com/foo/bar")
        .expect("should parse enterprise repository URL");
    assert_eq!(
        locator.api_base().as_str(),
        "https://ghe.example.com/api/v3",
        "enterprise api base mismatch"
    );
}

#[rstest]
fn parses_enterprise_repository_url_with_port() {
    let locator = RepositoryLocator::parse("https://ghe.example.com:8443/foo/bar")
        .expect("should parse enterprise repository URL with port");
    assert_eq!(
        locator.api_base().as_str(),
        "https://ghe.example.com:8443/api/v3",
        "enterprise api base should preserve port"
    );
}

#[rstest]
fn from_owner_repo() {
    let locator =
        RepositoryLocator::from_owner_repo("octo", "repo").expect("should create locator");
    assert_eq!(locator.owner().as_str(), "octo", "owner mismatch");
    assert_eq!(locator.repository().as_str(), "repo", "repository mismatch");
    assert_eq!(
        locator.pulls_path(),
        "/repos/octo/repo/pulls",
        "pulls path mismatch"
    );
}

#[rstest]
#[case::empty_owner("", "repo")]
#[case::empty_repo("octo", "")]
fn rejects_empty_segment(#[case] owner: &str, #[case] repo: &str) {
    let result = RepositoryLocator::from_owner_repo(owner, repo);
    assert!(
        matches!(result, Err(IntakeError::MissingPathSegments)),
        "expected MissingPathSegments, got {result:?}"
    );
}
