//! Tests for the repository gateway.

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{ListPullRequestsParams, OctocrabRepositoryGateway, PullRequestState};
use crate::github::error::IntakeError;
use crate::github::gateway::RepositoryGateway;
use crate::github::locator::{PersonalAccessToken, RepositoryLocator};

#[tokio::test]
async fn list_pull_requests_populates_page_info_from_page_response() {
    let server = MockServer::start().await;
    let locator = RepositoryLocator::parse(&format!("{}/owner/repo", server.uri()))
        .expect("should create repository locator");
    let token = PersonalAccessToken::new("valid-token").expect("token should be valid");
    let gateway =
        OctocrabRepositoryGateway::for_token(&token, &locator).expect("should create gateway");

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

    Mock::given(method("GET"))
        .and(path(pulls_path))
        .and(query_param("state", "all"))
        .and(query_param("page", page.to_string()))
        .and(query_param("per_page", per_page.to_string()))
        .respond_with(response)
        .mount(&server)
        .await;

    let params = ListPullRequestsParams {
        state: Some(PullRequestState::All),
        page: Some(page),
        per_page: Some(per_page),
    };
    let result = gateway
        .list_pull_requests(&locator, &params)
        .await
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

#[tokio::test]
async fn list_pull_requests_maps_rate_limit_errors() {
    const EXPECTED_RESET_AT: u64 = 1_700_000_000;

    let server = MockServer::start().await;
    let locator = RepositoryLocator::parse(&format!("{}/owner/repo", server.uri()))
        .expect("should create repository locator");
    let token = PersonalAccessToken::new("valid-token").expect("token should be valid");
    let gateway =
        OctocrabRepositoryGateway::for_token(&token, &locator).expect("should create gateway");

    let pulls_path = "/api/v3/repos/owner/repo/pulls";
    let response = ResponseTemplate::new(403).set_body_json(serde_json::json!({
        "message": "API rate limit exceeded for user",
        "documentation_url": "https://docs.github.com/rest/rate-limit"
    }));

    Mock::given(method("GET"))
        .and(path(pulls_path))
        .and(query_param("state", "open"))
        .and(query_param("page", "1"))
        .and(query_param("per_page", "30"))
        .respond_with(response)
        .mount(&server)
        .await;

    let rate_limit_response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "resources": {
            "core": { "limit": 5000, "used": 5000, "remaining": 0, "reset": EXPECTED_RESET_AT },
            "search": { "limit": 30, "used": 0, "remaining": 30, "reset": EXPECTED_RESET_AT }
        },
        "rate": { "limit": 5000, "used": 5000, "remaining": 0, "reset": EXPECTED_RESET_AT }
    }));
    Mock::given(method("GET"))
        .and(path("/api/v3/rate_limit"))
        .respond_with(rate_limit_response)
        .mount(&server)
        .await;

    let error = gateway
        .list_pull_requests(&locator, &ListPullRequestsParams::default())
        .await
        .expect_err("request should fail");

    match error {
        IntakeError::RateLimitExceeded {
            rate_limit,
            message,
        } => {
            let info = rate_limit.expect("expected rate_limit info to be populated");
            assert_eq!(
                info.reset_at(),
                EXPECTED_RESET_AT,
                "unexpected reset timestamp"
            );
            assert!(
                message.contains("API rate limit exceeded for user"),
                "unexpected message: {message}"
            );
            assert!(
                message.contains(&EXPECTED_RESET_AT.to_string()),
                "expected message to include reset time, got `{message}`"
            );
        }
        other => panic!("expected RateLimitExceeded, got {other:?}"),
    }
}

#[tokio::test]
async fn list_pull_requests_rejects_invalid_pagination_params() {
    let locator = RepositoryLocator::from_owner_repo("owner", "repo")
        .expect("should create repository locator");
    let token = PersonalAccessToken::new("valid-token").expect("token should be valid");
    let gateway =
        OctocrabRepositoryGateway::for_token(&token, &locator).expect("should create gateway");

    let params = ListPullRequestsParams {
        state: Some(PullRequestState::All),
        page: Some(0),
        per_page: Some(0),
    };
    let error = gateway
        .list_pull_requests(&locator, &params)
        .await
        .expect_err("invalid params should fail");

    assert!(
        matches!(error, IntakeError::InvalidPagination { .. }),
        "expected InvalidPagination, got {error:?}"
    );
}

#[tokio::test]
async fn list_pull_requests_rejects_per_page_over_maximum() {
    let locator = RepositoryLocator::from_owner_repo("owner", "repo")
        .expect("should create repository locator");
    let token = PersonalAccessToken::new("valid-token").expect("token should be valid");
    let gateway =
        OctocrabRepositoryGateway::for_token(&token, &locator).expect("should create gateway");

    let params = ListPullRequestsParams {
        state: Some(PullRequestState::All),
        page: Some(1),
        per_page: Some(101),
    };
    let error = gateway
        .list_pull_requests(&locator, &params)
        .await
        .expect_err("invalid per_page should fail");

    assert!(
        matches!(error, IntakeError::InvalidPagination { .. }),
        "expected InvalidPagination, got {error:?}"
    );
}

#[tokio::test]
async fn list_pull_requests_applies_default_query_params() {
    let server = MockServer::start().await;
    let locator = RepositoryLocator::parse(&format!("{}/owner/repo", server.uri()))
        .expect("should create repository locator");
    let token = PersonalAccessToken::new("valid-token").expect("token should be valid");
    let gateway =
        OctocrabRepositoryGateway::for_token(&token, &locator).expect("should create gateway");

    let pulls_path = "/api/v3/repos/owner/repo/pulls";
    let response = ResponseTemplate::new(200).set_body_json(serde_json::json!([]));

    Mock::given(method("GET"))
        .and(path(pulls_path))
        .and(query_param("state", "open"))
        .and(query_param("page", "1"))
        .and(query_param("per_page", "30"))
        .respond_with(response)
        .mount(&server)
        .await;

    let result = gateway
        .list_pull_requests(&locator, &ListPullRequestsParams::default())
        .await
        .expect("request should succeed");

    assert_eq!(result.items.len(), 0, "expected no items");
    assert_eq!(result.page_info.current_page(), 1);
    assert_eq!(result.page_info.per_page(), 30);
    assert!(!result.page_info.has_next());
    assert!(!result.page_info.has_prev());
}
