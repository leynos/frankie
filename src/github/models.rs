//! Data models representing pull request metadata and comments.

use serde::Deserialize;

/// Minimal pull request metadata used by the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
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
