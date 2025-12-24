//! Frankie CLI entrypoint for pull request intake.

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use frankie::github::RepositoryGateway;
use frankie::local::{LocalDiscoveryError, LocalRepository, discover_repository};
use frankie::persistence::{PersistenceError, migrate_database};
use frankie::telemetry::StderrJsonlTelemetrySink;
use frankie::{
    FrankieConfig, IntakeError, ListPullRequestsParams, OctocrabCachingGateway, OctocrabGateway,
    OctocrabRepositoryGateway, OperationMode, PaginatedPullRequests, PersonalAccessToken,
    PullRequestDetails, PullRequestIntake, PullRequestLocator, PullRequestState, RepositoryIntake,
    RepositoryLocator,
};
use ortho_config::OrthoConfig;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if writeln!(io::stderr().lock(), "{error}").is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), IntakeError> {
    let config = load_config()?;

    if config.migrate_db {
        return run_database_migrations(&config);
    }

    match config.operation_mode() {
        OperationMode::SinglePullRequest => run_single_pr(&config).await,
        OperationMode::RepositoryListing => run_repository_listing(&config).await,
        OperationMode::Interactive => run_interactive(&config).await,
    }
}

/// Runs in interactive mode, attempting local repository discovery.
async fn run_interactive(config: &FrankieConfig) -> Result<(), IntakeError> {
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

fn run_database_migrations(config: &FrankieConfig) -> Result<(), IntakeError> {
    let database_url =
        config
            .database_url
            .as_deref()
            .ok_or_else(|| IntakeError::Configuration {
                message: PersistenceError::MissingDatabaseUrl.to_string(),
            })?;

    let telemetry = StderrJsonlTelemetrySink;
    migrate_database(database_url, &telemetry)
        .map(drop)
        .map_err(|error| map_persistence_error(&error))
}

/// Maps a persistence error to an intake error.
///
/// Configuration-related errors (blank URL) become [`IntakeError::Configuration`],
/// while runtime errors (connection, migration, query failures) become
/// [`IntakeError::Io`].
fn map_persistence_error(error: &PersistenceError) -> IntakeError {
    if is_configuration_error(error) {
        IntakeError::Configuration {
            message: error.to_string(),
        }
    } else {
        IntakeError::Io {
            message: error.to_string(),
        }
    }
}

/// Returns true if the persistence error is a configuration problem.
const fn is_configuration_error(error: &PersistenceError) -> bool {
    matches!(error, PersistenceError::BlankDatabaseUrl)
}

/// Loads a single pull request by URL.
async fn run_single_pr(config: &FrankieConfig) -> Result<(), IntakeError> {
    let pr_url = config.require_pr_url()?;
    let token_value = config.resolve_token()?;

    let locator = PullRequestLocator::parse(pr_url)?;
    let token = PersonalAccessToken::new(token_value)?;

    let details = if let Some(database_url) = config.database_url.as_deref() {
        let gateway = OctocrabCachingGateway::for_token(
            &token,
            &locator,
            database_url,
            config.pr_metadata_cache_ttl_seconds,
        )?;
        let intake = PullRequestIntake::new(&gateway);
        intake.load(&locator).await?
    } else {
        let gateway = OctocrabGateway::for_token(&token, &locator)?;
        let intake = PullRequestIntake::new(&gateway);
        intake.load(&locator).await?
    };

    write_pr_summary(&details)
}

/// Lists pull requests for a repository.
async fn run_repository_listing(config: &FrankieConfig) -> Result<(), IntakeError> {
    let mut stdout = io::stdout().lock();
    run_repository_listing_with_gateway_builder(
        config,
        OctocrabRepositoryGateway::for_token,
        &mut stdout,
    )
    .await
}

async fn run_repository_listing_with_gateway_builder<G, F, W>(
    config: &FrankieConfig,
    build_gateway: F,
    writer: &mut W,
) -> Result<(), IntakeError>
where
    G: RepositoryGateway,
    F: FnOnce(&PersonalAccessToken, &RepositoryLocator) -> Result<G, IntakeError>,
    W: Write,
{
    let (owner, repo) = config.require_repository_info()?;
    let token_value = config.resolve_token()?;

    let locator = RepositoryLocator::from_owner_repo(owner, repo)?;
    let token = PersonalAccessToken::new(token_value)?;

    let gateway = build_gateway(&token, &locator)?;
    let intake = RepositoryIntake::new(&gateway);

    let result = intake
        .list_pull_requests(&locator, &default_listing_params())
        .await?;
    write_listing_summary(writer, &result, owner, repo)
}

/// Loads configuration from CLI, environment, and files.
///
/// # Errors
///
/// Returns [`IntakeError::Configuration`] when ortho-config fails to parse
/// arguments or load configuration files.
fn load_config() -> Result<FrankieConfig, IntakeError> {
    FrankieConfig::load().map_err(|error| IntakeError::Configuration {
        message: error.to_string(),
    })
}

fn write_pr_summary(details: &PullRequestDetails) -> Result<(), IntakeError> {
    let mut stdout = io::stdout().lock();
    let title = details
        .metadata
        .title
        .as_deref()
        .unwrap_or("untitled pull request");
    let author = details
        .metadata
        .author
        .as_deref()
        .unwrap_or("unknown author");
    let url = details
        .metadata
        .html_url
        .as_deref()
        .unwrap_or("no HTML URL provided");
    let message = format!(
        "Loaded PR #{} by {author}: {title}\nURL: {url}\nComments: {}",
        details.metadata.number,
        details.comments.len()
    );

    writeln!(stdout, "{message}").map_err(|error| IntakeError::Io {
        message: error.to_string(),
    })
}

fn write_listing_summary<W: Write>(
    writer: &mut W,
    result: &PaginatedPullRequests,
    owner: &str,
    repo: &str,
) -> Result<(), IntakeError> {
    let page_info = &result.page_info;

    writeln!(writer, "Pull requests for {owner}/{repo}:").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;

    for pr in &result.items {
        let title = pr.title.as_deref().unwrap_or("(no title)");
        let author = pr.author.as_deref().unwrap_or("unknown");
        let state = pr.state.as_deref().unwrap_or("unknown");
        writeln!(writer, "  #{} [{state}] {title} (@{author})", pr.number)
            .map_err(|e| io_error(&e))?;
    }

    writeln!(writer).map_err(|e| io_error(&e))?;
    writeln!(
        writer,
        "Page {} of {} ({} PRs shown)",
        page_info.current_page(),
        page_info.total_pages().unwrap_or(1),
        result.items.len()
    )
    .map_err(|e| io_error(&e))?;

    if page_info.has_next() {
        writeln!(writer, "More pages available.").map_err(|e| io_error(&e))?;
    }

    Ok(())
}

fn io_error(error: &io::Error) -> IntakeError {
    IntakeError::Io {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use frankie::github::PageInfo;
    use frankie::persistence::PersistenceError;
    use frankie::{PaginatedPullRequests, PullRequestSummary, RateLimitInfo, RepositoryLocator};
    use rstest::rstest;

    use super::{
        FrankieConfig, IntakeError, ListPullRequestsParams, PullRequestState, RepositoryGateway,
        run_repository_listing_with_gateway_builder, write_listing_summary,
    };

    #[test]
    fn persistence_error_classification_distinguishes_missing_from_blank() {
        assert!(
            !super::is_configuration_error(&PersistenceError::MissingDatabaseUrl),
            "MissingDatabaseUrl is handled before persistence runs"
        );
        assert!(
            super::is_configuration_error(&PersistenceError::BlankDatabaseUrl),
            "BlankDatabaseUrl is a configuration issue"
        );

        assert!(
            matches!(
                super::map_persistence_error(&PersistenceError::MissingDatabaseUrl),
                IntakeError::Io { .. }
            ),
            "MissingDatabaseUrl should not be treated as a persistence configuration error"
        );
        assert!(
            matches!(
                super::map_persistence_error(&PersistenceError::BlankDatabaseUrl),
                IntakeError::Configuration { .. }
            ),
            "BlankDatabaseUrl should map to IntakeError::Configuration"
        );
    }

    #[rstest]
    #[case::missing_database_url(None, "database URL is required")]
    #[case::blank_database_url(Some("   ".to_owned()), "database URL must not be blank")]
    fn migrate_db_rejects_invalid_database_url(
        #[case] database_url: Option<String>,
        #[case] expected_message_prefix: &str,
    ) {
        let config = FrankieConfig {
            database_url,
            migrate_db: true,
            ..Default::default()
        };

        let result = super::run_database_migrations(&config);

        match result {
            Err(IntakeError::Configuration { message }) => {
                assert!(
                    message.starts_with(expected_message_prefix),
                    "expected message starting with {expected_message_prefix:?}, got {message:?}"
                );
            }
            other => panic!("expected Configuration error, got {other:?}"),
        }
    }

    #[derive(Clone)]
    struct CapturingGateway {
        captured: Arc<Mutex<Option<(RepositoryLocator, ListPullRequestsParams)>>>,
        response: Arc<Mutex<Option<Result<PaginatedPullRequests, IntakeError>>>>,
    }

    #[async_trait]
    impl RepositoryGateway for CapturingGateway {
        async fn list_pull_requests(
            &self,
            locator: &RepositoryLocator,
            params: &ListPullRequestsParams,
        ) -> Result<PaginatedPullRequests, IntakeError> {
            self.captured
                .lock()
                .expect("captured mutex should be available")
                .replace((locator.clone(), params.clone()));

            self.response
                .lock()
                .expect("response mutex should be available")
                .take()
                .expect("response should only be consumed once")
        }
    }

    #[test]
    fn write_listing_summary_includes_items_and_pagination() {
        let page_info = PageInfo::builder(2, 50)
            .total_pages(Some(3))
            .has_next(true)
            .has_prev(true)
            .build();
        let result = frankie::PaginatedPullRequests {
            items: vec![PullRequestSummary {
                number: 42,
                title: Some("Add pagination".to_owned()),
                state: Some("open".to_owned()),
                author: Some("octocat".to_owned()),
                created_at: None,
                updated_at: None,
            }],
            page_info,
            rate_limit: Some(RateLimitInfo::new(5000, 4999, 1_700_000_000)),
        };

        let mut buffer = Vec::new();
        write_listing_summary(&mut buffer, &result, "octo", "repo")
            .expect("should write listing summary");

        let output = String::from_utf8(buffer).expect("output should be valid UTF-8");
        assert!(
            output.contains("Pull requests for octo/repo:"),
            "missing header: {output}"
        );
        assert!(
            output.contains("#42 [open] Add pagination (@octocat)"),
            "missing PR line: {output}"
        );
        assert!(
            output.contains("Page 2 of 3 (1 PRs shown)"),
            "missing page line: {output}"
        );
        assert!(
            output.contains("More pages available."),
            "missing next-page hint: {output}"
        );
    }

    #[tokio::test]
    async fn run_repository_listing_uses_expected_params_and_writes_output() {
        let config = FrankieConfig {
            token: Some("ghp_example".to_owned()),
            owner: Some("octo".to_owned()),
            repo: Some("repo".to_owned()),
            ..Default::default()
        };

        let captured = Arc::new(Mutex::new(None));
        let gateway = CapturingGateway {
            captured: Arc::clone(&captured),
            response: Arc::new(Mutex::new(Some(Ok(PaginatedPullRequests {
                items: vec![],
                page_info: PageInfo::default(),
                rate_limit: None,
            })))),
        };

        let mut buffer = Vec::new();
        run_repository_listing_with_gateway_builder(
            &config,
            move |token, locator| {
                assert_eq!(
                    token.value(),
                    "ghp_example",
                    "unexpected token passed to gateway builder"
                );
                assert_eq!(
                    locator.api_base().as_str(),
                    "https://api.github.com/",
                    "unexpected API base for github.com locator"
                );
                Ok(gateway)
            },
            &mut buffer,
        )
        .await
        .expect("repository listing should succeed");

        let (locator, params) = captured
            .lock()
            .expect("captured mutex should be available")
            .clone()
            .expect("gateway should have been called");
        assert_eq!(locator.owner().as_str(), "octo");
        assert_eq!(locator.repository().as_str(), "repo");
        assert_eq!(params.state, Some(PullRequestState::All));
        assert_eq!(params.per_page, Some(50));
        assert_eq!(params.page, Some(1));

        let output = String::from_utf8(buffer).expect("output should be valid UTF-8");
        assert!(
            output.contains("Pull requests for octo/repo:"),
            "missing header: {output}"
        );
    }

    #[tokio::test]
    async fn run_repository_listing_propagates_invalid_pagination_error() {
        let config = FrankieConfig {
            token: Some("ghp_example".to_owned()),
            owner: Some("octo".to_owned()),
            repo: Some("repo".to_owned()),
            ..Default::default()
        };

        let gateway = CapturingGateway {
            captured: Arc::new(Mutex::new(None)),
            response: Arc::new(Mutex::new(Some(Err(IntakeError::InvalidPagination {
                message: "page must be at least 1".to_owned(),
            })))),
        };

        let mut buffer = Vec::new();
        let result = run_repository_listing_with_gateway_builder(
            &config,
            |_token, _locator| Ok(gateway),
            &mut buffer,
        )
        .await;

        assert!(
            matches!(result, Err(IntakeError::InvalidPagination { .. })),
            "expected InvalidPagination, got {result:?}"
        );
    }

    #[test]
    fn write_listing_summary_defaults_total_pages_to_one_when_unknown() {
        let page_info = PageInfo::builder(1, 50).build();
        let result = frankie::PaginatedPullRequests {
            items: vec![],
            page_info,
            rate_limit: None,
        };

        let mut buffer = Vec::new();
        write_listing_summary(&mut buffer, &result, "octo", "repo")
            .expect("should write listing summary");

        let output = String::from_utf8(buffer).expect("output should be valid UTF-8");
        assert!(
            output.contains("Page 1 of 1 (0 PRs shown)"),
            "expected default total pages of 1, got: {output}"
        );
    }

    // RateLimitInfo unit tests live in `src/github/tests.rs`.

    mod discovery_error_handling {
        use frankie::IntakeError;
        use frankie::local::LocalDiscoveryError;

        use super::super::handle_discovery_error;

        /// Returns the expected error message for missing arguments.
        fn expected_missing_args_message() -> &'static str {
            "either --pr-url/-u or --owner/-o with --repo/-r is required"
        }

        #[test]
        fn not_a_repository_returns_missing_arguments_error() {
            let result = handle_discovery_error(LocalDiscoveryError::NotARepository);

            match result {
                Err(IntakeError::Configuration { message }) => {
                    assert!(
                        message.contains(expected_missing_args_message()),
                        "NotARepository should return missing arguments error, got: {message}"
                    );
                }
                other => panic!("expected Configuration error, got: {other:?}"),
            }
        }

        #[test]
        fn no_remotes_returns_missing_arguments_error() {
            let result = handle_discovery_error(LocalDiscoveryError::NoRemotes);

            match result {
                Err(IntakeError::Configuration { message }) => {
                    assert!(
                        message.contains(expected_missing_args_message()),
                        "NoRemotes should return missing arguments error, got: {message}"
                    );
                }
                other => panic!("expected Configuration error, got: {other:?}"),
            }
        }

        #[test]
        fn remote_not_found_returns_missing_arguments_error() {
            let result = handle_discovery_error(LocalDiscoveryError::RemoteNotFound {
                name: "upstream".to_owned(),
            });

            match result {
                Err(IntakeError::Configuration { message }) => {
                    assert!(
                        message.contains(expected_missing_args_message()),
                        "RemoteNotFound should return missing arguments error, got: {message}"
                    );
                }
                other => panic!("expected Configuration error, got: {other:?}"),
            }
        }

        #[test]
        fn invalid_remote_url_returns_missing_arguments_error() {
            let result = handle_discovery_error(LocalDiscoveryError::InvalidRemoteUrl {
                url: "not-a-url".to_owned(),
            });

            match result {
                Err(IntakeError::Configuration { message }) => {
                    assert!(
                        message.contains(expected_missing_args_message()),
                        "InvalidRemoteUrl should return missing arguments error, got: {message}"
                    );
                }
                other => panic!("expected Configuration error, got: {other:?}"),
            }
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

        use super::super::FrankieConfig;

        #[tokio::test]
        #[expect(
            clippy::excessive_nesting,
            reason = "nested test module structure"
        )]
        async fn no_local_discovery_returns_missing_arguments_error() {
            let config = FrankieConfig {
                no_local_discovery: true,
                ..Default::default()
            };

            let result = super::super::run_interactive(&config).await;
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
