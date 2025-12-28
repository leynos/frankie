//! Gateway for fetching PR review comments.

use async_trait::async_trait;
use octocrab::{Octocrab, Page};

use crate::github::error::IntakeError;
use crate::github::locator::{PersonalAccessToken, PullRequestLocator};
use crate::github::models::{ApiReviewComment, ReviewComment};
use crate::github::rate_limit::RateLimitInfo;

use super::ReviewCommentGateway;
use super::client::build_octocrab_client;
use super::error_mapping::{is_rate_limit_error, map_octocrab_error};

/// Gateway for loading PR review comments through Octocrab.
pub struct OctocrabReviewCommentGateway {
    client: Octocrab,
}

impl OctocrabReviewCommentGateway {
    /// Creates a new gateway for the given token and API base URL.
    ///
    /// # Arguments
    ///
    /// * `token` - Personal access token for authentication.
    /// * `api_base` - Base URL for the GitHub API (e.g. `https://api.github.com`).
    ///
    /// # Errors
    ///
    /// Returns an error if the Octocrab client cannot be built.
    pub fn new(token: &PersonalAccessToken, api_base: &str) -> Result<Self, IntakeError> {
        let client = build_octocrab_client(token, api_base)?;
        Ok(Self { client })
    }

    /// Fetches all review comments for a pull request.
    ///
    /// This method automatically handles pagination, fetching all pages of
    /// comments from the GitHub API and combining them into a single vector.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError`] when any of the following conditions occur:
    ///
    /// - **Network/HTTP failures**: Connection errors, timeouts, or other transport
    ///   issues when communicating with the GitHub API.
    /// - **Authentication/authorization errors**: Invalid or expired personal access
    ///   token, or insufficient permissions to access the repository.
    /// - **Rate limiting**: GitHub API rate limit exceeded. Returns
    ///   [`IntakeError::RateLimitExceeded`] with optional rate limit information
    ///   including reset time.
    /// - **Pagination failures**: Errors encountered while fetching subsequent pages
    ///   of results via [`Octocrab::all_pages`].
    /// - **Deserialization errors**: Malformed JSON responses or unexpected response
    ///   structure from the GitHub API.
    async fn fetch_review_comments(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<Vec<ReviewComment>, IntakeError> {
        let page: Page<ApiReviewComment> = match self
            .client
            .get(locator.review_comments_path(), None::<&()>)
            .await
        {
            Ok(page) => page,
            Err(error) => {
                return Err(self
                    .map_octocrab_error_with_rate_limit("review comments", &error)
                    .await);
            }
        };

        match self.client.all_pages(page).await {
            Ok(comments) => Ok(comments.into_iter().map(Into::into).collect()),
            Err(error) => Err(self
                .map_octocrab_error_with_rate_limit("review comments", &error)
                .await),
        }
    }

    /// Maps an Octocrab error to an [`IntakeError`], with special handling for
    /// rate limit errors.
    ///
    /// Rate limit errors (HTTP 403/429 with "rate limit" message) are returned as
    /// [`IntakeError::RateLimitExceeded`] with rate limit information fetched from
    /// the GitHub API when available.
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

    /// Fetches rate limit information from the GitHub API.
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

#[async_trait]
impl ReviewCommentGateway for OctocrabReviewCommentGateway {
    async fn list_review_comments(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<Vec<ReviewComment>, IntakeError> {
        self.fetch_review_comments(locator).await
    }
}

#[cfg(test)]
mod tests;
