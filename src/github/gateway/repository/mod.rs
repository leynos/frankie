//! Repository-level gateway for listing pull requests.
//!
//! This module contains the Octocrab-backed repository gateway and its tests.

use async_trait::async_trait;
use octocrab::{Octocrab, Page};

use crate::github::error::IntakeError;
use crate::github::locator::PersonalAccessToken;
use crate::github::models::{ApiPullRequestSummary, PullRequestSummary};
use crate::github::pagination::PageInfo;
use crate::github::rate_limit::RateLimitInfo;
use crate::github::repository_locator::RepositoryLocator;

use super::RepositoryGateway;
use super::client::build_octocrab_client;
use super::error_mapping::{is_rate_limit_error, map_octocrab_error};

mod types;

pub use types::{ListPullRequestsParams, PaginatedPullRequests, PullRequestState};

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

        // Extract pagination info before consuming items.
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
            rate_limit: None, // Rate limit headers not directly accessible via octocrab.
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

#[cfg(test)]
mod tests;
