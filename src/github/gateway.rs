//! Gateways for loading pull requests through Octocrab.
//!
//! This module provides trait-based gateways for communicating with the GitHub
//! API. The trait-based design enables mocking in tests while the Octocrab
//! implementations handle real HTTP requests.

use async_trait::async_trait;
use http::{StatusCode, Uri};
use octocrab::{Octocrab, Page};

use super::error::IntakeError;
use super::locator::{PersonalAccessToken, PullRequestLocator, RepositoryLocator};
use super::models::{
    ApiComment, ApiPullRequest, ApiPullRequestSummary, PullRequestComment, PullRequestMetadata,
    PullRequestSummary,
};
use super::pagination::PageInfo;
use super::rate_limit::RateLimitInfo;

/// Builds an Octocrab client for the given token and API base URL.
///
/// This helper consolidates the shared logic for parsing the base URI and
/// constructing an authenticated Octocrab client.
///
/// # Errors
///
/// Returns `IntakeError::InvalidUrl` when the base URI cannot be parsed or
/// `IntakeError::Api` when Octocrab fails to construct a client.
fn build_octocrab_client(
    token: &PersonalAccessToken,
    api_base: &str,
) -> Result<Octocrab, IntakeError> {
    let base_uri: Uri = api_base
        .parse::<Uri>()
        .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

    Octocrab::builder()
        .personal_token(token.as_ref())
        .base_uri(base_uri)
        .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?
        .build()
        .map_err(|error| map_octocrab_error("build client", &error))
}

/// Gateway that can load pull request data.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait PullRequestGateway: Send + Sync {
    /// Fetch the pull request metadata.
    async fn pull_request(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<PullRequestMetadata, IntakeError>;

    /// Fetch all issue comments for the pull request.
    async fn pull_request_comments(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<Vec<PullRequestComment>, IntakeError>;
}

/// Octocrab-backed gateway.
pub struct OctocrabGateway {
    client: Octocrab,
}

impl OctocrabGateway {
    /// Creates a new gateway from an Octocrab client.
    #[must_use]
    pub const fn new(client: Octocrab) -> Self {
        Self { client }
    }

    /// Builds an Octocrab client for the given token and pull request locator.
    ///
    /// # Errors
    ///
    /// Returns `IntakeError::InvalidUrl` when the base URI cannot be parsed or
    /// `IntakeError::Api` when Octocrab fails to construct a client.
    pub fn for_token(
        token: &PersonalAccessToken,
        locator: &PullRequestLocator,
    ) -> Result<Self, IntakeError> {
        let octocrab = build_octocrab_client(token, locator.api_base().as_str())?;
        Ok(Self::new(octocrab))
    }
}

#[async_trait]
impl PullRequestGateway for OctocrabGateway {
    async fn pull_request(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<PullRequestMetadata, IntakeError> {
        self.client
            .get::<ApiPullRequest, _, _>(locator.pull_request_path(), None::<&()>)
            .await
            .map(ApiPullRequest::into)
            .map_err(|error| map_octocrab_error("pull request", &error))
    }

    async fn pull_request_comments(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<Vec<PullRequestComment>, IntakeError> {
        let page = self
            .client
            .get::<Page<ApiComment>, _, _>(locator.comments_path(), None::<&()>)
            .await
            .map_err(|error| map_octocrab_error("issue comments", &error))?;

        self.client
            .all_pages(page)
            .await
            .map(|comments| comments.into_iter().map(ApiComment::into).collect())
            .map_err(|error| map_octocrab_error("issue comments", &error))
    }
}

// --- Repository Gateway for listing PRs ---

/// Pull request state filter for listing operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PullRequestState {
    /// Only open pull requests.
    #[default]
    Open,
    /// Only closed pull requests.
    Closed,
    /// All pull requests regardless of state.
    All,
}

impl PullRequestState {
    /// Returns the API parameter value for this state.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::All => "all",
        }
    }
}

/// Parameters for listing pull requests.
#[derive(Debug, Clone, Default)]
pub struct ListPullRequestsParams {
    /// Filter by state (open, closed, all). Defaults to open.
    pub state: Option<PullRequestState>,
    /// Page number to fetch (1-based). Defaults to 1.
    pub page: Option<u32>,
    /// Items per page (max 100). Defaults to 30.
    pub per_page: Option<u8>,
}

/// Paginated pull request listing result.
#[derive(Debug, Clone)]
pub struct PaginatedPullRequests {
    /// Pull request summaries on this page.
    pub items: Vec<PullRequestSummary>,
    /// Pagination state.
    pub page_info: PageInfo,
    /// Rate limit info if available from response headers.
    pub rate_limit: Option<RateLimitInfo>,
}

/// Gateway for repository-level operations.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait RepositoryGateway: Send + Sync {
    /// List pull requests for the repository with pagination.
    async fn list_pull_requests(
        &self,
        locator: &RepositoryLocator,
        params: &ListPullRequestsParams,
    ) -> Result<PaginatedPullRequests, IntakeError>;
}

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

        let query_params = [
            ("state", state.as_str()),
            ("page", &page.to_string()),
            ("per_page", &per_page.to_string()),
        ];

        let page_result: Page<ApiPullRequestSummary> = self
            .client
            .get(locator.pulls_path(), Some(&query_params))
            .await
            .map_err(|error| map_octocrab_error_with_rate_limit("list pulls", &error))?;

        // Extract pagination info before consuming items
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
            rate_limit: None, // Rate limit headers not directly accessible via octocrab
        })
    }
}

// --- Error mapping helpers ---

/// Checks if a GitHub error status indicates an authentication failure.
const fn is_auth_failure(status: StatusCode) -> bool {
    matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
}

/// Checks if a GitHub error indicates rate limiting.
fn is_rate_limit_error(source: &octocrab::GitHubError) -> bool {
    source.status_code == StatusCode::FORBIDDEN
        && source.message.to_lowercase().contains("rate limit")
}

/// Checks if an octocrab error represents a network/transport issue.
const fn is_network_error(error: &octocrab::Error) -> bool {
    matches!(
        error,
        octocrab::Error::Http { .. }
            | octocrab::Error::Hyper { .. }
            | octocrab::Error::Service { .. }
    )
}

pub(super) fn map_octocrab_error(operation: &str, error: &octocrab::Error) -> IntakeError {
    if let octocrab::Error::GitHub { source, .. } = error {
        return if is_auth_failure(source.status_code) {
            IntakeError::Authentication {
                message: format!(
                    "{operation} failed: GitHub returned {status} {message}",
                    status = source.status_code,
                    message = source.message
                ),
            }
        } else {
            IntakeError::Api {
                message: format!(
                    "{operation} failed with status {status}: {message}",
                    status = source.status_code,
                    message = source.message
                ),
            }
        };
    }

    if is_network_error(error) {
        return IntakeError::Network {
            message: format!("{operation} failed: {error}"),
        };
    }

    IntakeError::Api {
        message: format!("{operation} failed: {error}"),
    }
}

/// Maps octocrab errors with special handling for rate limit errors.
fn map_octocrab_error_with_rate_limit(operation: &str, error: &octocrab::Error) -> IntakeError {
    if let octocrab::Error::GitHub { source, .. } = error
        && is_rate_limit_error(source)
    {
        return IntakeError::RateLimitExceeded {
            rate_limit: None,
            message: format!("{operation} failed: {message}", message = source.message),
        };
    }

    map_octocrab_error(operation, error)
}
