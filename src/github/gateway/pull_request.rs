//! Octocrab implementation of the pull request gateway.

use async_trait::async_trait;
use octocrab::Octocrab;

use crate::github::error::IntakeError;
use crate::github::locator::{PersonalAccessToken, PullRequestLocator};
use crate::github::models::{ApiPullRequest, PullRequestComment, PullRequestMetadata};

use super::PullRequestGateway;
use super::client::build_octocrab_client;
use super::comments::fetch_pull_request_comments;
use super::error_mapping::map_octocrab_error;

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
        fetch_pull_request_comments(&self.client, locator).await
    }
}
