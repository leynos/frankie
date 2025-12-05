//! Error types exposed by the GitHub intake layer.

use thiserror::Error;

/// Errors surfaced while parsing input or communicating with GitHub.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IntakeError {
    /// The CLI did not include a pull request URL.
    #[error("pull request URL is required")]
    MissingPullRequestUrl,

    /// An unsupported CLI argument was supplied.
    #[error("unrecognised argument: {argument}")]
    InvalidArgument {
        /// The flag or value that the CLI does not accept.
        argument: String,
    },

    /// The provided URL could not be parsed.
    #[error("pull request URL is invalid: {0}")]
    InvalidUrl(String),

    /// The pull request path is incomplete.
    #[error("pull request URL must match /owner/repo/pull/<number>")]
    MissingPathSegments,

    /// The pull request number is not a valid integer.
    #[error("pull request number must be a positive integer")]
    InvalidPullRequestNumber,

    /// The authentication token was missing.
    #[error("personal access token is required")]
    MissingToken,

    /// The authentication token was rejected by GitHub.
    #[error("GitHub rejected the token: {message}")]
    Authentication {
        /// GitHub error message returned with the 401/403 response.
        message: String,
    },

    /// GitHub returned a non-authentication API error.
    #[error("GitHub API error: {message}")]
    Api {
        /// Response body from GitHub describing the failure.
        message: String,
    },

    /// Networking failed while calling GitHub.
    #[error("network error talking to GitHub: {message}")]
    Network {
        /// Transport-level error detail.
        message: String,
    },
}
