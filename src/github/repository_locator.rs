//! Repository URL parsing and locator for listing operations.
//!
//! Provides [`RepositoryLocator`] for identifying a GitHub repository (without
//! a specific pull request) via URL parsing, owner/repo strings, or local
//! git discovery.

use url::Url;

use super::error::IntakeError;
use super::locator::{RepositoryName, RepositoryOwner, parse_owner_repo_and_api};

/// Parsed repository URL with derived API base.
///
/// Unlike `PullRequestLocator`, this type represents a repository without
/// a specific pull request number, suitable for listing operations.
///
/// # Example
///
/// ```
/// use frankie::RepositoryLocator;
///
/// let locator = RepositoryLocator::parse("https://github.com/octo/repo")
///     .expect("should parse repository URL");
/// assert_eq!(locator.owner().as_str(), "octo");
/// assert_eq!(locator.repository().as_str(), "repo");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryLocator {
    api_base: Url,
    owner: RepositoryOwner,
    repository: RepositoryName,
}

impl RepositoryLocator {
    /// Creates a repository locator from owner and repository name strings.
    ///
    /// Uses `github.com` as the default host.
    ///
    /// # Errors
    ///
    /// Returns `IntakeError::MissingPathSegments` when owner or repo is empty.
    pub fn from_owner_repo(owner: &str, repo: &str) -> Result<Self, IntakeError> {
        let validated_owner = RepositoryOwner::new(owner)?;
        let repository = RepositoryName::new(repo)?;
        let api_base = Url::parse("https://api.github.com")
            .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

        Ok(Self {
            api_base,
            owner: validated_owner,
            repository,
        })
    }

    /// Parses a GitHub repository URL in the form
    /// `https://github.com/<owner>/<repo>`.
    ///
    /// # Errors
    ///
    /// Returns `IntakeError::InvalidUrl` when parsing fails or
    /// `MissingPathSegments` when the URL path is not `/owner/repo`.
    pub fn parse(input: &str) -> Result<Self, IntakeError> {
        let parsed =
            Url::parse(input).map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

        let (owner, repository, api_base) = parse_owner_repo_and_api(&parsed)?;

        Ok(Self {
            api_base,
            owner,
            repository,
        })
    }

    /// API base URL derived from the repository host.
    #[must_use]
    pub const fn api_base(&self) -> &Url {
        &self.api_base
    }

    /// Repository owner.
    #[must_use]
    pub const fn owner(&self) -> &RepositoryOwner {
        &self.owner
    }

    /// Repository name.
    #[must_use]
    pub const fn repository(&self) -> &RepositoryName {
        &self.repository
    }

    /// Returns the API path for listing pull requests.
    pub(crate) fn pulls_path(&self) -> String {
        format!(
            "/repos/{}/{}/pulls",
            self.owner.as_str(),
            self.repository.as_str()
        )
    }

    /// Creates a repository locator from a discovered GitHub origin.
    ///
    /// For standard `github.com` origins, uses the public API base. For GitHub
    /// Enterprise origins, derives the API base from the host.
    ///
    /// # Errors
    ///
    /// Returns `IntakeError::MissingPathSegments` if owner or repo is empty, or
    /// `IntakeError::InvalidUrl` if the URL cannot be parsed.
    ///
    /// # Example
    ///
    /// ```
    /// use frankie::RepositoryLocator;
    /// use frankie::local::GitHubOrigin;
    ///
    /// let origin = GitHubOrigin::GitHubCom {
    ///     owner: "octo".to_owned(),
    ///     repository: "cat".to_owned(),
    /// };
    /// let locator = RepositoryLocator::from_github_origin(&origin)
    ///     .expect("should create locator");
    /// assert_eq!(locator.owner().as_str(), "octo");
    /// assert_eq!(locator.repository().as_str(), "cat");
    /// ```
    pub fn from_github_origin(origin: &crate::local::GitHubOrigin) -> Result<Self, IntakeError> {
        match origin {
            crate::local::GitHubOrigin::GitHubCom { owner, repository } => {
                Self::from_owner_repo(owner, repository)
            }
            crate::local::GitHubOrigin::Enterprise {
                host,
                port,
                owner,
                repository,
            } => {
                // Build a URL to parse and derive API base, preserving port
                let url = port.map_or_else(
                    || format!("https://{host}/{owner}/{repository}"),
                    |p| format!("https://{host}:{p}/{owner}/{repository}"),
                );
                Self::parse(&url)
            }
        }
    }
}
