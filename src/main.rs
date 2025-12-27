//! Frankie CLI entrypoint for pull request intake.

use std::io::{self, Write};
use std::process::ExitCode;

use frankie::{FrankieConfig, IntakeError, OperationMode};
use ortho_config::OrthoConfig;

mod cli;

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
        return cli::migrations::run(&config);
    }

    match config.operation_mode() {
        OperationMode::SinglePullRequest => cli::single_pr::run(&config).await,
        OperationMode::RepositoryListing => cli::repository_listing::run(&config).await,
        OperationMode::Interactive => cli::interactive::run(&config).await,
        OperationMode::ReviewTui => cli::review_tui::run(&config).await,
    }
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
