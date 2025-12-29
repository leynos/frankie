//! TUI mode for reviewing PR comments.
//!
//! This module provides the entry point for the interactive terminal user
//! interface that allows users to navigate and filter review comments.

use std::io::{self, Write};
use std::sync::Arc;

use bubbletea_rs::Program;

use frankie::telemetry::StderrJsonlTelemetrySink;
use frankie::tui::{ReviewApp, set_initial_reviews, set_refresh_context, set_telemetry_sink};
use frankie::{
    FrankieConfig, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken,
    PullRequestLocator, ReviewCommentGateway,
};

/// Runs the TUI mode for reviewing PR comments.
///
/// # Errors
///
/// Returns an error if:
/// - The PR URL is missing or invalid
/// - The token is missing or invalid
/// - The GitHub API call fails
/// - The TUI fails to initialise
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let pr_url = config.require_pr_url()?;
    let locator = PullRequestLocator::parse(pr_url)?;
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

/// Runs the bubbletea-rs program with the `ReviewApp` model.
async fn run_tui() -> Result<(), bubbletea_rs::Error> {
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
}
