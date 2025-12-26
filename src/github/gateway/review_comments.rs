//! Gateway for fetching PR review comments.

use async_trait::async_trait;
use octocrab::{Octocrab, Page};

use crate::github::error::IntakeError;
use crate::github::locator::{PersonalAccessToken, PullRequestLocator};
use crate::github::models::{ApiReviewComment, ReviewComment};

use super::ReviewCommentGateway;
use super::client::build_octocrab_client;
use super::error_mapping::map_octocrab_error;

/// Fetches all review comments for a pull request.
pub(super) async fn fetch_review_comments(
    client: &Octocrab,
    locator: &PullRequestLocator,
) -> Result<Vec<ReviewComment>, IntakeError> {
    let page = client
        .get::<Page<ApiReviewComment>, _, _>(locator.review_comments_path(), None::<&()>)
        .await
        .map_err(|error| map_octocrab_error("review comments", &error))?;

    client
        .all_pages(page)
        .await
        .map(|comments| comments.into_iter().map(Into::into).collect())
        .map_err(|error| map_octocrab_error("review comments", &error))
}

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
}

#[async_trait]
impl ReviewCommentGateway for OctocrabReviewCommentGateway {
    async fn list_review_comments(
        &self,
        locator: &PullRequestLocator,
    ) -> Result<Vec<ReviewComment>, IntakeError> {
        fetch_review_comments(&self.client, locator).await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::github::models::ApiReviewComment;

    #[test]
    fn api_review_comment_deserialises_from_json() {
        let value = json!({
            "id": 456,
            "body": "Consider using a constant here.",
            "user": { "login": "reviewer" },
            "path": "src/main.rs",
            "line": 42,
            "original_line": 40,
            "diff_hunk": "@@ -38,6 +38,8 @@\n+    let x = 1;",
            "commit_id": "abc123",
            "in_reply_to_id": null,
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-02T00:00:00Z"
        });

        let api: ApiReviewComment =
            serde_json::from_value(value).expect("ApiReviewComment should deserialise");
        assert_eq!(api.id, 456);
        assert_eq!(api.body.as_deref(), Some("Consider using a constant here."));
        assert_eq!(
            api.user.as_ref().and_then(|u| u.login.as_deref()),
            Some("reviewer")
        );
        assert_eq!(api.path.as_deref(), Some("src/main.rs"));
        assert_eq!(api.line, Some(42));
        assert_eq!(api.original_line, Some(40));
        assert_eq!(api.commit_id.as_deref(), Some("abc123"));
        assert!(api.in_reply_to_id.is_none());
    }
}
