//! Error types for local repository operations.

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

    /// Git operation failed.
    #[error("git error: {message}")]
    Git {
        /// Error detail from the git2 library.
        message: String,
    },
}

/// Errors that may occur during Git operations for time-travel navigation.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum GitOperationError {
    /// The requested commit could not be found.
    #[error("commit not found: {sha}")]
    CommitNotFound {
        /// The SHA that could not be found.
        sha: String,
    },

    /// The requested file could not be found at the specified commit.
    #[error("file '{path}' not found at commit {sha}")]
    FileNotFound {
        /// The file path that could not be found.
        path: String,
        /// The commit SHA where the file was expected.
        sha: String,
    },

    /// Failed to access the commit.
    #[error("failed to access commit {sha}: {message}")]
    CommitAccessFailed {
        /// The commit SHA.
        sha: String,
        /// Error details.
        message: String,
    },

    /// Failed to compute diff between commits.
    #[error("failed to compute diff: {message}")]
    DiffComputationFailed {
        /// Error details.
        message: String,
    },

    /// The repository is not available.
    #[error("repository not available: {message}")]
    RepositoryNotAvailable {
        /// Error details.
        message: String,
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

impl From<git2::Error> for GitOperationError {
    fn from(error: git2::Error) -> Self {
        Self::Git {
            message: error.message().to_owned(),
        }
    }
}
