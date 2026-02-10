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

struct ExpectedLocator {
    number: u64,
    owner: &'static str,
    repo: &'static str,
    api_base: &'static str,
}

#[rstest]
#[case::github_com(
    github_com_origin(),
    "42",
    ExpectedLocator { number: 42, owner: "octo", repo: "repo", api_base: "https://api.github.com/" },
)]
#[case::enterprise(
    enterprise_origin(),
    "7",
    ExpectedLocator { number: 7, owner: "corp", repo: "internal", api_base: "https://ghe.example.com/api/v3" },
)]
#[case::enterprise_with_port(
    enterprise_origin_with_port(),
    "3",
    ExpectedLocator { number: 3, owner: "corp", repo: "internal", api_base: "https://ghe.example.com:8443/api/v3" },
)]
fn resolves_pr_number(
    #[case] origin: GitHubOrigin,
    #[case] identifier: &str,
    #[case] expected: ExpectedLocator,
) {
    let locator = PullRequestLocator::from_identifier(identifier, &origin).expect("should resolve");

    assert_eq!(locator.number().get(), expected.number, "number mismatch");
    assert_eq!(locator.owner().as_str(), expected.owner, "owner mismatch");
    assert_eq!(
        locator.repository().as_str(),
        expected.repo,
        "repo mismatch"
    );
    assert_eq!(
        locator.api_base().as_str(),
        expected.api_base,
        "api base mismatch"
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

#[derive(Debug, Clone, Copy)]
enum ExpectedError {
    InvalidPullRequestNumber,
    Configuration,
}

#[rstest]
#[case::zero("0", ExpectedError::InvalidPullRequestNumber)]
#[case::non_numeric("abc", ExpectedError::Configuration)]
#[case::negative("-5", ExpectedError::Configuration)]
fn rejects_invalid_identifier(#[case] identifier: &str, #[case] expected: ExpectedError) {
    let origin = github_com_origin();
    let result = PullRequestLocator::from_identifier(identifier, &origin);

    let is_expected = match expected {
        ExpectedError::InvalidPullRequestNumber => {
            matches!(result, Err(IntakeError::InvalidPullRequestNumber))
        }
        ExpectedError::Configuration => matches!(result, Err(IntakeError::Configuration { .. })),
    };
    assert!(is_expected, "expected {expected:?}, got {result:?}");
}
