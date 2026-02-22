//! TUI mode for reviewing PR comments.
//!
//! This module provides the entry point for the interactive terminal user
//! interface that allows users to navigate and filter review comments.

use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use bubbletea_rs::Program;

use frankie::local::{GitHubOrigin, create_git_ops, discover_repository};
use frankie::telemetry::StderrJsonlTelemetrySink;
use frankie::tui::{
    ReplyDraftConfig, ReplyDraftMaxLength, ReviewApp, TimeTravelContext, set_git_ops_context,
    set_initial_reviews, set_initial_terminal_size, set_refresh_context, set_reply_draft_config,
    set_telemetry_sink, set_time_travel_context,
};
use frankie::{
    FrankieConfig, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken,
    PullRequestLocator, ReviewCommentGateway,
};

/// Runs the TUI mode for reviewing PR comments.
///
/// Resolves the PR locator, fetches reviews from GitHub, wires up
/// time-travel when a local repository is available, and launches the
/// interactive TUI.
///
/// # Errors
///
/// Returns an error if locator resolution, token validation, the GitHub
/// API call, or TUI initialisation fails.
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let locator = resolve_locator(config)?;
    let token = PersonalAccessToken::new(config.resolve_token()?)?;

    // Create gateway and fetch review comments
    let gateway = OctocrabReviewCommentGateway::new(&token, locator.api_base().as_str())?;
    let reviews = gateway.list_review_comments(&locator).await?;

    let review_count = reviews.len();
    if !set_initial_reviews(reviews) {
        return Err(IntakeError::Api {
            message: format!(
                "TUI already initialised with reviews from a previous run. \
                 Cannot proceed with {review_count} newly fetched review(s) as stale data may be displayed. \
                 Restart the process to view fresh data."
            ),
        });
    }

    // Non-fatal: TUI launches without time-travel on failure.
    let discovery_failure = try_setup_git_ops(config, &locator);
    let _ = set_time_travel_context(TimeTravelContext {
        host: locator.host().to_owned(),
        owner: locator.owner().as_str().to_owned(),
        repo: locator.repository().as_str().to_owned(),
        pr_number: locator.number().get(),
        discovery_failure,
    });

    let _ = set_refresh_context(locator, token);
    let reply_draft_config = ReplyDraftConfig::new(
        ReplyDraftMaxLength::new(config.reply_max_length),
        config.reply_templates.clone(),
    );
    let _ = set_reply_draft_config(reply_draft_config);
    let _ = set_telemetry_sink(Arc::new(StderrJsonlTelemetrySink));
    run_tui().await.map_err(|error| IntakeError::Api {
        message: format!("TUI error: {error}"),
    })?;

    Ok(())
}

/// Resolves a [`PullRequestLocator`] from the configuration, preferring
/// the positional `pr_identifier` and falling back to `--pr-url`.
fn resolve_locator(config: &FrankieConfig) -> Result<PullRequestLocator, IntakeError> {
    if let Some(identifier) = config.pr_identifier() {
        return resolve_from_identifier(identifier, config.no_local_discovery);
    }

    let pr_url = config.require_pr_url()?;
    PullRequestLocator::parse(pr_url)
}

/// Resolves a locator from a positional PR identifier (URL or bare number).
///
/// URL identifiers bypass local discovery. Bare PR numbers require
/// discovery for owner/repo context; when `no_local_discovery` is `true`
/// a bare number is rejected with a configuration error.
fn resolve_from_identifier(
    identifier: &str,
    no_local_discovery: bool,
) -> Result<PullRequestLocator, IntakeError> {
    // URL identifiers skip local discovery entirely;
    // PullRequestLocator::from_identifier also handles URLs (see
    // locator.rs line ~215) but this early return avoids the unnecessary
    // discover_repository call below.
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

    // Bare PR number — discover owner/repo from local git remote
    let local_repo = frankie::discover_repository(Path::new(".")).map_err(|error| {
        IntakeError::LocalDiscovery {
            message: format!("{error}. Provide a full PR URL instead"),
        }
    })?;

    PullRequestLocator::from_identifier(identifier, local_repo.github_origin())
}

/// Attempts to set up Git operations for time-travel navigation.
///
/// Tries to discover or open a local repository matching the PR, then
/// creates git ops and stores them in global state for `Model::init()`.
///
/// Returns `None` on success, or a failure reason string when discovery
/// fails. Failures are non-fatal: the TUI launches without time-travel.
fn try_setup_git_ops(config: &FrankieConfig, locator: &PullRequestLocator) -> Option<String> {
    let result = discover_repo_for_locator(config, locator);

    match result {
        Ok((repo_path, head_sha)) => match create_git_ops(&repo_path) {
            Ok(git_ops) => {
                let _ = set_git_ops_context(git_ops, head_sha);
                None
            }
            Err(e) => Some(format!(
                "failed to open repository at {}: {e}",
                repo_path.display()
            )),
        },
        Err(reason) => Some(reason),
    }
}

/// Discovers a local repository matching the PR's origin.
///
/// Uses `--repo-path` if configured, otherwise auto-discovers from the
/// current directory. Validates that the discovered repository's origin
/// matches the PR's owner and repository.
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

    // Validate the discovered repository matches the PR's origin
    validate_repo_matches_locator(local_repo.github_origin(), locator)?;

    // Get HEAD SHA for line mapping verification
    let head_sha = local_repo.head_sha()?;

    Ok((local_repo.workdir().to_path_buf(), head_sha))
}

/// Chooses the path to use for local repository discovery.
///
/// Returns the explicit `--repo-path` when provided, rejects discovery
/// when `--no-local-discovery` is set, and falls back to the current
/// directory.
fn choose_repo_discovery_path(config: &FrankieConfig) -> Result<std::path::PathBuf, String> {
    if let Some(ref repo_path) = config.repo_path {
        return Ok(std::path::PathBuf::from(repo_path));
    }

    if config.no_local_discovery {
        return Err("local repository discovery is disabled (--no-local-discovery)".to_owned());
    }

    Ok(std::path::PathBuf::from("."))
}

/// Validates that a discovered repository's origin matches the PR's
/// host, owner, and repository.
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
                "local repository origin ({found_owner}/{found_repo}) does not ",
                "match the PR repository ({expected_owner}/{expected_repo})"
            ),
            found_owner = origin.owner(),
            found_repo = origin.repository(),
            expected_owner = expected_owner,
            expected_repo = expected_repo,
        ));
    }

    Ok(())
}

/// Runs the bubbletea-rs program with the `ReviewApp` model.
async fn run_tui() -> Result<(), bubbletea_rs::Error> {
    // Seed initial terminal dimensions so first render uses the actual size.
    if let Ok((width, height)) = crossterm::terminal::size() {
        let _ = set_initial_terminal_size(width, height);
    }

    // Build and run the program using the builder pattern.
    // ReviewApp::init() will retrieve data from module-level storage.
    let program = Program::<ReviewApp>::builder().alt_screen(true).build()?;

    program.run().await?;

    // Ensure stdout is flushed
    io::stdout().flush().ok();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_app_can_be_created_empty() {
        let app = ReviewApp::empty();
        assert_eq!(app.filtered_count(), 0);
    }

    #[test]
    fn bare_number_rejected_when_local_discovery_disabled() {
        let result = resolve_from_identifier("42", true);

        assert!(
            matches!(result, Err(IntakeError::Configuration { .. })),
            "bare number with no_local_discovery should fail, got {result:?}"
        );
    }

    #[test]
    fn url_identifier_allowed_when_local_discovery_disabled() {
        let result = resolve_from_identifier("https://github.com/octo/repo/pull/42", true);

        assert!(
            result.is_ok(),
            "URL identifier should succeed even with no_local_discovery, got {result:?}"
        );
    }

    /// Verifies that `StderrJsonlTelemetrySink` implements `TelemetrySink`
    /// and can be used with `set_telemetry_sink`, demonstrating the CLI
    /// telemetry wiring pattern used in the `run` function.
    ///
    /// This test covers the CLI side of telemetry wiring. For the full
    /// end-to-end integration test demonstrating events flowing from TUI
    /// sync handlers through to the telemetry sink, see the BDD scenario
    /// "Sync latency is logged to telemetry" in `tests/review_sync_bdd.rs`.
    #[test]
    fn cli_telemetry_wiring_pattern_is_valid() {
        use frankie::telemetry::TelemetrySink;

        // Create the sink exactly as done in run() at line 56
        let sink: Arc<dyn TelemetrySink> = Arc::new(StderrJsonlTelemetrySink);

        // Verify it implements TelemetrySink and can record events without panic
        sink.record(frankie::telemetry::TelemetryEvent::SyncLatencyRecorded {
            latency_ms: 42,
            comment_count: 5,
            incremental: true,
        });

        // Wire it to the TUI module (same call as in run())
        // The call may fail due to OnceLock if already set by another test,
        // but we verify the wiring pattern compiles and the sink is usable.
        let _ = set_telemetry_sink(sink);
    }

    #[rstest::rstest]
    #[case::matching_repo(
        "https://github.com/octocat/hello-world/pull/42",
        "octocat",
        "hello-world",
        true
    )]
    #[case::case_insensitive_matching(
        "https://github.com/OctoCat/Hello-World/pull/1",
        "octocat",
        "hello-world",
        true
    )]
    #[case::mismatched_repo(
        "https://github.com/octocat/hello-world/pull/1",
        "octocat",
        "other-repo",
        false
    )]
    #[case::mismatched_owner(
        "https://github.com/alice/hello-world/pull/1",
        "bob",
        "hello-world",
        false
    )]
    fn validate_repo_matches_locator_cases(
        #[case] locator_url: &str,
        #[case] origin_owner: &str,
        #[case] origin_repo: &str,
        #[case] should_succeed: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let locator = PullRequestLocator::parse(locator_url)?;
        let origin = GitHubOrigin::GitHubCom {
            owner: origin_owner.to_owned(),
            repository: origin_repo.to_owned(),
        };

        let result = validate_repo_matches_locator(&origin, &locator);
        if result.is_ok() != should_succeed {
            return Err(format!("expected is_ok={should_succeed}, got {result:?}").into());
        }

        Ok(())
    }

    #[test]
    fn validate_repo_rejects_mismatched_enterprise_host() -> Result<(), Box<dyn std::error::Error>>
    {
        let locator = PullRequestLocator::parse("https://ghe.corp.com/octocat/hello-world/pull/1")?;

        let origin = GitHubOrigin::Enterprise {
            host: "ghe.other.com".to_owned(),
            port: None,
            owner: "octocat".to_owned(),
            repository: "hello-world".to_owned(),
        };

        let result = validate_repo_matches_locator(&origin, &locator);
        let err = result
            .err()
            .ok_or("expected Err for mismatched enterprise host")?;
        if !err.contains("ghe.other.com") {
            return Err(format!("error should mention local host: {err}").into());
        }
        if !err.contains("ghe.corp.com") {
            return Err(format!("error should mention PR host: {err}").into());
        }

        Ok(())
    }
}
