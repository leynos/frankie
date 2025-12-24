//! Interactive mode with local repository discovery.

use std::io::{self, Write};
use std::path::Path;

use frankie::local::{LocalDiscoveryError, LocalRepository, discover_repository};
use frankie::{
    FrankieConfig, IntakeError, ListPullRequestsParams, OctocrabRepositoryGateway,
    PersonalAccessToken, PullRequestState, RepositoryIntake, RepositoryLocator,
};

use super::output::write_listing_summary;

/// Runs in interactive mode, attempting local repository discovery.
///
/// # Errors
///
/// Returns [`IntakeError::Configuration`] if local discovery is disabled or discovery fails.
/// Returns [`IntakeError::LocalDiscovery`] for Git errors during discovery.
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    if config.no_local_discovery {
        return Err(missing_arguments_error());
    }

    match discover_repository(Path::new(".")) {
        Ok(local_repo) => run_discovered_repository(config, &local_repo).await,
        Err(error) => handle_discovery_error(error),
    }
}

/// Runs repository listing using a discovered local repository.
async fn run_discovered_repository(
    config: &FrankieConfig,
    local_repo: &LocalRepository,
) -> Result<(), IntakeError> {
    let owner = local_repo.owner();
    let repo = local_repo.repository();

    // Log the discovery to stderr (ignore write errors)
    drop(writeln!(
        io::stderr(),
        "Discovered repository from local Git: {owner}/{repo}"
    ));

    let token_value = config.resolve_token()?;
    let locator = RepositoryLocator::from_github_origin(local_repo.github_origin())?;
    let token = PersonalAccessToken::new(token_value)?;

    let gateway = OctocrabRepositoryGateway::for_token(&token, &locator)?;
    let intake = RepositoryIntake::new(&gateway);

    let result = intake
        .list_pull_requests(&locator, &default_listing_params())
        .await?;
    let mut stdout = io::stdout().lock();
    write_listing_summary(&mut stdout, &result, owner, repo)
}

/// Handles discovery errors, printing warnings where appropriate.
fn handle_discovery_error(error: LocalDiscoveryError) -> Result<(), IntakeError> {
    match error {
        LocalDiscoveryError::NotARepository => {
            // Silent fallthrough - user is not in a repo
            Err(missing_arguments_error())
        }
        LocalDiscoveryError::NoRemotes => {
            drop(writeln!(
                io::stderr(),
                "Warning: Git repository has no remotes configured"
            ));
            Err(missing_arguments_error())
        }
        LocalDiscoveryError::RemoteNotFound { name } => {
            drop(writeln!(
                io::stderr(),
                "Warning: remote '{name}' not found in repository"
            ));
            Err(missing_arguments_error())
        }
        LocalDiscoveryError::InvalidRemoteUrl { url } => {
            drop(writeln!(
                io::stderr(),
                "Warning: could not parse remote URL: {url}"
            ));
            Err(missing_arguments_error())
        }
        LocalDiscoveryError::Git { message } => Err(IntakeError::LocalDiscovery { message }),
    }
}

/// Returns the standard error for missing CLI arguments.
fn missing_arguments_error() -> IntakeError {
    IntakeError::Configuration {
        message: concat!(
            "either --pr-url/-u or --owner/-o with --repo/-r is required\n",
            "Run 'frankie --help' for usage information"
        )
        .to_owned(),
    }
}

/// Returns the default parameters for listing pull requests.
const fn default_listing_params() -> ListPullRequestsParams {
    ListPullRequestsParams {
        state: Some(PullRequestState::All),
        per_page: Some(50),
        page: Some(1),
    }
}

#[cfg(test)]
mod tests {
    mod discovery_error_handling {
        use frankie::IntakeError;
        use frankie::local::LocalDiscoveryError;

        use super::super::handle_discovery_error;

        /// Returns the expected error message for missing arguments.
        fn expected_missing_args_message() -> &'static str {
            "either --pr-url/-u or --owner/-o with --repo/-r is required"
        }

        fn assert_returns_missing_arguments_error(error: LocalDiscoveryError, variant_name: &str) {
            let result = handle_discovery_error(error);

            match result {
                Err(IntakeError::Configuration { message }) => {
                    assert!(
                        message.contains(expected_missing_args_message()),
                        "{variant_name} should return missing arguments error, got: {message}"
                    );
                }
                other => panic!("expected Configuration error, got: {other:?}"),
            }
        }

        #[test]
        fn not_a_repository_returns_missing_arguments_error() {
            assert_returns_missing_arguments_error(
                LocalDiscoveryError::NotARepository,
                "NotARepository",
            );
        }

        #[test]
        fn no_remotes_returns_missing_arguments_error() {
            assert_returns_missing_arguments_error(LocalDiscoveryError::NoRemotes, "NoRemotes");
        }

        #[test]
        fn remote_not_found_returns_missing_arguments_error() {
            assert_returns_missing_arguments_error(
                LocalDiscoveryError::RemoteNotFound {
                    name: "upstream".to_owned(),
                },
                "RemoteNotFound",
            );
        }

        #[test]
        fn invalid_remote_url_returns_missing_arguments_error() {
            assert_returns_missing_arguments_error(
                LocalDiscoveryError::InvalidRemoteUrl {
                    url: "not-a-url".to_owned(),
                },
                "InvalidRemoteUrl",
            );
        }

        #[test]
        fn git_error_returns_local_discovery_error_with_message() {
            let error_message = "repository corrupt";
            let result = handle_discovery_error(LocalDiscoveryError::Git {
                message: error_message.to_owned(),
            });

            match result {
                Err(IntakeError::LocalDiscovery { message }) => {
                    assert_eq!(
                        message, error_message,
                        "Git error should preserve original message"
                    );
                }
                other => panic!("expected LocalDiscovery error, got: {other:?}"),
            }
        }
    }

    mod interactive_mode {
        use frankie::IntakeError;

        use super::super::{FrankieConfig, run};

        #[tokio::test]
        #[expect(clippy::excessive_nesting, reason = "nested test module structure")]
        async fn no_local_discovery_returns_missing_arguments_error() {
            let config = FrankieConfig {
                no_local_discovery: true,
                ..Default::default()
            };

            let result = run(&config).await;
            let Err(IntakeError::Configuration { message }) = result else {
                panic!("expected Configuration error, got: {result:?}");
            };

            assert!(
                message.contains("--pr-url"),
                "error should mention --pr-url: {message}"
            );
            assert!(
                message.contains("--owner"),
                "error should mention --owner: {message}"
            );
            assert!(
                message.contains("--repo"),
                "error should mention --repo: {message}"
            );
        }
    }

    mod default_listing_params {
        use frankie::github::PullRequestState;

        use super::super::default_listing_params;

        #[test]
        fn returns_all_state() {
            let params = default_listing_params();
            assert_eq!(
                params.state,
                Some(PullRequestState::All),
                "default state should be All"
            );
        }

        #[test]
        fn returns_page_size_of_50() {
            let params = default_listing_params();
            assert_eq!(params.per_page, Some(50), "default per_page should be 50");
        }

        #[test]
        fn returns_first_page() {
            let params = default_listing_params();
            assert_eq!(params.page, Some(1), "default page should be 1");
        }
    }

    mod repository_locator_from_github_origin {
        use frankie::RepositoryLocator;
        use frankie::local::GitHubOrigin;

        #[test]
        fn github_com_origin_produces_github_api_locator() {
            let origin = GitHubOrigin::GitHubCom {
                owner: "octo".to_owned(),
                repository: "cat".to_owned(),
            };

            let locator = RepositoryLocator::from_github_origin(&origin)
                .expect("should create locator from GitHubCom origin");

            assert_eq!(locator.owner().as_str(), "octo");
            assert_eq!(locator.repository().as_str(), "cat");
            assert_eq!(locator.api_base().as_str(), "https://api.github.com/");
        }

        #[test]
        fn enterprise_origin_produces_enterprise_api_locator() {
            let origin = GitHubOrigin::Enterprise {
                host: "ghe.example.com".to_owned(),
                owner: "org".to_owned(),
                repository: "project".to_owned(),
            };

            let locator = RepositoryLocator::from_github_origin(&origin)
                .expect("should create locator from Enterprise origin");

            assert_eq!(locator.owner().as_str(), "org");
            assert_eq!(locator.repository().as_str(), "project");
            assert!(
                locator
                    .api_base()
                    .as_str()
                    .starts_with("https://ghe.example.com/api/v3"),
                "API base should point to Enterprise server: {}",
                locator.api_base()
            );
        }
    }
}
