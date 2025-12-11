//! Frankie CLI entrypoint for pull request intake.

use std::io::{self, Write};
use std::process::ExitCode;

use frankie::{
    FrankieConfig, IntakeError, ListPullRequestsParams, OctocrabGateway, OctocrabRepositoryGateway,
    OperationMode, PaginatedPullRequests, PersonalAccessToken, PullRequestDetails,
    PullRequestIntake, PullRequestLocator, PullRequestState, RepositoryIntake, RepositoryLocator,
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

/// Loads a single pull request by URL.
async fn run_single_pr(config: &FrankieConfig) -> Result<(), IntakeError> {
    let pr_url = config.require_pr_url()?;
    let token_value = config.resolve_token()?;

    let locator = PullRequestLocator::parse(pr_url)?;
    let token = PersonalAccessToken::new(token_value)?;

    let gateway = OctocrabGateway::for_token(&token, &locator)?;
    let intake = PullRequestIntake::new(&gateway);
    let details = intake.load(&locator).await?;

    write_pr_summary(&details)
}

/// Lists pull requests for a repository.
async fn run_repository_listing(config: &FrankieConfig) -> Result<(), IntakeError> {
    let (owner, repo) = config.require_repository_info()?;
    let token_value = config.resolve_token()?;

    let locator = RepositoryLocator::from_owner_repo(owner, repo)?;
    let token = PersonalAccessToken::new(token_value)?;

    let gateway = OctocrabRepositoryGateway::for_token(&token, &locator)?;
    let intake = RepositoryIntake::new(&gateway);

    let params = ListPullRequestsParams {
        state: Some(PullRequestState::All),
        per_page: Some(50),
        page: Some(1),
    };

    let result = intake.list_pull_requests(&locator, &params).await?;
    write_listing_summary(&result, owner, repo)
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

fn write_listing_summary(
    result: &PaginatedPullRequests,
    owner: &str,
    repo: &str,
) -> Result<(), IntakeError> {
    let mut stdout = io::stdout().lock();
    let page_info = &result.page_info;

    writeln!(stdout, "Pull requests for {owner}/{repo}:").map_err(|e| io_error(&e))?;
    writeln!(stdout).map_err(|e| io_error(&e))?;

    for pr in &result.items {
        let title = pr.title.as_deref().unwrap_or("(no title)");
        let author = pr.author.as_deref().unwrap_or("unknown");
        let state = pr.state.as_deref().unwrap_or("unknown");
        writeln!(stdout, "  #{} [{state}] {title} (@{author})", pr.number)
            .map_err(|e| io_error(&e))?;
    }

    writeln!(stdout).map_err(|e| io_error(&e))?;
    writeln!(
        stdout,
        "Page {} of {} ({} PRs shown)",
        page_info.current_page(),
        page_info.total_pages().unwrap_or(1),
        result.items.len()
    )
    .map_err(|e| io_error(&e))?;

    if page_info.has_next() {
        writeln!(stdout, "More pages available.").map_err(|e| io_error(&e))?;
    }

    Ok(())
}

fn io_error(error: &io::Error) -> IntakeError {
    IntakeError::Io {
        message: error.to_string(),
    }
}
