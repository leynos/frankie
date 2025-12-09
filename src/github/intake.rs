//! High-level intake facade used by the CLI.

use super::error::IntakeError;
use super::gateway::PullRequestGateway;
use super::locator::PullRequestLocator;
use super::models::{PullRequestComment, PullRequestDetails, PullRequestMetadata};

/// Aggregates pull request metadata and comments using a gateway.
pub struct PullRequestIntake<'client, Gateway>
where
    Gateway: PullRequestGateway,
{
    client: &'client Gateway,
}

impl<'client, Gateway> PullRequestIntake<'client, Gateway>
where
    Gateway: PullRequestGateway,
{
    /// Create a new intake facade using the provided gateway.
    #[must_use]
    pub const fn new(client: &'client Gateway) -> Self {
        Self { client }
    }

    /// Load metadata and comments for the target pull request.
    ///
    /// # Errors
    ///
    /// Propagates any failure from the underlying gateway, including GitHub
    /// authentication errors or network problems.
    pub async fn load(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<PullRequestDetails, IntakeError> {
        let metadata: PullRequestMetadata = self.client.pull_request(locator).await?;
        let comments: Vec<PullRequestComment> = self.client.pull_request_comments(locator).await?;
        Ok(PullRequestDetails { metadata, comments })
    }
}
