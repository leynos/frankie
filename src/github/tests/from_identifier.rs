//! Tests for [`PullRequestLocator::from_identifier`].

use rstest::rstest;

use crate::github::error::IntakeError;
use crate::github::locator::PullRequestLocator;
use crate::local::GitHubOrigin;

/// Groups the input and expected values for a single
/// `from_identifier_resolves_pr_number` case.
struct ResolveCase {
    pr_number: &'static str,
    origin: GitHubOrigin,
    expected_number: u64,
    expected_owner: &'static str,
    expected_repo: &'static str,
    expected_api_base: &'static str,
}

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
#[case(ResolveCase {
    pr_number: "42",
    origin: github_com_origin(),
    expected_number: 42,
    expected_owner: "octo",
    expected_repo: "repo",
    expected_api_base: "https://api.github.com/",
})]
#[case(ResolveCase {
    pr_number: "7",
    origin: enterprise_origin(),
    expected_number: 7,
    expected_owner: "corp",
    expected_repo: "internal",
    expected_api_base: "https://ghe.example.com/api/v3",
})]
#[case(ResolveCase {
    pr_number: "3",
    origin: enterprise_origin_with_port(),
    expected_number: 3,
    expected_owner: "corp",
    expected_repo: "internal",
    expected_api_base: "https://ghe.example.com:8443/api/v3",
})]
fn from_identifier_resolves_pr_number(#[case] resolve_case: ResolveCase) {
    let locator = PullRequestLocator::from_identifier(resolve_case.pr_number, &resolve_case.origin)
        .expect("should resolve PR number");

    assert_eq!(
        locator.number().get(),
        resolve_case.expected_number,
        "number mismatch"
    );
    assert_eq!(
        locator.owner().as_str(),
        resolve_case.expected_owner,
        "owner mismatch"
    );
    assert_eq!(
        locator.repository().as_str(),
        resolve_case.expected_repo,
        "repo mismatch"
    );
    assert_eq!(
        locator.api_base().as_str(),
        resolve_case.expected_api_base,
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
