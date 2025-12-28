//! Tests for the review comments gateway.

type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;

use rstest::{fixture, rstest};
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::OctocrabReviewCommentGateway;
use crate::github::error::IntakeError;
use crate::github::gateway::ReviewCommentGateway;
use crate::github::locator::{PersonalAccessToken, PullRequestLocator};

const EXPECTED_RATE_LIMIT_RESET_AT: u64 = 1_700_000_000;

trait BlocksOnRuntime {
    fn runtime(&self) -> &Runtime;

    fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime().block_on(future)
    }
}

struct ReviewCommentGatewayFixture {
    runtime: Runtime,
    server: MockServer,
    locator: PullRequestLocator,
    gateway: OctocrabReviewCommentGateway,
}

impl BlocksOnRuntime for ReviewCommentGatewayFixture {
    fn runtime(&self) -> &Runtime {
        &self.runtime
    }
}

#[fixture]
fn token() -> FixtureResult<PersonalAccessToken> {
    Ok(PersonalAccessToken::new("valid-token")?)
}

#[fixture]
fn gateway_fixture(
    token: FixtureResult<PersonalAccessToken>,
) -> FixtureResult<ReviewCommentGatewayFixture> {
    let token_value = token?;
    let runtime = Runtime::new()?;
    let server = runtime.block_on(MockServer::start());
    let locator = PullRequestLocator::parse(&format!("{}/owner/repo/pull/42", server.uri()))?;
    let _guard = runtime.enter();
    let gateway =
        OctocrabReviewCommentGateway::new(&token_value, &format!("{}/api/v3", server.uri()))?;
    Ok(ReviewCommentGatewayFixture {
        runtime,
        server,
        locator,
        gateway,
    })
}

#[rstest]
fn list_review_comments_returns_comments(
    gateway_fixture: FixtureResult<ReviewCommentGatewayFixture>,
) {
    let fixture = gateway_fixture.expect("fixture should succeed");
    let server = &fixture.server;
    let locator = &fixture.locator;
    let gateway = &fixture.gateway;

    let comments_path = "/api/v3/repos/owner/repo/pulls/42/comments";
    let response = ResponseTemplate::new(200).set_body_json(serde_json::json!([
        {
            "id": 1,
            "body": "First comment",
            "user": { "login": "alice" },
            "path": "src/main.rs",
            "line": 10,
            "original_line": 10,
            "diff_hunk": "@@ -1,5 +1,6 @@",
            "commit_id": "abc123",
            "in_reply_to_id": null,
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-02T00:00:00Z"
        },
        {
            "id": 2,
            "body": "Reply to first",
            "user": { "login": "bob" },
            "path": "src/main.rs",
            "line": 10,
            "original_line": 10,
            "diff_hunk": "@@ -1,5 +1,6 @@",
            "commit_id": "abc123",
            "in_reply_to_id": 1,
            "created_at": "2025-01-01T01:00:00Z",
            "updated_at": "2025-01-02T01:00:00Z"
        }
    ]));

    fixture.block_on(
        Mock::given(method("GET"))
            .and(path(comments_path))
            .respond_with(response)
            .mount(server),
    );

    let result = fixture
        .block_on(gateway.list_review_comments(locator))
        .expect("request should succeed");

    assert_eq!(result.len(), 2, "expected two comments");

    let first = result.first().expect("should have first comment");
    assert_eq!(first.id, 1);
    assert_eq!(first.body.as_deref(), Some("First comment"));
    assert_eq!(first.author.as_deref(), Some("alice"));
    assert_eq!(first.file_path.as_deref(), Some("src/main.rs"));
    assert_eq!(first.line_number, Some(10));
    assert!(first.in_reply_to_id.is_none());

    let second = result.get(1).expect("should have second comment");
    assert_eq!(second.id, 2);
    assert_eq!(second.in_reply_to_id, Some(1));
}

#[rstest]
fn list_review_comments_maps_rate_limit_errors(
    gateway_fixture: FixtureResult<ReviewCommentGatewayFixture>,
) {
    let fixture = gateway_fixture.expect("fixture should succeed");
    let server = &fixture.server;
    let locator = &fixture.locator;
    let gateway = &fixture.gateway;

    let comments_path = "/api/v3/repos/owner/repo/pulls/42/comments";
    let response = ResponseTemplate::new(403).set_body_json(serde_json::json!({
        "message": "API rate limit exceeded for user",
        "documentation_url": "https://docs.github.com/rest/rate-limit"
    }));

    fixture.block_on(
        Mock::given(method("GET"))
            .and(path(comments_path))
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
        .block_on(gateway.list_review_comments(locator))
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
fn list_review_comments_maps_auth_errors(
    gateway_fixture: FixtureResult<ReviewCommentGatewayFixture>,
) {
    let fixture = gateway_fixture.expect("fixture should succeed");
    let server = &fixture.server;
    let locator = &fixture.locator;
    let gateway = &fixture.gateway;

    let comments_path = "/api/v3/repos/owner/repo/pulls/42/comments";
    let response = ResponseTemplate::new(401).set_body_json(serde_json::json!({
        "message": "Bad credentials",
        "documentation_url": "https://docs.github.com/rest"
    }));

    fixture.block_on(
        Mock::given(method("GET"))
            .and(path(comments_path))
            .respond_with(response)
            .mount(server),
    );

    let error = fixture
        .block_on(gateway.list_review_comments(locator))
        .expect_err("request should fail");

    match error {
        IntakeError::Authentication { message } => {
            assert!(
                message.contains("Bad credentials"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected Authentication error, got {other:?}"),
    }
}

#[rstest]
fn list_review_comments_returns_empty_list(
    gateway_fixture: FixtureResult<ReviewCommentGatewayFixture>,
) {
    let fixture = gateway_fixture.expect("fixture should succeed");
    let server = &fixture.server;
    let locator = &fixture.locator;
    let gateway = &fixture.gateway;

    let comments_path = "/api/v3/repos/owner/repo/pulls/42/comments";
    let response = ResponseTemplate::new(200).set_body_json(serde_json::json!([]));

    fixture.block_on(
        Mock::given(method("GET"))
            .and(path(comments_path))
            .respond_with(response)
            .mount(server),
    );

    let result = fixture
        .block_on(gateway.list_review_comments(locator))
        .expect("request should succeed");

    assert!(result.is_empty(), "expected empty list");
}
