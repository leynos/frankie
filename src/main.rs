//! Frankie CLI entrypoint for pull request intake.

use std::io::{self, Write};
use std::process::ExitCode;

use frankie::{
    FrankieConfig, IntakeError, OctocrabGateway, PersonalAccessToken, PullRequestDetails,
    PullRequestIntake, PullRequestLocator,
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

    let pr_url = config.require_pr_url()?;
    let token_value = config.resolve_token()?;

    let locator = PullRequestLocator::parse(pr_url)?;
    let token = PersonalAccessToken::new(token_value)?;

    let gateway = OctocrabGateway::for_token(&token, &locator)?;
    let intake = PullRequestIntake::new(&gateway);
    let details = intake.load(&locator).await?;

    write_summary(&details)?;
    Ok(())
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

fn write_summary(details: &PullRequestDetails) -> Result<(), IntakeError> {
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
