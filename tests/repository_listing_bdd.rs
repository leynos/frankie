//! Behavioural tests for repository pull request listing.

use frankie::{
    IntakeError, ListPullRequestsParams, OctocrabRepositoryGateway, PaginatedPullRequests,
    PersonalAccessToken, RepositoryIntake, RepositoryLocator,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use serde_json::json;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EXPECTED_RATE_LIMIT_RESET_AT: u64 = 1_700_000_000;

// --- Domain wrapper types to eliminate primitive obsession ---

/// Page number for pagination (1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PageNumber(u32);

impl PageNumber {
    const fn new(value: u32) -> Self {
        Self(value)
    }
    const fn value(self) -> u32 {
        self.0
    }
}

impl FromStr for PageNumber {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u32>().map(Self)
    }
}

impl fmt::Display for PageNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Count of pull requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PullRequestCount(u32);

impl PullRequestCount {
    const fn new(value: u32) -> Self {
        Self(value)
    }
    const fn value(self) -> u32 {
        self.0
    }
}

impl FromStr for PullRequestCount {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u32>().map(Self)
    }
}

impl fmt::Display for PullRequestCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Total number of pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PageCount(u32);

impl PageCount {
    const fn new(value: u32) -> Self {
        Self(value)
    }

    const fn value(self) -> u32 {
        self.0
    }
}

impl FromStr for PageCount {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u32>().map(Self::new)
    }
}

impl fmt::Display for PageCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Rate limit remaining count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RateLimitCount(u32);

impl RateLimitCount {
    const fn new(value: u32) -> Self {
        Self(value)
    }

    const fn value(self) -> u32 {
        self.0
    }
}

impl FromStr for RateLimitCount {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u32>().map(Self::new)
    }
}

impl fmt::Display for RateLimitCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Shared runtime wrapper that can be stored in rstest-bdd Slot.
#[derive(Clone)]
struct SharedRuntime(Rc<RefCell<Runtime>>);

impl SharedRuntime {
    fn new(runtime: Runtime) -> Self {
        Self(Rc::new(RefCell::new(runtime)))
    }

    fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.0.borrow().block_on(future)
    }
}

#[derive(ScenarioState, Default)]
struct ListingState {
    runtime: Slot<SharedRuntime>,
    server: Slot<MockServer>,
    token: Slot<String>,
    page: Slot<u32>,
    result: Slot<PaginatedPullRequests>,
    error: Slot<IntakeError>,
}

#[fixture]
fn listing_state() -> ListingState {
    ListingState::default()
}

/// Ensures the runtime and server are initialised in `ListingState`.
fn ensure_runtime_and_server(listing_state: &ListingState) -> SharedRuntime {
    if listing_state.runtime.with_ref(|_| ()).is_none() {
        let runtime = Runtime::new()
            .unwrap_or_else(|error| panic!("failed to create Tokio runtime: {error}"));
        listing_state.runtime.set(SharedRuntime::new(runtime));
    }

    let shared_runtime = listing_state
        .runtime
        .get()
        .unwrap_or_else(|| panic!("runtime not initialised after set"));

    if listing_state.server.with_ref(|_| ()).is_none() {
        listing_state
            .server
            .set(shared_runtime.block_on(MockServer::start()));
    }

    shared_runtime
}

fn generate_pr_list(
    count: PullRequestCount,
    page: PageNumber,
    per_page: PullRequestCount,
) -> Vec<serde_json::Value> {
    let start = (page.value() - 1) * per_page.value();
    (0..count.value())
        .map(|i| {
            let pr_number = start + i + 1;
            json!({
                "number": pr_number,
                "title": format!("PR #{pr_number}"),
                "state": "open",
                "user": { "login": "contributor" },
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            })
        })
        .collect()
}

#[given("a mock GitHub API server with {count:PullRequestCount} open PRs for owner/repo")]
fn seed_server_with_prs(listing_state: &ListingState, count: PullRequestCount) {
    let runtime = ensure_runtime_and_server(listing_state);

    let prs = generate_pr_list(count, PageNumber::new(1), count);
    let pulls_path = "/api/v3/repos/owner/repo/pulls";

    let mock = Mock::given(method("GET"))
        .and(path(pulls_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&prs));

    listing_state
        .server
        .with_ref(|server| {
            runtime.block_on(mock.mount(server));
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

#[given(
    "a mock GitHub API server with {total:PullRequestCount} PRs across {pages:PageCount} pages for owner/repo"
)]
#[expect(
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    reason = "test data: exact division is intentional for page setup"
)]
fn seed_server_with_paginated_prs(
    listing_state: &ListingState,
    total: PullRequestCount,
    pages: PageCount,
) {
    let runtime = ensure_runtime_and_server(listing_state);
    let per_page = total.value() / pages.value();

    let pulls_path = "/api/v3/repos/owner/repo/pulls";

    for page in 1..=pages.value() {
        let prs = generate_pr_list(
            PullRequestCount::new(per_page),
            PageNumber::new(page),
            PullRequestCount::new(per_page),
        );
        let server_uri = listing_state
            .server
            .with_ref(MockServer::uri)
            .unwrap_or_default();

        let mut response = ResponseTemplate::new(200).set_body_json(&prs);

        // Add Link header for pagination
        let mut links = Vec::new();
        if page < pages.value() {
            links.push(format!(
                "<{server_uri}{pulls_path}?page={}&per_page={per_page}>; rel=\"next\"",
                page + 1
            ));
        }
        if page > 1 {
            links.push(format!(
                "<{server_uri}{pulls_path}?page={}&per_page={per_page}>; rel=\"prev\"",
                page - 1
            ));
        }
        links.push(format!(
            "<{server_uri}{pulls_path}?page={}&per_page={per_page}>; rel=\"last\"",
            pages.value()
        ));

        if !links.is_empty() {
            response = response.insert_header("Link", links.join(", "));
        }

        let mock = Mock::given(method("GET"))
            .and(path(pulls_path))
            .and(query_param("page", page.to_string()))
            .respond_with(response);

        listing_state
            .server
            .with_ref(|server| {
                runtime.block_on(mock.mount(server));
            })
            .unwrap_or_else(|| panic!("mock server not initialised"));
    }
}

#[given(
    "a mock GitHub API server with rate limit headers showing {remaining:RateLimitCount} remaining"
)]
fn seed_server_with_rate_limit_headers(listing_state: &ListingState, remaining: RateLimitCount) {
    let runtime = ensure_runtime_and_server(listing_state);

    let prs = generate_pr_list(
        PullRequestCount::new(10),
        PageNumber::new(1),
        PullRequestCount::new(10),
    );
    let pulls_path = "/api/v3/repos/owner/repo/pulls";

    let response = ResponseTemplate::new(200)
        .set_body_json(&prs)
        .insert_header("X-RateLimit-Limit", "5000")
        .insert_header("X-RateLimit-Remaining", remaining.value().to_string())
        .insert_header(
            "X-RateLimit-Reset",
            EXPECTED_RATE_LIMIT_RESET_AT.to_string(),
        );

    let mock = Mock::given(method("GET"))
        .and(path(pulls_path))
        .respond_with(response);

    listing_state
        .server
        .with_ref(|server| {
            runtime.block_on(mock.mount(server));
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

#[given("a mock GitHub API server returning 403 rate limit exceeded")]
fn seed_server_with_rate_limit_error(listing_state: &ListingState) {
    let runtime = ensure_runtime_and_server(listing_state);

    let pulls_path = "/api/v3/repos/owner/repo/pulls";

    let response = ResponseTemplate::new(403)
        .set_body_json(json!({
            "message": "API rate limit exceeded for user",
            "documentation_url": "https://docs.github.com/rest/rate-limit"
        }))
        .insert_header("X-RateLimit-Limit", "5000")
        .insert_header("X-RateLimit-Remaining", "0")
        .insert_header(
            "X-RateLimit-Reset",
            EXPECTED_RATE_LIMIT_RESET_AT.to_string(),
        );

    let mock = Mock::given(method("GET"))
        .and(path(pulls_path))
        .respond_with(response);

    listing_state
        .server
        .with_ref(|server| {
            runtime.block_on(mock.mount(server));
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));

    let rate_limit_response = ResponseTemplate::new(200).set_body_json(json!({
        "resources": {
            "core": {
                "limit": 5000,
                "used": 5000,
                "remaining": 0,
                "reset": EXPECTED_RATE_LIMIT_RESET_AT
            },
            "search": {
                "limit": 30,
                "used": 0,
                "remaining": 30,
                "reset": EXPECTED_RATE_LIMIT_RESET_AT
            }
        },
        "rate": {
            "limit": 5000,
            "used": 5000,
            "remaining": 0,
            "reset": EXPECTED_RATE_LIMIT_RESET_AT
        }
    }));
    let rate_limit_mock = Mock::given(method("GET"))
        .and(path("/api/v3/rate_limit"))
        .respond_with(rate_limit_response);

    listing_state
        .server
        .with_ref(|server| {
            runtime.block_on(rate_limit_mock.mount(server));
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

#[given("a personal access token {token}")]
fn remember_token(listing_state: &ListingState, token: String) {
    listing_state.token.set(token);
}

#[when("the client lists pull requests for {repo_url} page {page:PageNumber}")]
fn list_pull_requests_with_page(listing_state: &ListingState, repo_url: String, page: PageNumber) {
    let server_url = listing_state
        .server
        .with_ref(MockServer::uri)
        .unwrap_or_else(|| panic!("mock server URL missing"));

    let cleaned_url = repo_url.trim_matches('"');
    let resolved_url = if cleaned_url.contains("://SERVER") {
        cleaned_url
            .replace("https://SERVER", &server_url)
            .replace("http://SERVER", &server_url)
    } else {
        cleaned_url.replace("SERVER", &server_url)
    };

    let locator = RepositoryLocator::parse(&resolved_url)
        .unwrap_or_else(|error| panic!("{resolved_url}: {error}"));

    listing_state.page.set(page.value());

    let runtime = listing_state
        .runtime
        .get()
        .unwrap_or_else(|| panic!("runtime not initialised"));

    let result = runtime.block_on(async {
        let token_value = listing_state.token.get().ok_or(IntakeError::MissingToken)?;
        let token = PersonalAccessToken::new(token_value)?;

        let gateway = OctocrabRepositoryGateway::for_token(&token, &locator)?;
        let intake = RepositoryIntake::new(&gateway);
        let params = ListPullRequestsParams {
            page: Some(page.value()),
            per_page: Some(50),
            ..Default::default()
        };
        intake.list_pull_requests(&locator, &params).await
    });

    match result {
        Ok(prs) => {
            // Clear any previous error.
            let _had_previous_error = listing_state.error.take().is_some();
            listing_state.result.set(prs);
        }
        Err(error) => {
            // Clear any previous result.
            let _had_previous_result = listing_state.result.take().is_some();
            listing_state.error.set(error);
        }
    }
}

#[then("the response includes {count:PullRequestCount} pull requests")]
fn assert_pr_count(listing_state: &ListingState, count: PullRequestCount) {
    let actual = listing_state
        .result
        .with_ref(|result| result.items.len())
        .unwrap_or_else(|| panic!("pull request listing missing"));

    assert_eq!(actual, count.value() as usize, "PR count mismatch");
}

#[then("the current page is {page:PageNumber}")]
fn assert_current_page(listing_state: &ListingState, page: PageNumber) {
    let actual = listing_state
        .result
        .with_ref(|result| result.page_info.current_page())
        .unwrap_or_else(|| panic!("pull request listing missing"));

    assert_eq!(actual, page.value(), "current page mismatch");
}

#[then("the pagination indicates page {page:PageNumber} of {total:PageCount}")]
fn assert_page_of_total(listing_state: &ListingState, page: PageNumber, total: PageCount) {
    let (actual_page, actual_total) = listing_state
        .result
        .with_ref(|result| {
            (
                result.page_info.current_page(),
                result.page_info.total_pages(),
            )
        })
        .unwrap_or_else(|| panic!("pull request listing missing"));

    assert_eq!(actual_page, page.value(), "current page mismatch");
    assert_eq!(actual_total, Some(total.value()), "total pages mismatch");
}

#[then("pagination has next page")]
fn assert_has_next_page(listing_state: &ListingState) {
    let has_next = listing_state
        .result
        .with_ref(|result| result.page_info.has_next())
        .unwrap_or(false);

    assert!(has_next, "expected pagination to have next page");
}

#[then("pagination has previous page")]
fn assert_has_prev_page(listing_state: &ListingState) {
    let has_prev = listing_state
        .result
        .with_ref(|result| result.page_info.has_prev())
        .unwrap_or(false);

    assert!(has_prev, "expected pagination to have previous page");
}

#[then("no error is raised")]
fn assert_no_error(listing_state: &ListingState) {
    let has_error = listing_state.error.with_ref(|_| ()).is_some();
    assert!(!has_error, "expected no error but got one");

    let has_result = listing_state.result.with_ref(|_| ()).is_some();
    assert!(has_result, "expected successful result");
}

#[then("the error indicates rate limit exceeded")]
fn assert_rate_limit_error(listing_state: &ListingState) {
    const EXPECTED_RATE_LIMIT_MESSAGE: &str = "API rate limit exceeded for user";

    let error = listing_state
        .error
        .with_ref(Clone::clone)
        .unwrap_or_else(|| panic!("expected rate limit error"));

    match error {
        IntakeError::RateLimitExceeded {
            rate_limit,
            message,
        } => {
            assert!(
                message.contains(EXPECTED_RATE_LIMIT_MESSAGE),
                "expected rate limit message to contain `{EXPECTED_RATE_LIMIT_MESSAGE}`, got `{message}`"
            );
            assert!(
                rate_limit.is_some(),
                "expected rate_limit to be populated for rate limit errors"
            );
        }
        other => {
            panic!("expected RateLimitExceeded variant, got {other:?}");
        }
    }
}

#[then("the error includes rate limit reset information")]
fn assert_rate_limit_reset_information(listing_state: &ListingState) {
    let error = listing_state
        .error
        .with_ref(Clone::clone)
        .unwrap_or_else(|| panic!("expected rate limit error"));

    match error {
        IntakeError::RateLimitExceeded { rate_limit, .. } => {
            let Some(info) = rate_limit else {
                panic!("expected rate_limit info to be populated")
            };
            assert_eq!(
                info.reset_at(),
                EXPECTED_RATE_LIMIT_RESET_AT,
                "unexpected rate limit reset time"
            );

            let rendered = error.to_string();
            assert!(
                rendered.contains(&EXPECTED_RATE_LIMIT_RESET_AT.to_string()),
                "expected error message to contain reset time, got `{rendered}`"
            );
        }
        other => panic!("expected RateLimitExceeded variant, got {other:?}"),
    }
}

#[scenario(path = "tests/features/repository_listing.feature", index = 0)]
fn list_pull_requests_success(listing_state: ListingState) {
    let _ = listing_state;
}

#[scenario(path = "tests/features/repository_listing.feature", index = 1)]
fn paginate_through_prs(listing_state: ListingState) {
    let _ = listing_state;
}

#[scenario(path = "tests/features/repository_listing.feature", index = 2)]
fn handle_rate_limit_headers(listing_state: ListingState) {
    let _ = listing_state;
}

#[scenario(path = "tests/features/repository_listing.feature", index = 3)]
fn handle_rate_limit_exhaustion(listing_state: ListingState) {
    let _ = listing_state;
}
