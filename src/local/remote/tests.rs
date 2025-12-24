//! Unit tests for Git remote URL parsing.

use rstest::rstest;

use super::super::error::LocalDiscoveryError;
use super::{GitHubOrigin, parse_github_remote};

/// GitHub.com remote URL success test cases.
#[rstest]
#[case::ssh_scp_style("git@github.com:owner/repo.git", "owner", "repo")]
#[case::ssh_scp_style_no_git_suffix("git@github.com:owner/repo", "owner", "repo")]
#[case::https("https://github.com/owner/repo.git", "owner", "repo")]
#[case::https_no_git_suffix("https://github.com/owner/repo", "owner", "repo")]
#[case::ssh_url_style("ssh://git@github.com/owner/repo.git", "owner", "repo")]
#[case::case_insensitive("git@GitHub.COM:owner/repo.git", "owner", "repo")]
#[case::with_trailing_slash("https://github.com/owner/repo/", "owner", "repo")]
fn parse_github_com_origins(
    #[case] input: &str,
    #[case] expected_owner: &str,
    #[case] expected_repo: &str,
) {
    let result = parse_github_remote(input).expect("should parse successfully");

    assert!(
        matches!(result, GitHubOrigin::GitHubCom { .. }),
        "expected GitHubCom variant for {input}"
    );
    assert_eq!(result.owner(), expected_owner);
    assert_eq!(result.repository(), expected_repo);
}

/// Expected result for an Enterprise origin test case.
struct EnterpriseExpected {
    host: &'static str,
    port: Option<u16>,
    owner: &'static str,
    repo: &'static str,
}

/// GitHub Enterprise remote URL success test cases.
#[rstest]
#[case::ssh(
    "git@ghe.example.com:owner/repo.git",
    EnterpriseExpected { host: "ghe.example.com", port: None, owner: "owner", repo: "repo" }
)]
#[case::https(
    "https://ghe.example.com/owner/repo",
    EnterpriseExpected { host: "ghe.example.com", port: None, owner: "owner", repo: "repo" }
)]
#[case::https_with_port(
    "https://ghe.example.com:8443/owner/repo.git",
    EnterpriseExpected { host: "ghe.example.com", port: Some(8443), owner: "owner", repo: "repo" }
)]
fn parse_enterprise_origins(#[case] input: &str, #[case] expected: EnterpriseExpected) {
    let result = parse_github_remote(input).expect("should parse successfully");

    match result {
        GitHubOrigin::Enterprise {
            host,
            port,
            owner,
            repository,
        } => {
            assert_eq!(host, expected.host);
            assert_eq!(port, expected.port);
            assert_eq!(owner, expected.owner);
            assert_eq!(repository, expected.repo);
        }
        GitHubOrigin::GitHubCom { .. } => {
            panic!("expected Enterprise variant for {input}");
        }
    }
}

/// Error test cases for invalid remote URLs.
#[rstest]
#[case::empty_url("")]
#[case::invalid_url("not-a-url")]
#[case::url_missing_repo("https://github.com/owner")]
#[case::too_many_path_segments("https://github.com/owner/repo/extra")]
fn parse_invalid_urls_returns_error(#[case] input: &str) {
    let result = parse_github_remote(input);

    assert!(
        matches!(result, Err(LocalDiscoveryError::InvalidRemoteUrl { .. })),
        "expected InvalidRemoteUrl for '{input}', got {result:?}"
    );
}

/// `GitHubOrigin` accessor tests.
mod accessors {
    use super::*;

    #[test]
    fn github_com_origin() {
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
    fn enterprise_origin_no_port() {
        let origin = GitHubOrigin::Enterprise {
            host: "ghe.example.com".to_owned(),
            port: None,
            owner: "org".to_owned(),
            repository: "project".to_owned(),
        };

        assert_eq!(origin.owner(), "org");
        assert_eq!(origin.repository(), "project");
        assert_eq!(origin.host(), "ghe.example.com");
        assert!(!origin.is_github_com());
        assert_eq!(origin.port(), None);
    }

    #[test]
    fn enterprise_origin_with_port() {
        let origin = GitHubOrigin::Enterprise {
            host: "ghe.example.com".to_owned(),
            port: Some(8443),
            owner: "org".to_owned(),
            repository: "project".to_owned(),
        };

        assert_eq!(origin.port(), Some(8443));
    }
}
