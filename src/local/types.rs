//! Domain-specific types for Git operations.
//!
//! This module provides newtype wrappers for common Git-related strings,
//! improving type safety and making APIs more self-documenting.

use std::fmt;

/// A Git commit SHA identifier.
///
/// This newtype wrapper provides type safety for commit SHA strings,
/// preventing accidental misuse of unrelated string values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitSha(String);

impl CommitSha {
    /// Creates a new `CommitSha` from a string.
    #[must_use]
    pub const fn new(sha: String) -> Self {
        Self(sha)
    }

    /// Returns the SHA as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommitSha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for CommitSha {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for CommitSha {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A file path within a Git repository.
///
/// This newtype wrapper provides type safety for repository file paths,
/// preventing accidental misuse of unrelated string values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoFilePath(String);

impl RepoFilePath {
    /// Creates a new `RepoFilePath` from a string.
    #[must_use]
    pub const fn new(path: String) -> Self {
        Self(path)
    }

    /// Returns the path as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoFilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for RepoFilePath {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for RepoFilePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
