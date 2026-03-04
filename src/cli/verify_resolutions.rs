//! CLI operation mode for automated resolution verification.
//!
//! This mode loads pull request review comments from GitHub and verifies each
//! comment against the local repository `HEAD` by replaying diffs and checking
//! deterministic conditions. Verification results are persisted in the local
//! `SQLite` cache for reuse across sessions.

use std::io::{self, Write};
use std::path::Path;

use frankie::local::{GitHubOrigin, create_git_ops, discover_repository};
use frankie::persistence::ReviewCommentVerificationCache;
use frankie::time::unix_now;
use frankie::verification::{DiffReplayResolutionVerifier, ResolutionVerificationService};
use frankie::{
    FrankieConfig, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken,
    PullRequestLocator, ReviewComment, ReviewCommentGateway,
};

/// Verifies review comments for a pull request and persists results.
///
/// # Errors
///
/// Returns an error if configuration is missing, the GitHub API call fails,
/// local repository discovery fails, or cache persistence fails.
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let locator = resolve_locator(config)?;
    let token = PersonalAccessToken::new(config.resolve_token()?)?;

    let database_url =
        config
            .database_url
            .as_deref()
            .ok_or_else(|| IntakeError::Configuration {
                message: "database URL is required for verification (use --database-url)"
                    .to_owned(),
            })?;
    let cache = ReviewCommentVerificationCache::new(database_url.to_owned()).map_err(|error| {
        IntakeError::Configuration {
            message: error.to_string(),
        }
    })?;

    let (repo_path, head_sha) = discover_repo_for_locator(config, &locator).map_err(|message| {
        IntakeError::Configuration {
            message: format!("failed to discover local repository: {message}"),
        }
    })?;
    let git_ops = create_git_ops(&repo_path).map_err(|error| IntakeError::Configuration {
        message: format!(
            "failed to open git repository at {}: {error}",
            repo_path.display()
        ),
    })?;

    let gateway = OctocrabReviewCommentGateway::new(&token, locator.api_base().as_str())?;
    let reviews = gateway.list_review_comments(&locator).await?;

    let verifier = DiffReplayResolutionVerifier::new(git_ops);
    let results = verifier.verify_comments(&reviews, &head_sha);

    let now_unix = unix_now();
    cache
        .upsert_all(&results, now_unix)
        .map_err(|error| IntakeError::Api {
            message: format!("failed to persist verification result: {error}"),
        })?;

    write_summary(&results, &reviews)?;
    Ok(())
}

fn comment_location(comment: &ReviewComment) -> String {
    let file = comment.file_path.as_deref().unwrap_or("(no file)");
    let line_suffix = comment
        .line_number
        .or(comment.original_line_number)
        .map_or_else(String::new, |line| format!(":{line}"));
    format!("{file}{line_suffix}")
}

fn write_summary(
    results: &[frankie::verification::CommentVerificationResult],
    reviews: &[ReviewComment],
) -> Result<(), IntakeError> {
    let verified_count = results
        .iter()
        .filter(|result| {
            result.status() == frankie::verification::CommentVerificationStatus::Verified
        })
        .count();
    let unverified_count = results.len().saturating_sub(verified_count);

    let mut writer = io::stdout().lock();
    writeln!(
        writer,
        "Verification: {verified_count} verified, {unverified_count} unverified"
    )
    .map_err(|e| IntakeError::Io {
        message: format!("failed to write output: {e}"),
    })?;

    let review_by_id: std::collections::HashMap<u64, &ReviewComment> =
        reviews.iter().map(|review| (review.id, review)).collect();

    for result in results {
        let location = review_by_id.get(&result.github_comment_id()).map_or_else(
            || "(unknown location)".to_owned(),
            |review| comment_location(review),
        );
        let evidence_suffix = result
            .evidence()
            .message
            .as_deref()
            .map_or_else(String::new, |message| format!(" - {message}"));
        writeln!(
            writer,
            "{} {} comment {} ({location}) {}{evidence_suffix}",
            result.status().symbol(),
            result.status(),
            result.github_comment_id(),
            result.evidence().kind
        )
        .map_err(|e| IntakeError::Io {
            message: format!("failed to write output: {e}"),
        })?;
    }
    Ok(())
}

/// Resolves a [`PullRequestLocator`] from the configuration, preferring the
/// positional `pr_identifier` and falling back to `--pr-url`.
fn resolve_locator(config: &FrankieConfig) -> Result<PullRequestLocator, IntakeError> {
    if let Some(identifier) = config.pr_identifier() {
        return resolve_from_identifier(
            identifier,
            config.no_local_discovery,
            config.repo_path.as_deref(),
        );
    }

    let pr_url = config.require_pr_url()?;
    PullRequestLocator::parse(pr_url)
}

fn resolve_from_identifier(
    identifier: &str,
    no_local_discovery: bool,
    repo_path: Option<&str>,
) -> Result<PullRequestLocator, IntakeError> {
    if identifier.contains("://") {
        return PullRequestLocator::parse(identifier);
    }

    if no_local_discovery {
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
        IntakeError::Api { message }
    })?;
    PullRequestLocator::from_identifier(identifier, local_repo.github_origin())
}

/// Discovers a local repository matching the PR's origin.
///
/// Returns the repository path and HEAD SHA on success.
fn discover_repo_for_locator(
    config: &FrankieConfig,
    locator: &PullRequestLocator,
) -> Result<(std::path::PathBuf, String), String> {
    let discovery_path = choose_repo_discovery_path(config)?;
    let local_repo = discover_repository(&discovery_path).map_err(|e| {
        if config.repo_path.is_some() {
            format!("--repo-path '{}': {e}", discovery_path.display())
        } else {
            format!("{e}")
        }
    })?;

    validate_repo_matches_locator(local_repo.github_origin(), locator)?;

    let head_sha = local_repo.head_sha()?;
    Ok((local_repo.workdir().to_path_buf(), head_sha))
}

fn choose_repo_discovery_path(config: &FrankieConfig) -> Result<std::path::PathBuf, String> {
    if let Some(ref repo_path) = config.repo_path {
        return Ok(std::path::PathBuf::from(repo_path));
    }

    if config.no_local_discovery {
        return Err("local repository discovery is disabled (--no-local-discovery)".to_owned());
    }

    Ok(std::path::PathBuf::from("."))
}

fn validate_repo_matches_locator(
    origin: &GitHubOrigin,
    locator: &PullRequestLocator,
) -> Result<(), String> {
    let expected_host = locator.host();
    let expected_owner = locator.owner().as_str();
    let expected_repo = locator.repository().as_str();

    if !origin.host().eq_ignore_ascii_case(expected_host) {
        return Err(format!(
            concat!(
                "local repository host ({found_host}) does not match the PR ",
                "host ({expected_host})"
            ),
            found_host = origin.host(),
            expected_host = expected_host,
        ));
    }

    if !origin.owner().eq_ignore_ascii_case(expected_owner)
        || !origin.repository().eq_ignore_ascii_case(expected_repo)
    {
        return Err(format!(
            concat!(
                "local repository owner/repo ({found_owner}/{found_repo}) does ",
                "not match the PR ({expected_owner}/{expected_repo})"
            ),
            found_owner = origin.owner(),
            found_repo = origin.repository(),
            expected_owner = expected_owner,
            expected_repo = expected_repo,
        ));
    }

    Ok(())
}
