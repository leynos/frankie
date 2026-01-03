//! Data models representing pull request metadata and comments.
//!
//! This module contains domain models for pull request data returned by the
//! GitHub API. Types prefixed with `Api` are internal deserialisation targets
//! that convert into public domain types.

use serde::Deserialize;

#[cfg(feature = "test-support")]
pub mod test_support;

/// Minimal pull request metadata used by the CLI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullRequestMetadata {
    /// Pull request number.
    pub number: u64,
    /// Title of the pull request.
    pub title: Option<String>,
    /// State (e.g. open, closed).
    pub state: Option<String>,
    /// HTML URL for displaying to a user.
    pub html_url: Option<String>,
    /// Author login if present.
    pub author: Option<String>,
}

/// Pull request issue comment details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestComment {
    /// Comment identifier.
    pub id: u64,
    /// Comment body.
    pub body: Option<String>,
    /// Author login.
    pub author: Option<String>,
}

/// Combined pull request details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestDetails {
    /// PR metadata.
    pub metadata: PullRequestMetadata,
    /// All issue comments attached to the PR.
    pub comments: Vec<PullRequestComment>,
}

/// Pull request review comment (distinct from issue comments).
///
/// Review comments are attached to specific lines in a pull request diff,
/// whereas issue comments are general discussion on the PR.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewComment {
    /// Comment identifier.
    pub id: u64,
    /// Comment body.
    pub body: Option<String>,
    /// Author login.
    pub author: Option<String>,
    /// File path the comment is attached to.
    pub file_path: Option<String>,
    /// Line number in the diff the comment refers to.
    pub line_number: Option<u32>,
    /// Original line number before any changes.
    pub original_line_number: Option<u32>,
    /// Diff hunk context for this comment.
    pub diff_hunk: Option<String>,
    /// Commit SHA this comment was made against.
    pub commit_sha: Option<String>,
    /// ID of the comment this is replying to, if any.
    pub in_reply_to_id: Option<u64>,
    /// Creation timestamp (ISO 8601 format).
    pub created_at: Option<String>,
    /// Last update timestamp (ISO 8601 format).
    pub updated_at: Option<String>,
}

/// Lightweight pull request summary for listing views.
///
/// Contains only the fields needed for PR listing, reducing payload size
/// compared to full `PullRequestMetadata`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestSummary {
    /// Pull request number.
    pub number: u64,
    /// Title of the pull request.
    pub title: Option<String>,
    /// State (e.g. open, closed).
    pub state: Option<String>,
    /// Author login if present.
    pub author: Option<String>,
    /// Creation timestamp (ISO 8601 format).
    pub created_at: Option<String>,
    /// Last update timestamp (ISO 8601 format).
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ApiPullRequest {
    pub(super) number: u64,
    pub(super) title: Option<String>,
    pub(super) state: Option<String>,
    pub(super) html_url: Option<String>,
    pub(super) user: Option<ApiUser>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ApiComment {
    pub(super) id: u64,
    pub(super) body: Option<String>,
    pub(super) user: Option<ApiUser>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ApiUser {
    pub(super) login: Option<String>,
}

/// API response type for PR listing.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct ApiPullRequestSummary {
    pub(super) number: u64,
    pub(super) title: Option<String>,
    pub(super) state: Option<String>,
    pub(super) user: Option<ApiUser>,
    pub(super) created_at: Option<String>,
    pub(super) updated_at: Option<String>,
}

/// API response type for PR review comments.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct ApiReviewComment {
    pub(super) id: u64,
    pub(super) body: Option<String>,
    pub(super) user: Option<ApiUser>,
    pub(super) path: Option<String>,
    pub(super) line: Option<u32>,
    pub(super) original_line: Option<u32>,
    pub(super) diff_hunk: Option<String>,
    pub(super) commit_id: Option<String>,
    pub(super) in_reply_to_id: Option<u64>,
    pub(super) created_at: Option<String>,
    pub(super) updated_at: Option<String>,
}

impl From<ApiPullRequest> for PullRequestMetadata {
    fn from(value: ApiPullRequest) -> Self {
        Self {
            number: value.number,
            title: value.title,
            state: value.state,
            html_url: value.html_url,
            author: value.user.and_then(|user| user.login),
        }
    }
}

impl From<ApiComment> for PullRequestComment {
    fn from(value: ApiComment) -> Self {
        Self {
            id: value.id,
            body: value.body,
            author: value.user.and_then(|user| user.login),
        }
    }
}

impl From<ApiPullRequestSummary> for PullRequestSummary {
    fn from(value: ApiPullRequestSummary) -> Self {
        Self {
            number: value.number,
            title: value.title,
            state: value.state,
            author: value.user.and_then(|user| user.login),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<ApiReviewComment> for ReviewComment {
    fn from(value: ApiReviewComment) -> Self {
        Self {
            id: value.id,
            body: value.body,
            author: value.user.and_then(|user| user.login),
            file_path: value.path,
            line_number: value.line,
            original_line_number: value.original_line,
            diff_hunk: value.diff_hunk,
            commit_sha: value.commit_id,
            in_reply_to_id: value.in_reply_to_id,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use serde_json::json;

    use super::{ApiPullRequestSummary, ApiReviewComment, ApiUser, PullRequestSummary};

    #[test]
    fn api_pull_request_summary_deserializes_from_json() {
        let value = json!({
            "number": 123,
            "title": "Add tests",
            "state": "open",
            "user": { "login": "octocat" },
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-02T00:00:00Z"
        });

        let api: ApiPullRequestSummary =
            serde_json::from_value(value).expect("ApiPullRequestSummary should deserialize");
        assert_eq!(api.number, 123);
        assert_eq!(api.title.as_deref(), Some("Add tests"));
        assert_eq!(api.state.as_deref(), Some("open"));
        assert_eq!(
            api.user.as_ref().and_then(|user| user.login.as_deref()),
            Some("octocat")
        );
        assert_eq!(api.created_at.as_deref(), Some("2025-01-01T00:00:00Z"));
        assert_eq!(api.updated_at.as_deref(), Some("2025-01-02T00:00:00Z"));
    }

    #[test]
    fn api_pull_request_summary_converts_into_pull_request_summary() {
        let api = ApiPullRequestSummary {
            number: 42,
            title: Some("Ship it".to_owned()),
            state: Some("closed".to_owned()),
            user: Some(ApiUser {
                login: Some("alice".to_owned()),
            }),
            created_at: None,
            updated_at: Some("2025-01-03T00:00:00Z".to_owned()),
        };

        let summary: PullRequestSummary = api.into();
        assert_eq!(summary.number, 42);
        assert_eq!(summary.title.as_deref(), Some("Ship it"));
        assert_eq!(summary.state.as_deref(), Some("closed"));
        assert_eq!(summary.author.as_deref(), Some("alice"));
        assert_eq!(summary.created_at, None);
        assert_eq!(summary.updated_at.as_deref(), Some("2025-01-03T00:00:00Z"));
    }

    #[fixture]
    fn sample_api_review_comment() -> ApiReviewComment {
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
        serde_json::from_value(value).expect("ApiReviewComment should deserialise")
    }

    #[rstest]
    fn api_review_comment_deserialises_core_fields(sample_api_review_comment: ApiReviewComment) {
        assert_eq!(sample_api_review_comment.id, 456);
        assert_eq!(
            sample_api_review_comment.body.as_deref(),
            Some("Consider using a constant here.")
        );
        assert_eq!(
            sample_api_review_comment
                .user
                .as_ref()
                .and_then(|u| u.login.as_deref()),
            Some("reviewer")
        );
        assert_eq!(
            sample_api_review_comment.path.as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(sample_api_review_comment.line, Some(42));
        assert_eq!(sample_api_review_comment.original_line, Some(40));
    }

    #[rstest]
    fn api_review_comment_deserialises_metadata_fields(
        sample_api_review_comment: ApiReviewComment,
    ) {
        assert_eq!(
            sample_api_review_comment.commit_id.as_deref(),
            Some("abc123")
        );
        assert!(sample_api_review_comment.in_reply_to_id.is_none());
        assert_eq!(
            sample_api_review_comment.diff_hunk.as_deref(),
            Some("@@ -38,6 +38,8 @@\n+    let x = 1;")
        );
        assert_eq!(
            sample_api_review_comment.created_at.as_deref(),
            Some("2025-01-01T00:00:00Z")
        );
        assert_eq!(
            sample_api_review_comment.updated_at.as_deref(),
            Some("2025-01-02T00:00:00Z")
        );
    }

    #[rstest]
    #[case::all_optional_fields_null(json!({
        "id": 789,
        "body": null,
        "user": null,
        "path": null,
        "line": null,
        "original_line": null,
        "diff_hunk": null,
        "commit_id": null,
        "in_reply_to_id": null,
        "created_at": null,
        "updated_at": null
    }))]
    #[case::optional_fields_absent(json!({
        "id": 789
    }))]
    fn api_review_comment_deserialises_with_missing_optional_fields(
        #[case] value: serde_json::Value,
    ) {
        let comment: ApiReviewComment =
            serde_json::from_value(value).expect("should deserialise with missing fields");

        assert_eq!(comment.id, 789);
        assert!(comment.body.is_none());
        assert!(comment.user.is_none());
        assert!(comment.path.is_none());
        assert!(comment.line.is_none());
        assert!(comment.original_line.is_none());
        assert!(comment.diff_hunk.is_none());
        assert!(comment.commit_id.is_none());
        assert!(comment.in_reply_to_id.is_none());
        assert!(comment.created_at.is_none());
        assert!(comment.updated_at.is_none());
    }
}
