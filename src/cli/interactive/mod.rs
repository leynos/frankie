//! Interactive mode with local repository discovery.

use std::io::{self, Write};
use std::path::Path;

use frankie::github::RepositoryGateway;
use frankie::local::{GitHubOrigin, LocalDiscoveryError, discover_repository};
use frankie::{
    FrankieConfig, IntakeError, OctocrabRepositoryGateway, PersonalAccessToken, RepositoryIntake,
    RepositoryLocator,
};

use super::default_listing_params;
use super::output::write_listing_summary;

#[cfg(test)]
mod tests;

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
        Ok(local_repo) => {
            let mut stdout = io::stdout().lock();
            run_discovered_repository_with_gateway_builder(
                config,
                local_repo.github_origin(),
                OctocrabRepositoryGateway::for_token,
                &mut stdout,
            )
            .await
        }
        Err(error) => handle_discovery_error(error),
    }
}

/// Runs repository listing using a discovered local repository.
///
/// This function is exposed for testing with mock gateways.
pub async fn run_discovered_repository_with_gateway_builder<G, F, W>(
    config: &FrankieConfig,
    github_origin: &GitHubOrigin,
    build_gateway: F,
    writer: &mut W,
) -> Result<(), IntakeError>
where
    G: RepositoryGateway,
    F: FnOnce(&PersonalAccessToken, &RepositoryLocator) -> Result<G, IntakeError>,
    W: Write,
{
    let owner = github_origin.owner();
    let repo = github_origin.repository();

    // Log the discovery to stderr (ignore write errors)
    drop(writeln!(
        io::stderr(),
        "Discovered repository from local Git: {owner}/{repo}"
    ));

    let token_value = config.resolve_token()?;
    let locator = RepositoryLocator::from_github_origin(github_origin)?;
    let token = PersonalAccessToken::new(token_value)?;

    let gateway = build_gateway(&token, &locator)?;
    let intake = RepositoryIntake::new(&gateway);

    let result = intake
        .list_pull_requests(&locator, &default_listing_params())
        .await?;
    write_listing_summary(writer, &result, owner, repo)
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
