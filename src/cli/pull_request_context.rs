//! Shared pull-request context helpers for CLI operation modes.

use std::path::Path;

use frankie::github::PullRequestGateway;
use frankie::local::discover_repository;
use frankie::{
    FrankieConfig, IntakeError, OctocrabGateway, PersonalAccessToken, PullRequestLocator,
};

/// Resolves a [`PullRequestLocator`] from CLI configuration.
///
/// Positional identifiers take precedence over `--pr-url`. Bare pull-request
/// numbers use local repository discovery to determine the owner/repository.
///
/// # Errors
///
/// Returns an error when no pull-request locator is configured, the configured
/// URL is invalid, or local discovery is required but unavailable.
pub(super) fn resolve_locator(config: &FrankieConfig) -> Result<PullRequestLocator, IntakeError> {
    if let Some(identifier) = config.pr_identifier() {
        return resolve_from_identifier(
            identifier,
            config.no_local_discovery,
            config.repo_path.as_deref(),
        );
    }

    PullRequestLocator::parse(config.require_pr_url()?)
}

/// Fetches the pull-request title for prompt context when it is available.
///
/// # Errors
///
/// Returns an error when the metadata request fails.
pub(super) async fn fetch_pull_request_title(
    locator: &PullRequestLocator,
    token: &PersonalAccessToken,
) -> Result<Option<String>, IntakeError> {
    let gateway = OctocrabGateway::for_token(token, locator)?;
    let metadata = gateway.pull_request(locator).await?;
    Ok(metadata.title)
}

pub(super) fn resolve_from_identifier(
    identifier: &str,
    no_local_discovery: bool,
    repo_path: Option<&str>,
) -> Result<PullRequestLocator, IntakeError> {
    if identifier.contains("://") {
        return PullRequestLocator::parse(identifier);
    }

    let has_repo_path = repo_path.is_some_and(|path| !path.trim().is_empty());
    if no_local_discovery && !has_repo_path {
        return Err(IntakeError::Configuration {
            message: concat!(
                "bare PR numbers require local git discovery to determine ",
                "owner/repo, but --no-local-discovery is set; provide a ",
                "full PR URL instead"
            )
            .to_owned(),
        });
    }

    let discovery_path =
        repo_path.map_or_else(|| Path::new(".").to_path_buf(), std::path::PathBuf::from);
    let local_repo = discover_repository(&discovery_path).map_err(|error| {
        let message = if repo_path.is_some() {
            format!(
                "failed to discover local repository at {}: {error}",
                discovery_path.display()
            )
        } else {
            format!("failed to discover local repository: {error}")
        };
        IntakeError::LocalDiscovery { message }
    })?;
    PullRequestLocator::from_identifier(identifier, local_repo.github_origin())
}
