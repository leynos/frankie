//! TUI mode for reviewing PR comments.
//!
//! This module provides the entry point for the interactive terminal user
//! interface that allows users to navigate and filter review comments.

use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use bubbletea_rs::Program;

use frankie::telemetry::StderrJsonlTelemetrySink;
use frankie::tui::{
    ReviewApp, set_initial_reviews, set_initial_terminal_size, set_refresh_context,
    set_telemetry_sink,
};
use frankie::{
    FrankieConfig, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken,
    PullRequestLocator, ReviewCommentGateway,
};

/// Runs the TUI mode for reviewing PR comments.
///
/// When a positional PR identifier is present, the locator is resolved via
/// [`PullRequestLocator::from_identifier`], using local git discovery for
/// bare PR numbers. Otherwise the existing `--pr-url` + `parse` flow is
/// used for backwards compatibility.
///
/// # Errors
///
/// Returns an error if:
/// - The PR URL or identifier is missing or invalid
/// - Local git discovery fails (bare PR number outside a repository)
/// - The token is missing or invalid
/// - The GitHub API call fails
/// - The TUI fails to initialise
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let locator = resolve_locator(config)?;
    let token = PersonalAccessToken::new(config.resolve_token()?)?;

    // Create gateway and fetch review comments
    let gateway = OctocrabReviewCommentGateway::new(&token, locator.api_base().as_str())?;
    let reviews = gateway.list_review_comments(&locator).await?;

    // Store reviews in global state for Model::init() to retrieve.
    // Returns false if already set (e.g. re-running TUI in same process).
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

    // Store refresh context for the refresh feature.
    // Returns false if already set; this is non-fatal since refresh will
    // simply use the existing context (which may reference a different PR).
    let _ = set_refresh_context(locator, token);

    // Configure telemetry for sync latency metrics.
    // Returns false if already set; this is non-fatal.
    let _ = set_telemetry_sink(Arc::new(StderrJsonlTelemetrySink));

    // Run the TUI program
    run_tui().await.map_err(|error| IntakeError::Api {
        message: format!("TUI error: {error}"),
    })?;

    Ok(())
}

/// Resolves a [`PullRequestLocator`] from the configuration.
///
/// Prefers the positional `pr_identifier` when available, falling back to
/// `--pr-url`. For bare PR numbers the local git repository is discovered
/// to obtain the owner and repository name.
fn resolve_locator(config: &FrankieConfig) -> Result<PullRequestLocator, IntakeError> {
    if let Some(identifier) = config.pr_identifier() {
        return resolve_from_identifier(identifier, config.no_local_discovery);
    }

    let pr_url = config.require_pr_url()?;
    PullRequestLocator::parse(pr_url)
}

/// Resolves a locator from a positional PR identifier (URL or bare number).
///
/// URL identifiers are forwarded directly to [`PullRequestLocator::parse`]
/// without local git discovery, avoiding unnecessary
/// `frankie::discover_repository` calls.
///
/// For bare PR numbers, local git discovery provides the owner/repo
/// context needed to construct a full URL. When `no_local_discovery` is
/// `true`, a bare number is rejected with a [`IntakeError::Configuration`]
/// error instructing the user to supply a full PR URL.
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
}
