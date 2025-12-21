//! URL parsing and identity wrappers for pull request intake.

use url::Url;

use super::error::IntakeError;

/// Repository owner wrapper to avoid stringly typed parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryOwner(String);

impl RepositoryOwner {
    pub(crate) fn new(value: &str) -> Result<Self, IntakeError> {
        if value.is_empty() {
            return Err(IntakeError::MissingPathSegments);
        }
        Ok(Self(value.to_owned()))
    }

    /// Borrow the owner value.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Repository name wrapper to prevent parameter mix-ups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryName(String);

impl RepositoryName {
    pub(crate) fn new(value: &str) -> Result<Self, IntakeError> {
        if value.is_empty() {
            return Err(IntakeError::MissingPathSegments);
        }
        Ok(Self(value.to_owned()))
    }

    /// Borrow the repository name.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Pull request number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PullRequestNumber(u64);

impl PullRequestNumber {
    pub(crate) const fn new(value: u64) -> Result<Self, IntakeError> {
        if value == 0 {
            return Err(IntakeError::InvalidPullRequestNumber);
        }
        Ok(Self(value))
    }

    /// Returns the numeric value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Personal access token wrapper enforcing presence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalAccessToken(String);

impl PersonalAccessToken {
    /// Validates that the token is non-empty and trims whitespace.
    ///
    /// # Errors
    ///
    /// Returns `IntakeError::MissingToken` when the supplied string is blank.
    pub fn new(token: impl AsRef<str>) -> Result<Self, IntakeError> {
        let trimmed = token.as_ref().trim();
        if trimmed.is_empty() {
            return Err(IntakeError::MissingToken);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the token value.
    #[must_use]
    pub const fn value(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for PersonalAccessToken {
    fn as_ref(&self) -> &str {
        self.value()
    }
}

/// Derives the GitHub API base URL from a host string.
fn derive_api_base_from_host(
    scheme: &str,
    host: &str,
    port: Option<u16>,
) -> Result<Url, IntakeError> {
    if host.eq_ignore_ascii_case("github.com") {
        Url::parse("https://api.github.com")
            .map_err(|error| IntakeError::InvalidUrl(error.to_string()))
    } else {
        let authority = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        let mut api_url = Url::parse(&format!("{scheme}://{authority}"))
            .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

        api_url
            .set_port(port)
            .map_err(|()| IntakeError::InvalidUrl("invalid port".to_owned()))?;
        api_url.set_path("api/v3");
        Ok(api_url)
    }
}

/// Derives the GitHub API base URL from a parsed URL.
fn derive_api_base(parsed: &Url) -> Result<Url, IntakeError> {
    let host = parsed
        .host_str()
        .ok_or_else(|| IntakeError::InvalidUrl("URL must include a host".to_owned()))?;

    derive_api_base_from_host(parsed.scheme(), host, parsed.port())
}

/// Parsed pull request URL and derived API base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestLocator {
    api_base: Url,
    owner: RepositoryOwner,
    repository: RepositoryName,
    number: PullRequestNumber,
}

impl PullRequestLocator {
    /// Parses a GitHub pull request URL in the form
    /// `https://github.com/<owner>/<repo>/pull/<number>`.
    ///
    /// # Errors
    ///
    /// Returns `IntakeError::InvalidUrl` when parsing fails, `MissingPathSegments`
    /// when the URL path is not `/owner/repo/pull/<number>`, and
    /// `InvalidPullRequestNumber` when the final segment is not a positive
    /// integer.
    pub fn parse(input: &str) -> Result<Self, IntakeError> {
        let parsed =
            Url::parse(input).map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

        let mut segments = parsed
            .path_segments()
            .ok_or(IntakeError::MissingPathSegments)?;

        let owner_segment = segments.next().ok_or(IntakeError::MissingPathSegments)?;
        let repository_segment = segments.next().ok_or(IntakeError::MissingPathSegments)?;
        let marker = segments.next().ok_or(IntakeError::MissingPathSegments)?;
        let number_segment = segments.next().ok_or(IntakeError::MissingPathSegments)?;

        if marker != "pull" {
            return Err(IntakeError::MissingPathSegments);
        }

        if number_segment.is_empty() {
            return Err(IntakeError::MissingPathSegments);
        }

        let owner = RepositoryOwner::new(owner_segment)?;
        let repository = RepositoryName::new(repository_segment)?;
        let number = number_segment
            .parse::<u64>()
            .map_err(|_| IntakeError::InvalidPullRequestNumber)
            .and_then(PullRequestNumber::new)?;

        let api_base = derive_api_base(&parsed)?;

        Ok(Self {
            api_base,
            owner,
            repository,
            number,
        })
    }

    /// API base URL derived from the pull request host.
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

    /// Pull request number.
    #[must_use]
    pub const fn number(&self) -> PullRequestNumber {
        self.number
    }

    pub(crate) fn pull_request_path(&self) -> String {
        format!(
            "/repos/{}/{}/pulls/{}",
            self.owner.as_str(),
            self.repository.as_str(),
            self.number.get()
        )
    }

    pub(crate) fn comments_path(&self) -> String {
        format!(
            "/repos/{}/{}/issues/{}/comments",
            self.owner.as_str(),
            self.repository.as_str(),
            self.number.get()
        )
    }
}

/// Parsed repository URL with derived API base.
///
/// Unlike `PullRequestLocator`, this type represents a repository without
/// a specific pull request number, suitable for listing operations.
///
/// # Example
///
/// ```
/// use frankie::github::locator::RepositoryLocator;
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

        let mut segments = parsed
            .path_segments()
            .ok_or(IntakeError::MissingPathSegments)?;

        let owner_segment = segments.next().ok_or(IntakeError::MissingPathSegments)?;
        let repository_segment = segments.next().ok_or(IntakeError::MissingPathSegments)?;

        let owner = RepositoryOwner::new(owner_segment)?;
        let repository = RepositoryName::new(repository_segment)?;
        let api_base = derive_api_base(&parsed)?;

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
    /// use frankie::github::locator::RepositoryLocator;
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
                owner,
                repository,
            } => {
                // Build a URL to parse and derive API base
                let url = format!("https://{host}/{owner}/{repository}");
                Self::parse(&url)
            }
        }
    }
}
