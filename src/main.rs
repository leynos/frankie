//! Frankie CLI entrypoint for pull request intake.

use std::ffi::OsString;
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
        OperationMode::ExportComments => cli::export_comments::run(&config).await,
    }
}

/// CLI flags that consume the following argument as their value.
///
/// Boolean flags (`--tui`, `--migrate-db`, `--no-local-discovery`) are omitted
/// because they do not consume a trailing value.
const VALUE_FLAGS: &[&str] = &[
    "--pr-url",
    "-u",
    "--token",
    "-t",
    "--owner",
    "-o",
    "--repo",
    "-r",
    "--database-url",
    "--pr-metadata-cache-ttl-seconds",
    "--export",
    "-e",
    "--output",
    "--template",
    "--config-path",
];

/// Extracts a positional PR identifier from the raw argument list.
///
/// Walks the arguments after argv\[0\] looking for the first value that is
/// neither a flag nor the value consumed by a preceding flag. When found it
/// is removed from the returned argument vector so that ortho-config never
/// sees it (ortho-config does not support positional arguments).
///
/// Returns `(identifier, filtered_args)` where `identifier` is `None` when
/// no positional argument was found.
fn extract_positional_pr_identifier(args: Vec<OsString>) -> (Option<String>, Vec<OsString>) {
    // argv[0] is always kept
    let mut filtered = Vec::with_capacity(args.len());
    let mut identifier: Option<String> = None;
    let mut skip_next = false;
    let mut first = true;

    for arg in args {
        // Always keep argv[0]
        if first {
            filtered.push(arg);
            first = false;
            continue;
        }

        if skip_next {
            // This arg is the value of a preceding flag — keep it
            filtered.push(arg);
            skip_next = false;
            continue;
        }

        let arg_str = arg.to_string_lossy();

        // Flags starting with - are passed through to ortho-config
        if arg_str.starts_with('-') {
            // Check if this flag consumes the next argument.
            // `--flag=value` is self-contained; `--flag value` needs a skip.
            let needs_skip =
                !arg_str.contains('=') && VALUE_FLAGS.iter().any(|f| *f == arg_str.as_ref());
            skip_next = needs_skip;
            filtered.push(arg);
            continue;
        }

        // First non-flag, non-consumed argument is the positional identifier
        if identifier.is_none() {
            identifier = Some(arg_str.into_owned());
        } else {
            // Subsequent positional args are unexpected — pass them through
            // and let ortho-config report the error.
            filtered.push(arg);
        }
    }

    (identifier, filtered)
}

/// Loads configuration from CLI, environment, and files.
///
/// Extracts any positional PR identifier from the raw arguments before
/// delegating to ortho-config, then validates the resulting configuration.
///
/// # Errors
///
/// Returns [`IntakeError::Configuration`] when ortho-config fails to parse
/// arguments, configuration files cannot be loaded, or the configuration
/// is internally inconsistent (e.g. both positional identifier and
/// `--pr-url` are provided).
fn load_config() -> Result<FrankieConfig, IntakeError> {
    let raw_args: Vec<OsString> = std::env::args_os().collect();
    let (identifier, filtered_args) = extract_positional_pr_identifier(raw_args);

    let mut config = FrankieConfig::load_from_iter(filtered_args).map_err(|error| {
        IntakeError::Configuration {
            message: error.to_string(),
        }
    })?;

    if let Some(value) = identifier {
        config.set_pr_identifier(value);
    }

    config.validate()?;

    Ok(config)
}

#[cfg(test)]
mod tests;
