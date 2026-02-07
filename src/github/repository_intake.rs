//! High-level repository intake facade for PR listing.
//!
//! This module provides the `RepositoryIntake` facade that aggregates
//! repository-level operations. It wraps the `RepositoryGateway` trait
//! to provide a simplified interface for listing pull requests.

use super::error::IntakeError;
use super::gateway::{ListPullRequestsParams, PaginatedPullRequests, RepositoryGateway};
use super::repository_locator::RepositoryLocator;

/// Aggregates repository-level operations using a gateway.
///
/// # Example
///
/// ```ignore
/// use frankie::{
///     OctocrabRepositoryGateway, PersonalAccessToken, RepositoryIntake, RepositoryLocator,
/// };
///
/// let token = PersonalAccessToken::new("ghp_example")?;
/// let locator = RepositoryLocator::from_owner_repo("owner", "repo")?;
/// let gateway = OctocrabRepositoryGateway::for_token(&token, &locator)?;
/// let intake = RepositoryIntake::new(&gateway);
/// let result = intake.list_pull_requests(&locator, &Default::default()).await?;
/// ```
pub struct RepositoryIntake<'client, Gateway>
where
    Gateway: RepositoryGateway,
{
    client: &'client Gateway,
}

impl<'client, Gateway> RepositoryIntake<'client, Gateway>
where
    Gateway: RepositoryGateway,
{
    /// Create a new repository intake facade.
    #[must_use]
    pub const fn new(client: &'client Gateway) -> Self {
        Self { client }
    }

    /// List pull requests with pagination.
    ///
    /// # Errors
    ///
    /// Returns an error if the GitHub API request fails due to authentication,
    /// network issues, rate limiting, or other API errors.
    pub async fn list_pull_requests(
        &self,
        locator: &RepositoryLocator,
        params: &ListPullRequestsParams,
    ) -> Result<PaginatedPullRequests, IntakeError> {
        self.client.list_pull_requests(locator, params).await
    }
}
