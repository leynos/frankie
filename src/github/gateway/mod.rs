//! Gateways for loading pull requests through Octocrab.
//!
//! This module provides trait-based gateways for communicating with the GitHub
//! API. The trait-based design enables mocking in tests while the Octocrab
//! implementations handle real HTTP requests.

mod caching;
mod client;
mod error_mapping;
mod http_utils;
mod pull_request;
mod repository;

pub use caching::OctocrabCachingGateway;
pub use pull_request::OctocrabGateway;
pub use repository::{
    ListPullRequestsParams, OctocrabRepositoryGateway, PaginatedPullRequests, PullRequestState,
};

use async_trait::async_trait;

use crate::github::error::IntakeError;
use crate::github::locator::{PullRequestLocator, RepositoryLocator};
use crate::github::models::{PullRequestComment, PullRequestMetadata};

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
