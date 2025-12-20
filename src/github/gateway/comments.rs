//! Shared comment fetching helpers for GitHub gateways.

use octocrab::{Octocrab, Page};

use crate::github::error::IntakeError;
use crate::github::locator::PullRequestLocator;
use crate::github::models::{ApiComment, PullRequestComment};

use super::error_mapping::map_octocrab_error;

pub(super) async fn fetch_pull_request_comments(
    client: &Octocrab,
    locator: &PullRequestLocator,
) -> Result<Vec<PullRequestComment>, IntakeError> {
    let page = client
        .get::<Page<ApiComment>, _, _>(locator.comments_path(), None::<&()>)
        .await
        .map_err(|error| map_octocrab_error("issue comments", &error))?;

    client
        .all_pages(page)
        .await
        .map(|comments| comments.into_iter().map(ApiComment::into).collect())
        .map_err(|error| map_octocrab_error("issue comments", &error))
}
