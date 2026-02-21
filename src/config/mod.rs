//! Application configuration loaded from CLI, environment, and files.
//!
//! Provides a unified configuration struct merging values from command-line
//! arguments, environment variables, and configuration files using
//! ortho-config's layered approach.
//!
//! # Precedence (lowest to highest)
//!
//! 1. **Defaults** – Built-in application defaults
//! 2. **Configuration file** – `.frankie.toml` (current dir, home, or XDG)
//! 3. **Environment variables** – `FRANKIE_*` or legacy `GITHUB_TOKEN`
//! 4. **Command-line arguments** – `--pr-url`/`-u`, `--token`/`-t`, etc.

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
    /// Export review comments in structured format.
    ExportComments,
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
/// - `FRANKIE_TEMPLATE` or `--template`: Template file path for custom export
/// - `FRANKIE_REPLY_MAX_LENGTH` or `--reply-max-length`: Max reply length
/// - `FRANKIE_REPLY_TEMPLATES` or `--reply-templates`: Reply templates
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

    /// Export format for review comments.
    ///
    /// When set, Frankie exports review comments in the specified format
    /// instead of displaying them interactively. Valid values are `markdown`,
    /// `jsonl`, and `template` (requires `--template` to specify a template file).
    ///
    /// Can be provided via:
    /// - CLI: `--export <FORMAT>` or `-e <FORMAT>`
    /// - Environment: `FRANKIE_EXPORT`
    /// - Config file: `export = "markdown"`
    #[ortho_config(cli_short = 'e')]
    pub export: Option<String>,

    /// Output file path for exported comments.
    ///
    /// When set, Frankie writes exported comments to the specified file
    /// instead of stdout. Requires `--export` to be set.
    ///
    /// Can be provided via:
    /// - CLI: `--output <PATH>`
    /// - Environment: `FRANKIE_OUTPUT`
    /// - Config file: `output = "comments.md"`
    #[ortho_config()]
    pub output: Option<String>,

    /// Template file path for custom export format.
    ///
    /// When `--export template` is used, this specifies the Jinja2-compatible
    /// template file. The template uses `minijinja` syntax with placeholders
    /// for comment fields (`{{ file }}`, `{{ line }}`, `{{ reviewer }}`, etc.).
    ///
    /// Can be provided via:
    /// - CLI: `--template <PATH>`
    /// - Environment: `FRANKIE_TEMPLATE`
    /// - Config file: `template = "my-template.j2"`
    #[ortho_config()]
    pub template: Option<String>,

    /// Local repository path for time-travel features (`--repo-path`,
    /// `FRANKIE_REPO_PATH`). Overrides auto-discovery from the current
    /// working directory.
    #[ortho_config()]
    pub repo_path: Option<String>,

    /// Maximum character count allowed for TUI reply drafts.
    ///
    /// Reply drafting enforces this limit while typing and during template
    /// insertion. Character counting is based on Unicode scalar values.
    ///
    /// Can be provided via:
    /// - CLI: `--reply-max-length <COUNT>`
    /// - Environment: `FRANKIE_REPLY_MAX_LENGTH`
    /// - Config file: `reply_max_length = 500`
    #[ortho_config()]
    pub reply_max_length: usize,

    /// Ordered template list used for keyboard reply insertion in the TUI.
    ///
    /// Templates are rendered with `MiniJinja` and can reference review-comment
    /// variables: `comment_id`, `reviewer`, `file`, `line`, and `body`.
    ///
    /// Can be provided via:
    /// - CLI: `--reply-templates '<json-array>'`
    /// - Environment: `FRANKIE_REPLY_TEMPLATES`
    /// - Config file:
    ///   `reply_templates = ["Thanks {{ reviewer }}", "..."]`
    #[ortho_config()]
    pub reply_templates: Vec<String>,

    /// Positional PR identifier (bare number or full URL) extracted from
    /// command-line arguments before ortho-config processes the remaining
    /// flags. When set, the TUI is launched without requiring `-T`.
    #[serde(skip)]
    pub pr_identifier: Option<String>,
}

const DEFAULT_PR_METADATA_CACHE_TTL_SECONDS: u64 = 86_400;
pub(crate) const DEFAULT_REPLY_MAX_LENGTH: usize = 500;

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
            export: None,
            output: None,
            template: None,
            repo_path: None,
            reply_max_length: DEFAULT_REPLY_MAX_LENGTH,
            reply_templates: default_reply_templates(),
            pr_identifier: None,
        }
    }
}

pub(crate) fn default_reply_templates() -> Vec<String> {
    vec![
        "Thanks for the review on {{ file }}:{{ line }}. I will update this.".to_owned(),
        "Good catch, {{ reviewer }}. I will address this in the next commit.".to_owned(),
        "I have addressed this feedback and pushed an update.".to_owned(),
    ]
}

impl FrankieConfig {
    /// CLI flags that consume the following argument as their value.
    ///
    /// Boolean flags (`--tui`, `--migrate-db`, `--no-local-discovery`) are
    /// omitted because they do not consume a trailing value. Update this
    /// list whenever a value-bearing flag is added to or removed from the
    /// struct fields above.
    pub const VALUE_FLAGS: &[&str] = &[
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
        "--repo-path",
        "--reply-max-length",
        "--reply-templates",
        "--config-path",
    ];

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

    const fn has_pr_url(&self) -> bool {
        self.pr_url.is_some()
    }

    const fn has_repo(&self) -> bool {
        self.owner.is_some() && self.repo.is_some()
    }

    const fn should_export_comments(&self) -> bool {
        self.export.is_some()
    }

    const fn has_pr_identifier(&self) -> bool {
        self.pr_identifier.is_some()
    }

    const fn should_review_tui(&self) -> bool {
        self.has_pr_identifier() || (self.tui && self.has_pr_url())
    }

    /// Determines the operation mode based on provided configuration.
    ///
    /// Returns `ExportComments` if export format is set (PR URL validation
    /// is deferred to `export_comments::run`), `ReviewTui` if a positional
    /// PR identifier is present or TUI mode is enabled with a PR URL,
    /// `SinglePullRequest` if a PR URL is provided without TUI or export,
    /// `RepositoryListing` if both owner and repo are provided, or
    /// `Interactive` otherwise.
    #[must_use]
    pub const fn operation_mode(&self) -> OperationMode {
        if self.should_export_comments() {
            OperationMode::ExportComments
        } else if self.should_review_tui() {
            OperationMode::ReviewTui
        } else if self.has_pr_url() {
            OperationMode::SinglePullRequest
        } else if self.has_repo() {
            OperationMode::RepositoryListing
        } else {
            OperationMode::Interactive
        }
    }

    /// Sets the positional PR identifier extracted from raw CLI arguments.
    pub fn set_pr_identifier(&mut self, value: String) {
        self.pr_identifier = Some(value);
    }

    /// Returns the positional PR identifier, if provided.
    #[must_use]
    pub fn pr_identifier(&self) -> Option<&str> {
        self.pr_identifier.as_deref()
    }

    /// Validates that the configuration is internally consistent.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::Configuration`] when both `pr_identifier` and
    /// `pr_url` are provided, since they are mutually exclusive ways to
    /// specify a pull request.
    pub fn validate(&self) -> Result<(), IntakeError> {
        if self.has_pr_identifier() && self.has_pr_url() {
            return Err(IntakeError::Configuration {
                message: concat!(
                    "positional PR identifier and --pr-url are mutually ",
                    "exclusive; provide one or the other"
                )
                .to_owned(),
            });
        }
        Ok(())
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
