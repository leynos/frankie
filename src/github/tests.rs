//! Unit tests for the GitHub intake module.

use std::fmt::Debug;

use mockall::predicate::always;
use rstest::rstest;

use super::{
    IntakeError, MockPullRequestGateway, PersonalAccessToken, PullRequestComment,
    PullRequestDetails, PullRequestIntake, PullRequestLocator, PullRequestMetadata,
};

fn sample_locator() -> Result<PullRequestLocator, IntakeError> {
    PullRequestLocator::parse("https://github.com/octo/repo/pull/4")
}

fn ensure_eq<T>(actual: &T, expected: &T, label: &str) -> Result<(), IntakeError>
where
    T: PartialEq + Debug,
{
    if actual == expected {
        Ok(())
    } else {
        Err(IntakeError::Api {
            message: format!("{label} mismatch: expected {expected:?}, got {actual:?}"),
        })
    }
}

#[rstest]
fn parses_standard_github_url() -> Result<(), IntakeError> {
    let locator = PullRequestLocator::parse("https://github.com/octo/repo/pull/12/files")?;
    ensure_eq(&locator.owner().as_str(), &"octo", "owner")?;
    ensure_eq(&locator.repository().as_str(), &"repo", "repository")?;
    ensure_eq(&locator.number().get(), &12_u64, "number")?;
    ensure_eq(
        &locator.api_base().as_str(),
        &"https://api.github.com/",
        "api base",
    )?;
    Ok(())
}

#[rstest]
fn parses_enterprise_url() -> Result<(), IntakeError> {
    let locator = PullRequestLocator::parse("https://ghe.example.com/foo/bar/pull/7")?;
    ensure_eq(
        &locator.api_base().as_str(),
        &"https://ghe.example.com/api/v3",
        "enterprise api base",
    )?;
    Ok(())
}

#[rstest]
fn rejects_missing_number() -> Result<(), IntakeError> {
    let result = PullRequestLocator::parse("https://github.com/octo/repo/pull/");
    match result {
        Err(IntakeError::MissingPathSegments) => Ok(()),
        other => Err(IntakeError::Api {
            message: format!("expected missing path segments, got {other:?}"),
        }),
    }
}

#[rstest]
fn rejects_empty_token() -> Result<(), IntakeError> {
    let result = PersonalAccessToken::new(String::new());
    match result {
        Err(IntakeError::MissingToken) => Ok(()),
        other => Err(IntakeError::Api {
            message: format!("expected missing token error, got {other:?}"),
        }),
    }
}

#[tokio::test]
async fn aggregates_comments_from_gateway() -> Result<(), IntakeError> {
    let locator = sample_locator()?;
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
    let PullRequestDetails { comments, .. } = intake.load(&locator).await?;
    ensure_eq(&comments.len(), &2_usize, "comment count")
}
