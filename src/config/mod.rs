//! Application configuration loaded from CLI, environment, and files.
//!
//! This module provides a unified configuration struct that merges values
//! from command-line arguments, environment variables, and configuration
//! files using ortho-config's layered approach.
//!
//! # Precedence
//!
//! Configuration values are loaded with the following precedence (lowest to
//! highest):
//!
//! 1. **Defaults** – Built-in application defaults
//! 2. **Configuration file** – `.frankie.toml` in current directory, home
//!    directory, or XDG config directory
//! 3. **Environment variables** – `FRANKIE_PR_URL`, `FRANKIE_TOKEN`, or legacy
//!    `GITHUB_TOKEN`
//! 4. **Command-line arguments** – `--pr-url`/`-u` and `--token`/`-t`
//!
//! # Configuration File
//!
//! Place `.frankie.toml` in the current directory, home directory, or
//! XDG config directory with:
//!
//! ```toml
//! pr_url = "https://github.com/owner/repo/pull/123"
//! token = "ghp_example"
//! owner = "octocat"
//! repo = "hello-world"
//! database_url = "frankie.sqlite"
//! migrate_db = true
//! ```

use std::env;

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

use crate::github::error::IntakeError;

/// Operation mode determined by CLI arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    /// Load a single PR by URL.
    SinglePullRequest,
    /// List PRs for a repository.
    RepositoryListing,
    /// Interactive repository discovery (future).
    Interactive,
    /// Interactive TUI for reviewing PR comments.
    ReviewTui,
}

/// Application configuration supporting CLI, environment, and file sources.
///
/// # Environment Variables
///
/// - `FRANKIE_PR_URL` or `--pr-url`: Pull request URL
/// - `FRANKIE_TOKEN`, `GITHUB_TOKEN`, or `--token`: Authentication token
/// - `FRANKIE_OWNER` or `--owner`: Repository owner
/// - `FRANKIE_REPO` or `--repo`: Repository name
/// - `FRANKIE_DATABASE_URL` or `--database-url`: Local `SQLite` database path
///
/// # Example
///
/// ```no_run
/// use frankie::FrankieConfig;
/// use ortho_config::OrthoConfig;
///
/// let config = FrankieConfig::load().expect("failed to load configuration");
/// let pr_url = config.require_pr_url().expect("PR URL required");
/// let token = config.resolve_token().expect("token required");
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, OrthoConfig)]
#[serde(default)]
#[ortho_config(
    prefix = "FRANKIE",
    discovery(
        dotfile_name = ".frankie.toml",
        config_file_name = "frankie.toml",
        app_name = "frankie"
    )
)]
pub struct FrankieConfig {
    /// GitHub pull request URL to load.
    ///
    /// Can be provided via:
    /// - CLI: `--pr-url <URL>` or `-u <URL>`
    /// - Environment: `FRANKIE_PR_URL`
    /// - Config file: `pr_url = "..."`
    #[ortho_config(cli_short = 'u')]
    pub pr_url: Option<String>,

    /// Personal access token for GitHub API authentication.
    ///
    /// Can be provided via:
    /// - CLI: `--token <TOKEN>` or `-t <TOKEN>`
    /// - Environment: `FRANKIE_TOKEN` or `GITHUB_TOKEN` (legacy)
    /// - Config file: `token = "..."`
    #[ortho_config(cli_short = 't')]
    pub token: Option<String>,

    /// Repository owner (e.g., "octocat").
    ///
    /// Can be provided via:
    /// - CLI: `--owner <OWNER>` or `-o <OWNER>`
    /// - Environment: `FRANKIE_OWNER`
    /// - Config file: `owner = "..."`
    #[ortho_config(cli_short = 'o')]
    pub owner: Option<String>,

    /// Repository name (e.g., "hello-world").
    ///
    /// Can be provided via:
    /// - CLI: `--repo <REPO>` or `-r <REPO>`
    /// - Environment: `FRANKIE_REPO`
    /// - Config file: `repo = "..."`
    #[ortho_config(cli_short = 'r')]
    pub repo: Option<String>,

    /// Local `SQLite` database URL/path used for persistence.
    ///
    /// Diesel uses a filesystem path for `SQLite` connections. The same value is
    /// also used by the Diesel CLI via `DATABASE_URL` when running migrations.
    ///
    /// Can be provided via:
    /// - CLI: `--database-url <PATH>`
    /// - Environment: `FRANKIE_DATABASE_URL`
    /// - Config file: `database_url = "..."`
    #[ortho_config()]
    pub database_url: Option<String>,

    /// Runs database migrations and exits.
    ///
    /// When set, Frankie initializes the database at `database_url`, applies
    /// any pending Diesel migrations, records the schema version in telemetry,
    /// and exits without performing GitHub operations.
    ///
    /// Can be provided via:
    /// - CLI: `--migrate-db`
    /// - Environment: `FRANKIE_MIGRATE_DB`
    /// - Config file: `migrate_db = true`
    #[ortho_config()]
    pub migrate_db: bool,

    /// TTL for cached pull request metadata, in seconds.
    ///
    /// When `database_url` is set, Frankie can cache pull request metadata in
    /// the local `SQLite` database and reuse it across sessions. Entries are
    /// treated as fresh until this TTL expires, after which Frankie performs a
    /// conditional request using stored `ETag` / `Last-Modified` validators when
    /// available.
    ///
    /// Defaults to 24 hours.
    #[ortho_config()]
    pub pr_metadata_cache_ttl_seconds: u64,

    /// Disables automatic local repository discovery.
    ///
    /// When set to true, Frankie will not attempt to detect owner/repo from
    /// the current Git repository even when running in Interactive mode.
    ///
    /// Can be provided via:
    /// - CLI: `--no-local-discovery` / `-n`
    /// - Config file: `no_local_discovery = true`
    ///
    /// Note: Environment variable `FRANKIE_NO_LOCAL_DISCOVERY` is not supported
    /// because `ortho_config` does not load boolean values from the environment.
    #[ortho_config(cli_short = 'n')]
    pub no_local_discovery: bool,

    /// Enables interactive TUI mode for reviewing PR comments.
    ///
    /// When set, Frankie launches a terminal user interface for navigating
    /// and filtering review comments on a pull request.
    ///
    /// Can be provided via:
    /// - CLI: `--tui` / `-T`
    /// - Config file: `tui = true`
    #[ortho_config(cli_short = 'T')]
    pub tui: bool,
}

const DEFAULT_PR_METADATA_CACHE_TTL_SECONDS: u64 = 86_400;

impl Default for FrankieConfig {
    fn default() -> Self {
        Self {
            pr_url: None,
            token: None,
            owner: None,
            repo: None,
            database_url: None,
            migrate_db: false,
            pr_metadata_cache_ttl_seconds: DEFAULT_PR_METADATA_CACHE_TTL_SECONDS,
            no_local_discovery: false,
            tui: false,
        }
    }
}

impl FrankieConfig {
    /// Resolves the token from configuration or the legacy `GITHUB_TOKEN`
    /// environment variable.
    ///
    /// For backward compatibility, if no token is provided via `FRANKIE_TOKEN`,
    /// the CLI, or a configuration file, this method falls back to reading
    /// `GITHUB_TOKEN` from the environment.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::MissingToken`] when no token source provides a
    /// value.
    pub fn resolve_token(&self) -> Result<String, IntakeError> {
        self.token
            .clone()
            .or_else(|| env::var("GITHUB_TOKEN").ok())
            .ok_or(IntakeError::MissingToken)
    }

    /// Returns the pull request URL or an error if missing.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::MissingPullRequestUrl`] when no URL is configured.
    pub fn require_pr_url(&self) -> Result<&str, IntakeError> {
        self.pr_url
            .as_deref()
            .ok_or(IntakeError::MissingPullRequestUrl)
    }

    /// Determines the operation mode based on provided configuration.
    ///
    /// Returns `ReviewTui` if TUI mode is enabled with a PR URL, `SinglePullRequest`
    /// if a PR URL is provided without TUI, `RepositoryListing` if both owner and
    /// repo are provided, or `Interactive` otherwise.
    #[must_use]
    pub const fn operation_mode(&self) -> OperationMode {
        if self.tui && self.pr_url.is_some() {
            OperationMode::ReviewTui
        } else if self.pr_url.is_some() {
            OperationMode::SinglePullRequest
        } else if self.owner.is_some() && self.repo.is_some() {
            OperationMode::RepositoryListing
        } else {
            OperationMode::Interactive
        }
    }

    /// Returns owner and repo if both are configured.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::Configuration`] when owner or repo is missing.
    pub fn require_repository_info(&self) -> Result<(&str, &str), IntakeError> {
        match (&self.owner, &self.repo) {
            (Some(owner), Some(repo)) => Ok((owner.as_str(), repo.as_str())),
            (None, _) => Err(IntakeError::Configuration {
                message: "repository owner is required (use --owner or -o)".to_owned(),
            }),
            (_, None) => Err(IntakeError::Configuration {
                message: "repository name is required (use --repo or -r)".to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests;
