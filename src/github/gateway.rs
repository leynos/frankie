//! Gateways for loading pull requests through Octocrab.

use async_trait::async_trait;
use http::{StatusCode, Uri};
use octocrab::{Octocrab, Page};

use super::error::IntakeError;
use super::locator::{PersonalAccessToken, PullRequestLocator};
use super::models::{ApiComment, ApiPullRequest, PullRequestComment, PullRequestMetadata};

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
        let base_uri: Uri = locator
            .api_base()
            .as_str()
            .parse::<Uri>()
            .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

        let octocrab = Octocrab::builder()
            .personal_token(token.as_ref())
            .base_uri(base_uri)
            .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?
            .build()
            .map_err(|error| map_octocrab_error("build client", &error))?;

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
