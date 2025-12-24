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
mod tests {
    use super::*;

    /// Test case for successful parsing of a GitHub remote URL.
    struct TestCase {
        name: &'static str,
        input: &'static str,
        expected: GitHubOrigin,
    }

    /// Test case for parsing failures.
    struct ErrorTestCase {
        name: &'static str,
        input: &'static str,
    }

    /// GitHub.com remote URL test cases.
    fn github_com_test_cases() -> Vec<TestCase> {
        vec![
            TestCase {
                name: "ssh_scp_style_github_com",
                input: "git@github.com:owner/repo.git",
                expected: GitHubOrigin::GitHubCom {
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
            TestCase {
                name: "ssh_scp_style_no_git_suffix",
                input: "git@github.com:owner/repo",
                expected: GitHubOrigin::GitHubCom {
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
            TestCase {
                name: "https_github_com",
                input: "https://github.com/owner/repo.git",
                expected: GitHubOrigin::GitHubCom {
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
            TestCase {
                name: "https_no_git_suffix",
                input: "https://github.com/owner/repo",
                expected: GitHubOrigin::GitHubCom {
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
            TestCase {
                name: "ssh_url_style",
                input: "ssh://git@github.com/owner/repo.git",
                expected: GitHubOrigin::GitHubCom {
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
            TestCase {
                name: "case_insensitive_github_com",
                input: "git@GitHub.COM:owner/repo.git",
                expected: GitHubOrigin::GitHubCom {
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
            TestCase {
                name: "with_trailing_slash",
                input: "https://github.com/owner/repo/",
                expected: GitHubOrigin::GitHubCom {
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
        ]
    }

    /// GitHub Enterprise remote URL test cases.
    fn github_enterprise_test_cases() -> Vec<TestCase> {
        vec![
            TestCase {
                name: "github_enterprise_ssh",
                input: "git@ghe.example.com:owner/repo.git",
                expected: GitHubOrigin::Enterprise {
                    host: "ghe.example.com".to_owned(),
                    port: None,
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
            TestCase {
                name: "github_enterprise_https",
                input: "https://ghe.example.com/owner/repo",
                expected: GitHubOrigin::Enterprise {
                    host: "ghe.example.com".to_owned(),
                    port: None,
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
            TestCase {
                name: "github_enterprise_https_with_port",
                input: "https://ghe.example.com:8443/owner/repo.git",
                expected: GitHubOrigin::Enterprise {
                    host: "ghe.example.com".to_owned(),
                    port: Some(8443),
                    owner: "owner".to_owned(),
                    repository: "repo".to_owned(),
                },
            },
        ]
    }

    /// All successful parsing test cases (GitHub.com and Enterprise).
    fn success_test_cases() -> Vec<TestCase> {
        let mut cases = github_com_test_cases();
        cases.extend(github_enterprise_test_cases());
        cases
    }

    #[test]
    fn parse_github_remote_success_cases() {
        for case in success_test_cases() {
            let result = parse_github_remote(case.input);
            assert_eq!(
                result,
                Ok(case.expected),
                "test case '{}' failed for input '{}'",
                case.name,
                case.input
            );
        }
    }

    #[test]
    fn parse_github_remote_error_cases() {
        let cases = vec![
            ErrorTestCase {
                name: "empty_url",
                input: "",
            },
            ErrorTestCase {
                name: "invalid_url",
                input: "not-a-url",
            },
            ErrorTestCase {
                name: "url_missing_repo",
                input: "https://github.com/owner",
            },
            ErrorTestCase {
                name: "too_many_path_segments",
                input: "https://github.com/owner/repo/extra",
            },
        ];

        for case in cases {
            let result = parse_github_remote(case.input);
            assert!(
                matches!(result, Err(LocalDiscoveryError::InvalidRemoteUrl { .. })),
                "test case '{}' should fail for input '{}', but got {:?}",
                case.name,
                case.input,
                result
            );
        }
    }

    #[test]
    fn github_com_origin_accessors() {
        let origin = GitHubOrigin::GitHubCom {
            owner: "octo".to_owned(),
            repository: "cat".to_owned(),
        };
        assert_eq!(origin.owner(), "octo");
        assert_eq!(origin.repository(), "cat");
        assert_eq!(origin.host(), "github.com");
        assert!(origin.is_github_com());
        assert_eq!(origin.port(), None);
    }

    #[test]
    fn enterprise_origin_accessors() {
        let enterprise = GitHubOrigin::Enterprise {
            host: "ghe.example.com".to_owned(),
            port: None,
            owner: "org".to_owned(),
            repository: "project".to_owned(),
        };
        assert_eq!(enterprise.owner(), "org");
        assert_eq!(enterprise.repository(), "project");
        assert_eq!(enterprise.host(), "ghe.example.com");
        assert!(!enterprise.is_github_com());
        assert_eq!(enterprise.port(), None);
    }

    #[test]
    fn enterprise_origin_with_port_accessor() {
        let enterprise_with_port = GitHubOrigin::Enterprise {
            host: "ghe.example.com".to_owned(),
            port: Some(8443),
            owner: "org".to_owned(),
            repository: "project".to_owned(),
        };
        assert_eq!(enterprise_with_port.port(), Some(8443));
    }
}
