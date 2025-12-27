//! CLI operation mode handlers.
//!
//! This module contains the implementations for different operation modes:
//! - [`interactive`]: Local repository discovery and listing
//! - [`migrations`]: Database schema migrations
//! - [`repository_listing`]: List PRs for a specified repository
//! - [`review_tui`]: Interactive TUI for reviewing PR comments
//! - [`single_pr`]: Load details for a single pull request
//!
//! Output formatting utilities are in [`output`].

use frankie::{ListPullRequestsParams, PullRequestState};

pub mod interactive;
pub mod migrations;
pub mod output;
pub mod repository_listing;
pub mod review_tui;
pub mod single_pr;

#[cfg(test)]
pub mod test_utils;

/// Returns the default parameters for listing pull requests.
pub const fn default_listing_params() -> ListPullRequestsParams {
    ListPullRequestsParams {
        state: Some(PullRequestState::All),
        per_page: Some(50),
        page: Some(1),
    }
}
