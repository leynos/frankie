//! Repository-level gateway for listing pull requests.
//!
//! This module contains the Octocrab-backed repository gateway and its tests.

use async_trait::async_trait;
use octocrab::{Octocrab, Page};

use crate::github::error::IntakeError;
use crate::github::locator::{PersonalAccessToken, RepositoryLocator};
use crate::github::models::{ApiPullRequestSummary, PullRequestSummary};
use crate::github::pagination::PageInfo;
use crate::github::rate_limit::RateLimitInfo;

use super::RepositoryGateway;
use super::client::build_octocrab_client;
use super::error_mapping::{is_rate_limit_error, map_octocrab_error};

mod types;

pub use types::{ListPullRequestsParams, PaginatedPullRequests, PullRequestState};

/// Octocrab-backed repository gateway.
pub struct OctocrabRepositoryGateway {
    client: Octocrab,
}

impl OctocrabRepositoryGateway {
    /// Creates a new gateway from an Octocrab client.
    #[must_use]
    pub const fn new(client: Octocrab) -> Self {
        Self { client }
    }

    /// Builds an Octocrab client for the given token and repository locator.
    ///
    /// # Errors
    ///
    /// Returns `IntakeError::InvalidUrl` when the base URI cannot be parsed or
    /// `IntakeError::Api` when Octocrab fails to construct a client.
    pub fn for_token(
        token: &PersonalAccessToken,
        locator: &RepositoryLocator,
    ) -> Result<Self, IntakeError> {
        let octocrab = build_octocrab_client(token, locator.api_base().as_str())?;
        Ok(Self::new(octocrab))
    }
}

#[async_trait]
impl RepositoryGateway for OctocrabRepositoryGateway {
    async fn list_pull_requests(
        &self,
        locator: &RepositoryLocator,
        params: &ListPullRequestsParams,
    ) -> Result<PaginatedPullRequests, IntakeError> {
        let state = params.state.unwrap_or_default();
        let page = params.page.unwrap_or(1);
        let per_page = params.per_page.unwrap_or(30);

        validate_pagination_params(page, per_page)?;

        let page_str = page.to_string();
        let per_page_str = per_page.to_string();

        let query_params = [
            ("state", state.as_str()),
            ("page", page_str.as_str()),
            ("per_page", per_page_str.as_str()),
        ];

        let page_result: Page<ApiPullRequestSummary> = match self
            .client
            .get(locator.pulls_path(), Some(&query_params))
            .await
        {
            Ok(page_result) => page_result,
            Err(error) => {
                return Err(self
                    .map_octocrab_error_with_rate_limit("list pulls", &error)
                    .await);
            }
        };

        // Extract pagination info before consuming items.
        let has_next = page_result.next.is_some();
        let has_prev = page_result.prev.is_some();
        let total_pages = page_result.number_of_pages();

        let items: Vec<PullRequestSummary> = page_result
            .items
            .into_iter()
            .map(ApiPullRequestSummary::into)
            .collect();

        let page_info = PageInfo::builder(page, per_page)
            .total_pages(total_pages)
            .has_next(has_next)
            .has_prev(has_prev)
            .build();

        Ok(PaginatedPullRequests {
            items,
            page_info,
            rate_limit: None, // Rate limit headers not directly accessible via octocrab.
        })
    }
}

impl OctocrabRepositoryGateway {
    async fn map_octocrab_error_with_rate_limit(
        &self,
        operation: &str,
        error: &octocrab::Error,
    ) -> IntakeError {
        match error {
            octocrab::Error::GitHub { source, .. } if is_rate_limit_error(source) => {
                let rate_limit = self.fetch_rate_limit_info().await;
                let base_message =
                    format!("{operation} failed: {message}", message = source.message);
                let message = match &rate_limit {
                    Some(info) => format!(
                        "{base_message} (resets at {reset})",
                        reset = info.reset_at()
                    ),
                    None => base_message,
                };

                IntakeError::RateLimitExceeded {
                    rate_limit,
                    message,
                }
            }
            _ => map_octocrab_error(operation, error),
        }
    }

    async fn fetch_rate_limit_info(&self) -> Option<RateLimitInfo> {
        let rate = self.client.ratelimit().get().await.ok()?.rate;
        let Ok(limit) = u32::try_from(rate.limit) else {
            return None;
        };
        let Ok(remaining) = u32::try_from(rate.remaining) else {
            return None;
        };
        Some(RateLimitInfo::new(limit, remaining, rate.reset))
    }
}

fn validate_pagination_params(page: u32, per_page: u8) -> Result<(), IntakeError> {
    if page == 0 {
        return Err(IntakeError::InvalidPagination {
            message: "page must be at least 1".to_owned(),
        });
    }

    if per_page == 0 {
        return Err(IntakeError::InvalidPagination {
            message: "per_page must be at least 1".to_owned(),
        });
    }

    if per_page > 100 {
        return Err(IntakeError::InvalidPagination {
            message: "per_page must not exceed 100".to_owned(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
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
}
