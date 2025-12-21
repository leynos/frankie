//! Error types for local repository discovery.

use thiserror::Error;

/// Errors that may occur during local repository discovery.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum LocalDiscoveryError {
    /// Current directory is not within a Git repository.
    #[error("not inside a Git repository")]
    NotARepository,

    /// The repository has no remotes configured.
    #[error("repository has no remotes configured")]
    NoRemotes,

    /// The specified remote does not exist.
    #[error("remote '{name}' not found")]
    RemoteNotFound {
        /// Name of the missing remote.
        name: String,
    },

    /// The remote URL could not be parsed.
    #[error("could not parse remote URL: {url}")]
    InvalidRemoteUrl {
        /// The unparseable URL string.
        url: String,
    },

    /// The remote URL is not a GitHub origin.
    #[error("remote '{name}' is not a GitHub origin: {url}")]
    NotGitHubOrigin {
        /// Name of the remote.
        name: String,
        /// The non-GitHub URL.
        url: String,
    },

    /// Git operation failed.
    #[error("git error: {message}")]
    Git {
        /// Error detail from the git2 library.
        message: String,
    },
}

impl From<git2::Error> for LocalDiscoveryError {
    fn from(error: git2::Error) -> Self {
        Self::Git {
            message: error.message().to_owned(),
        }
    }
}
