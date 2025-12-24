//! Error types exposed by the GitHub intake layer.

use thiserror::Error;

use super::rate_limit::RateLimitInfo;

/// Errors surfaced while parsing input or communicating with GitHub.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
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

    /// Local I/O operation failed.
    #[error("I/O error: {message}")]
    Io {
        /// Error detail from the underlying I/O operation.
        message: String,
    },

    /// Configuration could not be loaded.
    #[error("configuration error: {message}")]
    Configuration {
        /// Details about the configuration failure.
        message: String,
    },

    /// Rate limit exceeded - the API returned 403 with rate limit message.
    #[error("GitHub API rate limit exceeded: {message}")]
    RateLimitExceeded {
        /// Rate limit info if available from response headers.
        rate_limit: Option<RateLimitInfo>,
        /// Error message from GitHub.
        message: String,
    },

    /// Invalid pagination parameters.
    #[error("invalid pagination: {message}")]
    InvalidPagination {
        /// Description of the invalid parameter.
        message: String,
    },

    /// Local repository discovery failed.
    #[error("local discovery: {message}")]
    LocalDiscovery {
        /// Details about the discovery failure.
        message: String,
    },
}
