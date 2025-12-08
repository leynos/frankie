//! Unit tests for the GitHub intake module.

use mockall::predicate::always;
use rstest::rstest;

use super::{
    IntakeError, MockPullRequestGateway, PersonalAccessToken, PullRequestComment,
    PullRequestDetails, PullRequestIntake, PullRequestLocator, PullRequestMetadata,
};

fn sample_locator() -> PullRequestLocator {
    PullRequestLocator::parse("https://github.com/octo/repo/pull/4")
        .expect("sample locator should parse")
}

#[rstest]
fn parses_standard_github_url() {
    let locator = PullRequestLocator::parse("https://github.com/octo/repo/pull/12/files")
        .expect("should parse standard GitHub URL");
    assert_eq!(locator.owner().as_str(), "octo", "owner mismatch");
    assert_eq!(locator.repository().as_str(), "repo", "repository mismatch");
    assert_eq!(locator.number().get(), 12_u64, "number mismatch");
    assert_eq!(
        locator.api_base().as_str(),
        "https://api.github.com/",
        "api base mismatch"
    );
}

#[rstest]
fn parses_enterprise_url() {
    let locator = PullRequestLocator::parse("https://ghe.example.com/foo/bar/pull/7")
        .expect("should parse enterprise URL");
    assert_eq!(
        locator.api_base().as_str(),
        "https://ghe.example.com/api/v3",
        "enterprise api base mismatch"
    );
}

#[rstest]
fn rejects_missing_number() {
    let result = PullRequestLocator::parse("https://github.com/octo/repo/pull/");
    assert!(
        matches!(result, Err(IntakeError::MissingPathSegments)),
        "expected MissingPathSegments, got {result:?}"
    );
}

#[rstest]
fn rejects_non_numeric_number() {
    let result = PullRequestLocator::parse("https://github.com/octo/repo/pull/not-a-number");
    assert!(
        matches!(result, Err(IntakeError::InvalidPullRequestNumber)),
        "expected InvalidPullRequestNumber, got {result:?}"
    );
}

#[rstest]
fn rejects_zero_number() {
    let result = PullRequestLocator::parse("https://github.com/octo/repo/pull/0");
    assert!(
        matches!(result, Err(IntakeError::InvalidPullRequestNumber)),
        "expected InvalidPullRequestNumber for zero, got {result:?}"
    );
}

#[rstest]
fn rejects_issues_path() {
    let result = PullRequestLocator::parse("https://github.com/octo/repo/issues/4");
    assert!(
        matches!(result, Err(IntakeError::MissingPathSegments)),
        "expected MissingPathSegments for issues path, got {result:?}"
    );
}

#[rstest]
fn rejects_pulls_collection_path() {
    let result = PullRequestLocator::parse("https://github.com/octo/repo/pulls/4");
    assert!(
        matches!(result, Err(IntakeError::MissingPathSegments)),
        "expected MissingPathSegments for pulls path, got {result:?}"
    );
}

#[rstest]
fn rejects_invalid_url() {
    let result = PullRequestLocator::parse("octo/repo/pull/4");
    assert!(
        matches!(result, Err(IntakeError::InvalidUrl(_))),
        "expected InvalidUrl for malformed URL, got {result:?}"
    );
}

#[rstest]
fn rejects_empty_token() {
    let result = PersonalAccessToken::new(String::new());
    assert!(
        matches!(result, Err(IntakeError::MissingToken)),
        "expected MissingToken, got {result:?}"
    );
}

#[tokio::test]
async fn aggregates_comments_from_gateway() {
    let locator = sample_locator();
    let mut gateway = MockPullRequestGateway::new();

    gateway
        .expect_pull_request()
        .with(always())
        .times(1)
        .returning(|_| {
            Ok(PullRequestMetadata {
                number: 4,
                title: Some(String::from("demo")),
                state: Some(String::from("open")),
                html_url: None,
                author: Some(String::from("octocat")),
            })
        });

    gateway
        .expect_pull_request_comments()
        .with(always())
        .times(1)
        .returning(|_| {
            Ok(vec![
                PullRequestComment {
                    id: 1,
                    body: Some(String::from("first")),
                    author: Some(String::from("a")),
                },
                PullRequestComment {
                    id: 2,
                    body: Some(String::from("second")),
                    author: Some(String::from("b")),
                },
            ])
        });

    let intake = PullRequestIntake::new(&gateway);
    let PullRequestDetails { metadata, comments } =
        intake.load(&locator).await.expect("intake should succeed");

    assert_eq!(metadata.number, 4, "number mismatch");
    assert_eq!(metadata.title, Some(String::from("demo")), "title mismatch");
    assert_eq!(
        metadata.author,
        Some(String::from("octocat")),
        "author mismatch"
    );
    assert_eq!(metadata.state, Some(String::from("open")), "state mismatch");
    assert_eq!(comments.len(), 2, "comment count mismatch");
}
