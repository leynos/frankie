//! Gateways for loading pull requests through Octocrab.
//!
//! This module provides trait-based gateways for communicating with the GitHub
//! API. The trait-based design enables mocking in tests while the Octocrab
//! implementations handle real HTTP requests.

mod caching;
mod client;
mod comments;
mod error_mapping;
mod http_utils;
mod pull_request;
mod repository;
mod review_comments;

pub use caching::OctocrabCachingGateway;
pub use pull_request::OctocrabGateway;
pub use repository::{
    ListPullRequestsParams, OctocrabRepositoryGateway, PaginatedPullRequests, PullRequestState,
};
pub use review_comments::OctocrabReviewCommentGateway;

use async_trait::async_trait;

use crate::github::error::IntakeError;
use crate::github::locator::PullRequestLocator;
use crate::github::models::{PullRequestComment, PullRequestMetadata, ReviewComment};
use crate::github::repository_locator::RepositoryLocator;

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

/// Gateway for fetching PR review comments.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ReviewCommentGateway: Send + Sync {
    /// Fetch all review comments for the pull request.
    async fn list_review_comments(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<Vec<ReviewComment>, IntakeError>;
}
