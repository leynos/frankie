//! Unit tests for the GitHub intake module.

use mockall::predicate::{always, function};
use rstest::rstest;

use super::{
    IntakeError, ListPullRequestsParams, MockPullRequestGateway, MockRepositoryGateway, PageInfo,
    PaginatedPullRequests, PersonalAccessToken, PullRequestComment, PullRequestDetails,
    PullRequestIntake, PullRequestLocator, PullRequestMetadata, PullRequestState,
    PullRequestSummary, RateLimitInfo, RepositoryIntake, RepositoryLocator,
};

fn sample_locator() -> PullRequestLocator {
    PullRequestLocator::parse("https://github.com/octo/repo/pull/4")
        .expect("sample locator should parse")
}

#[rstest]
fn parses_standard_github_url_segments() {
    let locator = PullRequestLocator::parse("https://github.com/octo/repo/pull/12/files")
        .expect("should parse standard GitHub URL");
    assert_eq!(locator.owner().as_str(), "octo", "owner mismatch");
    assert_eq!(locator.repository().as_str(), "repo", "repository mismatch");
    assert_eq!(locator.number().get(), 12_u64, "number mismatch");
}

#[rstest]
fn parses_standard_github_url_api_base() {
    let locator = PullRequestLocator::parse("https://github.com/octo/repo/pull/12/files")
        .expect("should parse standard GitHub URL");
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

/// Sets up a mock gateway for pull request intake tests.
fn setup_pull_request_gateway() -> MockPullRequestGateway {
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

    gateway
}

#[tokio::test]
async fn aggregates_metadata_from_gateway() {
    let locator = sample_locator();
    let gateway = setup_pull_request_gateway();

    let intake = PullRequestIntake::new(&gateway);
    let PullRequestDetails { metadata, .. } =
        intake.load(&locator).await.expect("intake should succeed");

    assert_eq!(metadata.number, 4, "number mismatch");
    assert_eq!(metadata.title, Some(String::from("demo")), "title mismatch");
    assert_eq!(
        metadata.author,
        Some(String::from("octocat")),
        "author mismatch"
    );
    assert_eq!(metadata.state, Some(String::from("open")), "state mismatch");
}

#[tokio::test]
async fn aggregates_comments_list_from_gateway() {
    let locator = sample_locator();
    let gateway = setup_pull_request_gateway();

    let intake = PullRequestIntake::new(&gateway);
    let PullRequestDetails { comments, .. } =
        intake.load(&locator).await.expect("intake should succeed");

    assert_eq!(comments.len(), 2, "comment count mismatch");
    assert_eq!(
        comments.first().and_then(|c| c.body.clone()),
        Some(String::from("first")),
        "first comment body mismatch"
    );
    assert_eq!(
        comments.get(1).and_then(|c| c.body.clone()),
        Some(String::from("second")),
        "second comment body mismatch"
    );
}

mod from_identifier;

// --- RepositoryLocator tests ---

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
fn repository_locator_from_owner_repo() {
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
fn repository_locator_rejects_empty_owner() {
    let result = RepositoryLocator::from_owner_repo("", "repo");
    assert!(
        matches!(result, Err(IntakeError::MissingPathSegments)),
        "expected MissingPathSegments for empty owner, got {result:?}"
    );
}

#[rstest]
fn repository_locator_rejects_empty_repo() {
    let result = RepositoryLocator::from_owner_repo("octo", "");
    assert!(
        matches!(result, Err(IntakeError::MissingPathSegments)),
        "expected MissingPathSegments for empty repo, got {result:?}"
    );
}

// --- PageInfo tests ---

#[derive(Debug, Clone, Copy)]
enum PageInfoPositionCase {
    First,
    Middle,
    Last,
}

#[rstest]
#[case::first(PageInfoPositionCase::First)]
#[case::middle(PageInfoPositionCase::Middle)]
#[case::last(PageInfoPositionCase::Last)]
fn page_info_position_behaviour(#[case] case: PageInfoPositionCase) {
    let (current_page, has_next, has_prev) = match case {
        PageInfoPositionCase::First => (1, true, false),
        PageInfoPositionCase::Middle => (2, true, true),
        PageInfoPositionCase::Last => (5, false, true),
    };

    let info = PageInfo::builder(current_page, 50)
        .total_pages(Some(5))
        .has_next(has_next)
        .has_prev(has_prev)
        .build();

    assert_eq!(info.has_next(), has_next, "unexpected has_next");
    assert_eq!(info.has_prev(), has_prev, "unexpected has_prev");

    let is_first_page = matches!(case, PageInfoPositionCase::First);
    let is_last_page = matches!(case, PageInfoPositionCase::Last);
    assert_eq!(
        info.is_first_page(),
        is_first_page,
        "unexpected is_first_page"
    );
    assert_eq!(info.is_last_page(), is_last_page, "unexpected is_last_page");
}

#[rstest]
fn page_info_accessors() {
    let info = PageInfo::builder(3, 25)
        .total_pages(Some(10))
        .has_next(true)
        .has_prev(true)
        .build();
    assert_eq!(info.current_page(), 3, "current page mismatch");
    assert_eq!(info.per_page(), 25, "per page mismatch");
    assert_eq!(info.total_pages(), Some(10), "total pages mismatch");
}

// --- RateLimitInfo tests ---

const EXPECTED_RATE_LIMIT_RESET_AT: u64 = 1_700_000_000;

#[rstest]
fn rate_limit_exhausted() {
    let info = RateLimitInfo::new(5000, 0, EXPECTED_RATE_LIMIT_RESET_AT);
    assert!(info.is_exhausted(), "should be exhausted with 0 remaining");
}

#[rstest]
fn rate_limit_not_exhausted() {
    let info = RateLimitInfo::new(5000, 100, EXPECTED_RATE_LIMIT_RESET_AT);
    assert!(
        !info.is_exhausted(),
        "should not be exhausted with 100 remaining"
    );
}

#[rstest]
fn rate_limit_accessors() {
    let info = RateLimitInfo::new(5000, 4999, EXPECTED_RATE_LIMIT_RESET_AT);
    assert_eq!(info.limit(), 5000, "limit mismatch");
    assert_eq!(info.remaining(), 4999, "remaining mismatch");
    assert_eq!(
        info.reset_at(),
        EXPECTED_RATE_LIMIT_RESET_AT,
        "reset_at mismatch"
    );
}

// --- RepositoryIntake tests ---

/// Sets up a mock gateway for repository intake tests with sample pull requests.
fn setup_repository_gateway() -> MockRepositoryGateway {
    let mut gateway = MockRepositoryGateway::new();

    gateway
        .expect_list_pull_requests()
        .with(
            always(),
            function(|params: &ListPullRequestsParams| {
                assert_eq!(
                    params.state,
                    Some(PullRequestState::Open),
                    "unexpected default state"
                );
                assert_eq!(params.page, Some(1), "unexpected default page");
                assert_eq!(params.per_page, Some(30), "unexpected default per_page");
                true
            }),
        )
        .times(1)
        .returning(|_, _| {
            Ok(PaginatedPullRequests {
                items: vec![
                    PullRequestSummary {
                        number: 1,
                        title: Some(String::from("First PR")),
                        state: Some(String::from("open")),
                        author: Some(String::from("alice")),
                        created_at: None,
                        updated_at: None,
                    },
                    PullRequestSummary {
                        number: 2,
                        title: Some(String::from("Second PR")),
                        state: Some(String::from("closed")),
                        author: Some(String::from("bob")),
                        created_at: None,
                        updated_at: None,
                    },
                ],
                page_info: PageInfo::builder(1, 30).total_pages(Some(1)).build(),
                rate_limit: None,
            })
        });

    gateway
}

#[tokio::test]
async fn lists_pull_requests_returns_items() {
    let locator =
        RepositoryLocator::from_owner_repo("octo", "repo").expect("should create locator");
    let gateway = setup_repository_gateway();

    let intake = RepositoryIntake::new(&gateway);
    let params = ListPullRequestsParams::default();
    let result = intake
        .list_pull_requests(&locator, &params)
        .await
        .expect("listing should succeed");

    assert_eq!(result.items.len(), 2, "item count mismatch");
    assert_eq!(
        result.items.first().map(|p| p.number),
        Some(1),
        "first PR number mismatch"
    );
    assert_eq!(
        result.items.get(1).map(|p| p.number),
        Some(2),
        "second PR number mismatch"
    );
}

#[tokio::test]
async fn lists_pull_requests_returns_page_info() {
    let locator =
        RepositoryLocator::from_owner_repo("octo", "repo").expect("should create locator");
    let gateway = setup_repository_gateway();

    let intake = RepositoryIntake::new(&gateway);
    let params = ListPullRequestsParams::default();
    let result = intake
        .list_pull_requests(&locator, &params)
        .await
        .expect("listing should succeed");

    assert_eq!(
        result.page_info.current_page(),
        1,
        "current page should be 1"
    );
    assert_eq!(result.page_info.per_page(), 30, "per page should be 30");
    assert!(!result.page_info.has_next(), "should not have next page");
}

#[tokio::test]
async fn lists_pull_requests_with_rate_limit_info() {
    let locator =
        RepositoryLocator::from_owner_repo("octo", "repo").expect("should create locator");
    let mut gateway = MockRepositoryGateway::new();

    gateway
        .expect_list_pull_requests()
        .with(
            always(),
            function(|params: &ListPullRequestsParams| {
                assert_eq!(
                    params.state,
                    Some(PullRequestState::Open),
                    "unexpected default state"
                );
                assert_eq!(params.page, Some(1), "unexpected default page");
                assert_eq!(params.per_page, Some(30), "unexpected default per_page");
                true
            }),
        )
        .times(1)
        .returning(|_, _| {
            Ok(PaginatedPullRequests {
                items: vec![],
                page_info: PageInfo::builder(1, 30).total_pages(Some(1)).build(),
                rate_limit: Some(RateLimitInfo::new(5000, 4950, EXPECTED_RATE_LIMIT_RESET_AT)),
            })
        });

    let intake = RepositoryIntake::new(&gateway);
    let params = ListPullRequestsParams::default();
    let result = intake
        .list_pull_requests(&locator, &params)
        .await
        .expect("listing should succeed");

    let rate_limit = result.rate_limit.expect("should have rate limit info");
    assert_eq!(rate_limit.remaining(), 4950, "remaining mismatch");
}
