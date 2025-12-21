//! Tests for the repository gateway.

type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;

use rstest::{fixture, rstest};
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{ListPullRequestsParams, OctocrabRepositoryGateway, PullRequestState};
use crate::github::error::IntakeError;
use crate::github::gateway::RepositoryGateway;
use crate::github::locator::{PersonalAccessToken, RepositoryLocator};

const EXPECTED_RATE_LIMIT_RESET_AT: u64 = 1_700_000_000;

struct RepositoryGatewayFixture {
    runtime: Runtime,
    server: MockServer,
    locator: RepositoryLocator,
    gateway: OctocrabRepositoryGateway,
}

impl RepositoryGatewayFixture {
    fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }
}

struct LocalGatewayFixture {
    runtime: Runtime,
    locator: RepositoryLocator,
    gateway: OctocrabRepositoryGateway,
}

impl LocalGatewayFixture {
    fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }
}

#[fixture]
fn token() -> FixtureResult<PersonalAccessToken> {
    Ok(PersonalAccessToken::new("valid-token")?)
}

#[fixture]
fn gateway_fixture(
    token: FixtureResult<PersonalAccessToken>,
) -> FixtureResult<RepositoryGatewayFixture> {
    let token_value = token?;
    let runtime = Runtime::new()?;
    let server = runtime.block_on(MockServer::start());
    let locator = RepositoryLocator::parse(&format!("{}/owner/repo", server.uri()))?;
    let gateway = {
        let _guard = runtime.enter();
        OctocrabRepositoryGateway::for_token(&token_value, &locator)?
    };
    Ok(RepositoryGatewayFixture {
        runtime,
        server,
        locator,
        gateway,
    })
}

#[fixture]
fn local_gateway(token: FixtureResult<PersonalAccessToken>) -> FixtureResult<LocalGatewayFixture> {
    let token_value = token?;
    let runtime = Runtime::new()?;
    let locator = RepositoryLocator::from_owner_repo("owner", "repo")?;
    let gateway = {
        let _guard = runtime.enter();
        OctocrabRepositoryGateway::for_token(&token_value, &locator)?
    };
    Ok(LocalGatewayFixture {
        runtime,
        locator,
        gateway,
    })
}

/// Helper to test that invalid pagination parameters are rejected.
fn test_invalid_pagination_params(
    local_gateway: &LocalGatewayFixture,
    params: ListPullRequestsParams,
) {
    let locator = &local_gateway.locator;
    let gateway = &local_gateway.gateway;

    let error = local_gateway
        .block_on(gateway.list_pull_requests(locator, &params))
        .expect_err("invalid params should fail");

    assert!(
        matches!(error, IntakeError::InvalidPagination { .. }),
        "expected InvalidPagination, got {error:?}"
    );
}

#[rstest]
fn list_pull_requests_populates_page_info_from_page_response(
    gateway_fixture: FixtureResult<RepositoryGatewayFixture>,
) {
    let fixture = gateway_fixture.expect("fixture should succeed");
    let server = &fixture.server;
    let locator = &fixture.locator;
    let gateway = &fixture.gateway;

    let pulls_path = "/api/v3/repos/owner/repo/pulls";
    let page = 2_u32;
    let per_page = 50_u8;
    let next_url = format!(
        "{server_uri}{pulls_path}?state=all&page=3&per_page={per_page}",
        server_uri = server.uri()
    );
    let prev_url = format!(
        "{server_uri}{pulls_path}?state=all&page=1&per_page={per_page}",
        server_uri = server.uri()
    );
    let last_url = format!(
        "{server_uri}{pulls_path}?state=all&page=3&per_page={per_page}",
        server_uri = server.uri()
    );
    let link_header = format!(
        "<{next_url}>; rel=\"next\", <{prev_url}>; rel=\"prev\", <{last_url}>; rel=\"last\""
    );

    let response = ResponseTemplate::new(200)
        .set_body_json(serde_json::json!([{
            "number": 1,
            "title": "First PR",
            "state": "open",
            "user": { "login": "octocat" },
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-02T00:00:00Z"
        }]))
        .insert_header("Link", link_header);

    fixture.block_on(
        Mock::given(method("GET"))
            .and(path(pulls_path))
            .and(query_param("state", "all"))
            .and(query_param("page", page.to_string()))
            .and(query_param("per_page", per_page.to_string()))
            .respond_with(response)
            .mount(server),
    );

    let params = ListPullRequestsParams {
        state: Some(PullRequestState::All),
        page: Some(page),
        per_page: Some(per_page),
    };
    let result = fixture
        .block_on(gateway.list_pull_requests(locator, &params))
        .expect("request should succeed");

    assert_eq!(result.items.len(), 1, "expected one item");
    let first = result.items.first().expect("should have first item");
    assert_eq!(first.number, 1);
    assert_eq!(first.author.as_deref(), Some("octocat"));

    let info = result.page_info;
    assert_eq!(info.current_page(), 2);
    assert_eq!(info.per_page(), 50);
    assert_eq!(info.total_pages(), Some(3));
    assert!(info.has_next());
    assert!(info.has_prev());
}

#[rstest]
fn list_pull_requests_maps_rate_limit_errors(
    gateway_fixture: FixtureResult<RepositoryGatewayFixture>,
) {
    let fixture = gateway_fixture.expect("fixture should succeed");
    let server = &fixture.server;
    let locator = &fixture.locator;
    let gateway = &fixture.gateway;

    let pulls_path = "/api/v3/repos/owner/repo/pulls";
    let response = ResponseTemplate::new(403).set_body_json(serde_json::json!({
        "message": "API rate limit exceeded for user",
        "documentation_url": "https://docs.github.com/rest/rate-limit"
    }));

    fixture.block_on(
        Mock::given(method("GET"))
            .and(path(pulls_path))
            .and(query_param("state", "open"))
            .and(query_param("page", "1"))
            .and(query_param("per_page", "30"))
            .respond_with(response)
            .mount(server),
    );

    let rate_limit_response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "resources": {
            "core": { "limit": 5000, "used": 5000, "remaining": 0, "reset": EXPECTED_RATE_LIMIT_RESET_AT },
            "search": { "limit": 30, "used": 0, "remaining": 30, "reset": EXPECTED_RATE_LIMIT_RESET_AT }
        },
        "rate": { "limit": 5000, "used": 5000, "remaining": 0, "reset": EXPECTED_RATE_LIMIT_RESET_AT }
    }));
    fixture.block_on(
        Mock::given(method("GET"))
            .and(path("/api/v3/rate_limit"))
            .respond_with(rate_limit_response)
            .mount(server),
    );

    let error = fixture
        .block_on(gateway.list_pull_requests(locator, &ListPullRequestsParams::default()))
        .expect_err("request should fail");

    match error {
        IntakeError::RateLimitExceeded {
            rate_limit,
            message,
        } => {
            let info = rate_limit.expect("expected rate_limit info to be populated");
            assert_eq!(
                info.reset_at(),
                EXPECTED_RATE_LIMIT_RESET_AT,
                "unexpected reset timestamp"
            );
            assert!(
                message.contains("API rate limit exceeded for user"),
                "unexpected message: {message}"
            );
            assert!(
                message.contains(&EXPECTED_RATE_LIMIT_RESET_AT.to_string()),
                "expected message to include reset time, got `{message}`"
            );
        }
        other => panic!("expected RateLimitExceeded, got {other:?}"),
    }
}

#[rstest]
fn list_pull_requests_rejects_invalid_pagination_params(
    local_gateway: FixtureResult<LocalGatewayFixture>,
) {
    let fixture = local_gateway.expect("fixture should succeed");
    let params = ListPullRequestsParams {
        state: Some(PullRequestState::All),
        page: Some(0),
        per_page: Some(0),
    };
    test_invalid_pagination_params(&fixture, params);
}

#[rstest]
fn list_pull_requests_rejects_per_page_over_maximum(
    local_gateway: FixtureResult<LocalGatewayFixture>,
) {
    let fixture = local_gateway.expect("fixture should succeed");
    let params = ListPullRequestsParams {
        state: Some(PullRequestState::All),
        page: Some(1),
        per_page: Some(101),
    };
    test_invalid_pagination_params(&fixture, params);
}

#[rstest]
fn list_pull_requests_applies_default_query_params(
    gateway_fixture: FixtureResult<RepositoryGatewayFixture>,
) {
    let fixture = gateway_fixture.expect("fixture should succeed");
    let server = &fixture.server;
    let locator = &fixture.locator;
    let gateway = &fixture.gateway;

    let pulls_path = "/api/v3/repos/owner/repo/pulls";
    let response = ResponseTemplate::new(200).set_body_json(serde_json::json!([]));

    fixture.block_on(
        Mock::given(method("GET"))
            .and(path(pulls_path))
            .and(query_param("state", "open"))
            .and(query_param("page", "1"))
            .and(query_param("per_page", "30"))
            .respond_with(response)
            .mount(server),
    );

    let result = fixture
        .block_on(gateway.list_pull_requests(locator, &ListPullRequestsParams::default()))
        .expect("request should succeed");

    assert_eq!(result.items.len(), 0, "expected no items");
    assert_eq!(result.page_info.current_page(), 1);
    assert_eq!(result.page_info.per_page(), 30);
    assert!(!result.page_info.has_next());
    assert!(!result.page_info.has_prev());
}
