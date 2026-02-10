//! Tests for [`PullRequestLocator::from_identifier`].

// rstest macro expansion duplicates parameters into generated test functions,
// triggering `too_many_arguments` on the expanded code. A function-level
// `#[expect]` is not propagated by rstest, so we scope this to the module.
#![expect(
    clippy::too_many_arguments,
    reason = "rstest #[case] expansion creates functions with the full parameter list"
)]

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
#[case(
    "42",
    github_com_origin(),
    42,
    "octo",
    "repo",
    "https://api.github.com/"
)]
#[case(
    "7",
    enterprise_origin(),
    7,
    "corp",
    "internal",
    "https://ghe.example.com/api/v3"
)]
#[case(
    "3",
    enterprise_origin_with_port(),
    3,
    "corp",
    "internal",
    "https://ghe.example.com:8443/api/v3"
)]
fn from_identifier_resolves_pr_number(
    #[case] pr_number: &str,
    #[case] origin: GitHubOrigin,
    #[case] expected_number: u64,
    #[case] expected_owner: &str,
    #[case] expected_repo: &str,
    #[case] expected_api_base: &str,
) {
    let locator =
        PullRequestLocator::from_identifier(pr_number, &origin).expect("should resolve PR number");

    assert_eq!(locator.number().get(), expected_number, "number mismatch");
    assert_eq!(locator.owner().as_str(), expected_owner, "owner mismatch");
    assert_eq!(
        locator.repository().as_str(),
        expected_repo,
        "repo mismatch"
    );
    assert_eq!(
        locator.api_base().as_str(),
        expected_api_base,
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
