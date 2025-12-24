//! Single pull request loading operation.

use frankie::{
    FrankieConfig, IntakeError, OctocrabCachingGateway, OctocrabGateway, PersonalAccessToken,
    PullRequestIntake, PullRequestLocator,
};

use super::output::write_pr_summary;

/// Loads a single pull request by URL.
///
/// # Errors
///
/// Returns [`IntakeError::Configuration`] if required configuration is missing.
/// Returns [`IntakeError::GitHub`] if the API request fails.
pub async fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
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
