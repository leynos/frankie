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
        OperationMode::AiRewrite => cli::ai_rewrite::run(&config),
    }
}

/// Extracts a positional PR identifier from the raw argument list.
///
/// Walks the arguments after argv\[0\] looking for the first value that is
/// neither a flag nor the value consumed by a preceding flag. When found it
/// is removed from the returned argument vector so that ortho-config never
/// sees it (ortho-config does not support positional arguments).
///
/// A bare `--` terminates option parsing: everything after it is treated as
/// positional. The `--` marker itself is consumed and not forwarded.
///
/// Returns `(identifier, filtered_args)` where `identifier` is `None` when
/// no positional argument was found.
fn extract_positional_pr_identifier(args: Vec<OsString>) -> (Option<String>, Vec<OsString>) {
    let mut iter = args.into_iter();
    let mut state = ArgParserState::new(iter.size_hint().0);

    // Always keep argv[0]
    if let Some(program) = iter.next() {
        state.filtered.push(program);
    }

    for arg in iter {
        match state.phase {
            ParsePhase::Normal => state.handle_normal_arg(arg),
            ParsePhase::SkipNext => state.handle_flag_value(arg),
            ParsePhase::AfterSeparator => state.handle_post_separator_arg(arg),
        }
    }

    (state.identifier, state.filtered)
}

/// State machine for argument parsing.
enum ParsePhase {
    Normal,
    SkipNext,
    AfterSeparator,
}

struct ArgParserState {
    filtered: Vec<OsString>,
    identifier: Option<String>,
    phase: ParsePhase,
}

impl ArgParserState {
    fn new(capacity: usize) -> Self {
        Self {
            filtered: Vec::with_capacity(capacity),
            identifier: None,
            phase: ParsePhase::Normal,
        }
    }

    /// Handles arguments after `--` — the first becomes the identifier,
    /// the rest are passed through.
    fn handle_post_separator_arg(&mut self, arg: OsString) {
        if self.identifier.is_none() {
            self.identifier = Some(arg.to_string_lossy().into_owned());
        } else {
            self.filtered.push(arg);
        }
    }

    /// Handles the value argument consumed by a preceding flag.
    fn handle_flag_value(&mut self, arg: OsString) {
        self.filtered.push(arg);
        self.phase = ParsePhase::Normal;
    }

    /// Handles an argument during normal parsing (not after `--`, not a
    /// flag value).
    fn handle_normal_arg(&mut self, arg: OsString) {
        let arg_string = arg.to_string_lossy().into_owned();

        if is_separator(&arg_string) {
            self.phase = ParsePhase::AfterSeparator;
            return;
        }

        if is_flag_token(&arg_string) {
            self.handle_flag(arg, &arg_string);
            return;
        }

        self.handle_positional(arg, arg_string);
    }

    /// Handles flag arguments, checking whether the flag consumes the
    /// next argument as its value.
    fn handle_flag(&mut self, arg: OsString, arg_str: &str) {
        if is_flag_requiring_value(arg_str) {
            self.phase = ParsePhase::SkipNext;
        }
        self.filtered.push(arg);
    }

    /// Handles positional arguments — the first becomes the identifier,
    /// subsequent ones are passed through and let ortho-config report
    /// the error.
    fn handle_positional(&mut self, arg: OsString, arg_string: String) {
        if self.identifier.is_none() {
            self.identifier = Some(arg_string);
        } else {
            self.filtered.push(arg);
        }
    }
}

/// Returns `true` when the argument is the `--` option-parsing terminator.
fn is_separator(arg: &str) -> bool {
    arg == "--"
}

/// Returns `true` when the argument looks like a flag (starts with `-`).
fn is_flag_token(arg: &str) -> bool {
    arg.starts_with('-')
}

/// Checks whether a flag requires a following value argument.
///
/// `--flag=value` is self-contained; `--flag value` needs a skip.
fn is_flag_requiring_value(flag: &str) -> bool {
    !flag.contains('=') && FrankieConfig::VALUE_FLAGS.contains(&flag)
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
