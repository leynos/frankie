//! Unit tests for interactive mode.

mod run_discovered_repository;

mod discovery_error_handling {
    use frankie::IntakeError;
    use frankie::local::LocalDiscoveryError;
    use rstest::rstest;

    use super::super::handle_discovery_error;

    /// Returns the expected error message for missing arguments.
    fn expected_missing_args_message() -> &'static str {
        "either --pr-url/-u or --owner/-o with --repo/-r is required"
    }

    #[rstest]
    #[case::not_a_repository(LocalDiscoveryError::NotARepository, "NotARepository")]
    #[case::no_remotes(LocalDiscoveryError::NoRemotes, "NoRemotes")]
    #[case::remote_not_found(
        LocalDiscoveryError::RemoteNotFound { name: "upstream".to_owned() },
        "RemoteNotFound"
    )]
    #[case::invalid_remote_url(
        LocalDiscoveryError::InvalidRemoteUrl { url: "not-a-url".to_owned() },
        "InvalidRemoteUrl"
    )]
    fn discovery_errors_return_missing_arguments_error(
        #[case] error: LocalDiscoveryError,
        #[case] variant_name: &str,
    ) {
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

    #[rstest]
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

    use crate::cli::default_listing_params;

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
            port: None,
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

    #[test]
    fn enterprise_origin_preserves_custom_port_in_api_base() {
        let origin = GitHubOrigin::Enterprise {
            host: "ghe.example.com".to_owned(),
            port: Some(8443),
            owner: "org".to_owned(),
            repository: "project".to_owned(),
        };

        let locator = RepositoryLocator::from_github_origin(&origin)
            .expect("should create locator from Enterprise origin with port");

        assert!(
            locator
                .api_base()
                .as_str()
                .starts_with("https://ghe.example.com:8443/api/v3"),
            "API base should preserve custom port: {}",
            locator.api_base()
        );
    }
}
