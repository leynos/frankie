//! Frankie CLI entrypoint for pull request intake.

use std::io::{self, Write};
use std::process::ExitCode;

use frankie::github::RepositoryGateway;
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
        OperationMode::Interactive => Err(IntakeError::Configuration {
            message: concat!(
                "either --pr-url/-u or --owner/-o with --repo/-r is required\n",
                "Run 'frankie --help' for usage information"
            )
            .to_owned(),
        }),
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

    let params = ListPullRequestsParams {
        state: Some(PullRequestState::All),
        per_page: Some(50),
        page: Some(1),
    };

    let result = intake.list_pull_requests(&locator, &params).await?;
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
            pr_url: None,
            token: Some("ghp_example".to_owned()),
            owner: Some("octo".to_owned()),
            repo: Some("repo".to_owned()),
            database_url: None,
            migrate_db: false,
            pr_metadata_cache_ttl_seconds: 86_400,
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
            pr_url: None,
            token: Some("ghp_example".to_owned()),
            owner: Some("octo".to_owned()),
            repo: Some("repo".to_owned()),
            database_url: None,
            migrate_db: false,
            pr_metadata_cache_ttl_seconds: 86_400,
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
}
