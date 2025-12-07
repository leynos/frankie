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

        let segments = if let Some(segments) = parsed.path_segments() {
            segments.collect::<Vec<_>>()
        } else {
            return Err(IntakeError::MissingPathSegments);
        };

        let (owner_segment, repository_segment, marker, number_segment) = match segments.as_slice()
        {
            [owner, repository, marker, number, ..] => (*owner, *repository, *marker, *number),
            _ => return Err(IntakeError::MissingPathSegments),
        };

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

        let host = parsed
            .host_str()
            .ok_or_else(|| IntakeError::InvalidUrl("URL must include a host".to_owned()))?;

        let api_base = if host.eq_ignore_ascii_case("github.com") {
            Url::parse("https://api.github.com")
                .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?
        } else {
            let mut api_url = Url::parse(&format!("{}://{}", parsed.scheme(), host))
                .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

            api_url
                .set_port(parsed.port())
                .map_err(|()| IntakeError::InvalidUrl("invalid port".to_owned()))?;
            api_url.set_path("api/v3");
            api_url
        };

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
