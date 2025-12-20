//! Gateways for loading pull requests through Octocrab.
//!
//! This module provides trait-based gateways for communicating with the GitHub
//! API. The trait-based design enables mocking in tests while the Octocrab
//! implementations handle real HTTP requests.

use async_trait::async_trait;
use http::header::{ETAG, HeaderMap, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
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
use crate::persistence::{
    CachedPullRequestMetadata, PersistenceError, PullRequestMetadataCache,
    PullRequestMetadataCacheWrite,
};

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
        .map_err(|error| IntakeError::Api {
            message: format!("build client failed: {error}"),
        })?
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

// --- Pull request metadata caching gateway ---

#[derive(Debug, Clone)]
struct ResponseValidators {
    etag: Option<String>,
    last_modified: Option<String>,
}

/// Octocrab-backed gateway that caches pull request metadata in `SQLite`.
///
/// Only the metadata call is cached; comment listing currently always calls the
/// GitHub API.
pub struct OctocrabCachingGateway {
    client: Octocrab,
    cache: PullRequestMetadataCache,
    ttl_seconds: u64,
}

impl OctocrabCachingGateway {
    /// Builds a caching gateway for the given token, pull request locator, and
    /// database URL.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError`] when the Octocrab client cannot be constructed
    /// or when the database URL is invalid.
    pub fn for_token(
        token: &PersonalAccessToken,
        locator: &PullRequestLocator,
        database_url: &str,
        ttl_seconds: u64,
    ) -> Result<Self, IntakeError> {
        let octocrab = build_octocrab_client(token, locator.api_base().as_str())?;
        let cache = PullRequestMetadataCache::new(database_url.to_owned())
            .map_err(|error| map_persistence_error("initialise cache", &error))?;
        Ok(Self {
            client: octocrab,
            cache,
            ttl_seconds,
        })
    }

    fn expiry_window(&self, now_unix: i64) -> (i64, i64) {
        let ttl_unix = i64::try_from(self.ttl_seconds).unwrap_or(i64::MAX);
        let expires_at = now_unix.saturating_add(ttl_unix);
        (now_unix, expires_at)
    }

    async fn fetch_pull_request(
        &self,
        locator: &PullRequestLocator,
        conditional: Option<&CachedPullRequestMetadata>,
    ) -> Result<FetchResult, IntakeError> {
        let headers = conditional.and_then(build_conditional_headers);
        let uri: Uri = locator
            .pull_request_path()
            .parse::<Uri>()
            .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

        let response = self
            .client
            ._get_with_headers(uri, headers)
            .await
            .map_err(|error| map_octocrab_error("pull request", &error))?;

        match response.status() {
            StatusCode::NOT_MODIFIED => Ok(FetchResult::NotModified),
            StatusCode::OK => {
                let validators = ResponseValidators {
                    etag: header_to_string(response.headers().get(ETAG)),
                    last_modified: header_to_string(response.headers().get(LAST_MODIFIED)),
                };

                let body = self
                    .client
                    .body_to_string(response)
                    .await
                    .map_err(|error| IntakeError::Api {
                        message: format!("pull request response decode failed: {error}"),
                    })?;

                let api: ApiPullRequest =
                    serde_json::from_str(&body).map_err(|error| IntakeError::Api {
                        message: format!("pull request response deserialisation failed: {error}"),
                    })?;

                Ok(FetchResult::Modified {
                    metadata: api.into(),
                    validators,
                })
            }
            status => {
                let body = self
                    .client
                    .body_to_string(response)
                    .await
                    .unwrap_or_else(|_| String::new());

                Err(map_http_error(
                    "pull request",
                    status,
                    extract_github_message(&body),
                ))
            }
        }
    }
}

enum FetchResult {
    NotModified,
    Modified {
        metadata: PullRequestMetadata,
        validators: ResponseValidators,
    },
}

#[async_trait]
impl PullRequestGateway for OctocrabCachingGateway {
    async fn pull_request(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<PullRequestMetadata, IntakeError> {
        let now = PullRequestMetadataCache::now_unix_seconds();
        let cached = self
            .cache
            .get(locator)
            .map_err(|error| map_persistence_error("read cache", &error))?;

        if let Some(entry) = cached {
            if !entry.is_expired(now) {
                return Ok(entry.metadata);
            }

            match self.fetch_pull_request(locator, Some(&entry)).await? {
                FetchResult::NotModified => {
                    let (fetched_at, expires_at) = self.expiry_window(now);
                    self.cache
                        .touch(locator, fetched_at, expires_at)
                        .map_err(|error| map_persistence_error("update cache", &error))?;
                    Ok(entry.metadata)
                }
                FetchResult::Modified {
                    metadata,
                    validators,
                } => {
                    let (fetched_at, expires_at) = self.expiry_window(now);
                    self.cache
                        .upsert(
                            locator,
                            PullRequestMetadataCacheWrite {
                                metadata: &metadata,
                                etag: validators.etag.as_deref(),
                                last_modified: validators.last_modified.as_deref(),
                                fetched_at_unix: fetched_at,
                                expires_at_unix: expires_at,
                            },
                        )
                        .map_err(|error| map_persistence_error("write cache", &error))?;
                    Ok(metadata)
                }
            }
        } else {
            match self.fetch_pull_request(locator, None).await? {
                FetchResult::NotModified => Err(IntakeError::Api {
                    message: "unexpected 304 for uncached pull request".to_owned(),
                }),
                FetchResult::Modified {
                    metadata,
                    validators,
                } => {
                    let (fetched_at, expires_at) = self.expiry_window(now);
                    self.cache
                        .upsert(
                            locator,
                            PullRequestMetadataCacheWrite {
                                metadata: &metadata,
                                etag: validators.etag.as_deref(),
                                last_modified: validators.last_modified.as_deref(),
                                fetched_at_unix: fetched_at,
                                expires_at_unix: expires_at,
                            },
                        )
                        .map_err(|error| map_persistence_error("write cache", &error))?;
                    Ok(metadata)
                }
            }
        }
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
#[derive(Debug, Clone)]
pub struct ListPullRequestsParams {
    /// Filter by state (open, closed, all). Defaults to open.
    pub state: Option<PullRequestState>,
    /// Page number to fetch (1-based). Defaults to 1.
    pub page: Option<u32>,
    /// Items per page (max 100). Defaults to 30.
    pub per_page: Option<u8>,
}

impl Default for ListPullRequestsParams {
    fn default() -> Self {
        Self {
            state: Some(PullRequestState::Open),
            page: Some(1),
            per_page: Some(30),
        }
    }
}

/// Paginated pull request listing result.
#[derive(Debug, Clone)]
pub struct PaginatedPullRequests {
    /// Pull request summaries on this page.
    pub items: Vec<PullRequestSummary>,
    /// Pagination state.
    pub page_info: PageInfo,
    /// Rate limit information when available.
    ///
    /// This is currently always `None` for successful responses because
    /// Octocrab does not expose rate limit headers on normal requests. Rate
    /// limit errors are instead mapped to `IntakeError::RateLimitExceeded`
    /// (with optional rate limit data when it can be fetched).
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

// --- Error mapping helpers ---

/// Checks if a GitHub error status indicates an authentication failure.
const fn is_auth_failure(status: StatusCode) -> bool {
    matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
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

/// Checks whether the GitHub error represents a rate limit error based on the
/// HTTP status and message / documentation URL content.
fn is_rate_limit_error(source: &octocrab::GitHubError) -> bool {
    let is_rate_limit_status = matches!(
        source.status_code,
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
    );

    let message_indicates_rate_limit = source.message.to_lowercase().contains("rate limit")
        || source
            .documentation_url
            .as_deref()
            .is_some_and(|url| url.contains("rate-limit"));

    is_rate_limit_status && message_indicates_rate_limit
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

fn build_conditional_headers(cached: &CachedPullRequestMetadata) -> Option<HeaderMap> {
    let mut headers = HeaderMap::new();

    if let Some(etag) = cached.etag.as_deref()
        && let Ok(value) = etag.parse()
    {
        headers.insert(IF_NONE_MATCH, value);
    }

    if let Some(last_modified) = cached.last_modified.as_deref()
        && let Ok(value) = last_modified.parse()
    {
        headers.insert(IF_MODIFIED_SINCE, value);
    }

    if headers.is_empty() {
        None
    } else {
        Some(headers)
    }
}

fn header_to_string(header_value: Option<&http::header::HeaderValue>) -> Option<String> {
    header_value
        .and_then(|raw| raw.to_str().ok())
        .map(ToOwned::to_owned)
}

fn extract_github_message(body: &str) -> Option<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return None;
    };
    value
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

fn map_http_error(
    operation: &str,
    status: StatusCode,
    maybe_message: Option<String>,
) -> IntakeError {
    let message = maybe_message.unwrap_or_else(|| "unknown error".to_owned());
    if is_auth_failure(status) {
        IntakeError::Authentication {
            message: format!("{operation} failed: GitHub returned {status} {message}"),
        }
    } else {
        IntakeError::Api {
            message: format!("{operation} failed with status {status}: {message}"),
        }
    }
}

fn map_persistence_error(operation: &str, error: &PersistenceError) -> IntakeError {
    match error {
        PersistenceError::MissingDatabaseUrl
        | PersistenceError::BlankDatabaseUrl
        | PersistenceError::SchemaNotInitialised => IntakeError::Configuration {
            message: format!("{operation}: {error}"),
        },
        _ => IntakeError::Io {
            message: format!("{operation}: {error}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{
        IntakeError, ListPullRequestsParams, OctocrabRepositoryGateway, PullRequestState,
        RepositoryGateway,
    };
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
