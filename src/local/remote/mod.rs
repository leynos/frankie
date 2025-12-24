//! Git remote URL parsing with GitHub origin detection.
//!
//! This module handles parsing of various Git remote URL formats to extract
//! owner and repository information for GitHub origins.

use super::error::LocalDiscoveryError;

/// Represents a parsed GitHub origin with owner and repository.
///
/// Distinguishes between standard `github.com` repositories and GitHub
/// Enterprise installations on custom hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubOrigin {
    /// Standard `github.com` repository.
    GitHubCom {
        /// Repository owner (user or organisation).
        owner: String,
        /// Repository name.
        repository: String,
    },
    /// GitHub Enterprise repository on a custom host.
    Enterprise {
        /// The GitHub Enterprise host (e.g., `ghe.example.com`).
        host: String,
        /// Optional port number for non-default HTTPS ports.
        port: Option<u16>,
        /// Repository owner (user or organisation).
        owner: String,
        /// Repository name.
        repository: String,
    },
}

impl GitHubOrigin {
    /// Returns the repository owner.
    #[must_use]
    pub fn owner(&self) -> &str {
        match self {
            Self::GitHubCom { owner, .. } | Self::Enterprise { owner, .. } => owner,
        }
    }

    /// Returns the repository name.
    #[must_use]
    pub fn repository(&self) -> &str {
        match self {
            Self::GitHubCom { repository, .. } | Self::Enterprise { repository, .. } => repository,
        }
    }

    /// Returns the host for this origin.
    #[must_use]
    pub fn host(&self) -> &str {
        match self {
            Self::GitHubCom { .. } => "github.com",
            Self::Enterprise { host, .. } => host,
        }
    }

    /// Returns true if this is a standard `github.com` origin.
    #[must_use]
    pub const fn is_github_com(&self) -> bool {
        matches!(self, Self::GitHubCom { .. })
    }

    /// Returns the port for this origin, if any.
    ///
    /// Returns `None` for `github.com` origins and Enterprise origins using
    /// the default HTTPS port.
    #[must_use]
    pub const fn port(&self) -> Option<u16> {
        match self {
            Self::GitHubCom { .. } => None,
            Self::Enterprise { port, .. } => *port,
        }
    }
}

/// Parses a Git remote URL and extracts GitHub origin information.
///
/// Supports the following URL formats:
/// - SSH: `git@github.com:owner/repo.git`
/// - SSH with protocol: `ssh://git@github.com/owner/repo.git`
/// - SSH with port: `ssh://git@github.com:22/owner/repo.git`
/// - HTTPS: `https://github.com/owner/repo.git`
/// - HTTPS: `https://github.com/owner/repo`
///
/// The `.git` suffix is optional and stripped if present.
///
/// # Errors
///
/// Returns `LocalDiscoveryError::InvalidRemoteUrl` if the URL cannot be parsed.
pub fn parse_github_remote(url: &str) -> Result<GitHubOrigin, LocalDiscoveryError> {
    let trimmed = url.trim();

    if trimmed.is_empty() {
        return Err(LocalDiscoveryError::InvalidRemoteUrl {
            url: url.to_owned(),
        });
    }

    // Try SSH SCP-style format first: git@host:owner/repo.git
    if let Some(origin) = try_parse_scp_style(trimmed) {
        return Ok(origin);
    }

    // Try URL-style formats (https://, ssh://, git://)
    if let Some(origin) = try_parse_url_style(trimmed) {
        return Ok(origin);
    }

    Err(LocalDiscoveryError::InvalidRemoteUrl {
        url: url.to_owned(),
    })
}

/// Attempts to parse SCP-style SSH URL: `git@host:owner/repo.git`
///
/// SCP-style URLs do not support port numbers, so port is always `None`.
fn try_parse_scp_style(url: &str) -> Option<GitHubOrigin> {
    // Pattern: user@host:path
    let at_pos = url.find('@')?;
    let colon_pos = url.find(':')?;

    // Colon must come after @
    if colon_pos <= at_pos {
        return None;
    }

    // If there's a :// this is URL-style, not SCP-style
    if url.get(colon_pos..colon_pos.saturating_add(3)) == Some("://") {
        return None;
    }

    let host = url.get(at_pos.saturating_add(1)..colon_pos)?;
    let path = url.get(colon_pos.saturating_add(1)..)?;

    // SCP-style URLs don't have port numbers
    extract_owner_repo_from_path(host, None, path)
}

/// Attempts to parse URL-style remote: `https://host/owner/repo.git`
fn try_parse_url_style(url: &str) -> Option<GitHubOrigin> {
    // Parse as URL
    let parsed = url::Url::parse(url).ok()?;

    let host = parsed.host_str()?;
    let port = parsed.port();
    // Path should start with /
    let path_stripped = parsed.path().strip_prefix('/')?;

    extract_owner_repo_from_path(host, port, path_stripped)
}

/// Extracts owner and repository from a path like `owner/repo.git`.
fn extract_owner_repo_from_path(
    host: &str,
    port: Option<u16>,
    raw_path: &str,
) -> Option<GitHubOrigin> {
    let trimmed_path = raw_path.trim_matches('/');

    if trimmed_path.is_empty() {
        return None;
    }

    // Split by /
    let mut parts = trimmed_path.split('/');
    let owner_str = parts.next()?;
    let repo_with_suffix = parts.next()?;

    // Only allow owner/repo, not owner/repo/extra/stuff
    // But we should allow empty trailing parts from trailing slashes
    let extra = parts.next();
    if extra.is_some_and(|s| !s.is_empty()) {
        return None;
    }

    if owner_str.is_empty() || repo_with_suffix.is_empty() {
        return None;
    }

    // Strip .git suffix if present
    let repo_name = repo_with_suffix
        .strip_suffix(".git")
        .unwrap_or(repo_with_suffix);

    if repo_name.is_empty() {
        return None;
    }

    let owner = owner_str.to_owned();
    let repository = repo_name.to_owned();

    if host.eq_ignore_ascii_case("github.com") {
        Some(GitHubOrigin::GitHubCom { owner, repository })
    } else {
        Some(GitHubOrigin::Enterprise {
            host: host.to_owned(),
            port,
            owner,
            repository,
        })
    }
}

#[cfg(test)]
mod tests;
