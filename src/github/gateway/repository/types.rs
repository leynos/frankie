//! Public types for repository gateway operations.

use crate::github::models::PullRequestSummary;
use crate::github::pagination::PageInfo;
use crate::github::rate_limit::RateLimitInfo;

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
