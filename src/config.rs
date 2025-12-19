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
}

/// Application configuration supporting CLI, environment, and file sources.
///
/// # Environment Variables
///
/// - `FRANKIE_PR_URL` or `--pr-url`: Pull request URL
/// - `FRANKIE_TOKEN`, `GITHUB_TOKEN`, or `--token`: Authentication token
/// - `FRANKIE_OWNER` or `--owner`: Repository owner
/// - `FRANKIE_REPO` or `--repo`: Repository name
/// - `FRANKIE_DATABASE_URL` or `--database-url`: Local sqlite database path
///
/// # Example
///
/// ```no_run
/// use frankie::FrankieConfig;
///
/// let config = FrankieConfig::load().expect("failed to load configuration");
/// let pr_url = config.require_pr_url().expect("PR URL required");
/// let token = config.resolve_token().expect("token required");
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize, OrthoConfig)]
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

    /// Local sqlite database URL/path used for persistence.
    ///
    /// Diesel uses a filesystem path for sqlite connections. The same value is
    /// also used by the Diesel CLI via `DATABASE_URL` when running migrations.
    ///
    /// Can be provided via:
    /// - CLI: `--database-url <PATH>`
    /// - Environment: `FRANKIE_DATABASE_URL`
    /// - Config file: `database_url = "..."`
    pub database_url: Option<String>,

    /// Runs database migrations and exits.
    ///
    /// When set, Frankie initializes the database at `database_url`, applies
    /// any pending Diesel migrations, records the schema version in telemetry,
    /// and exits without performing GitHub operations.
    pub migrate_db: bool,
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
    /// Returns `SinglePullRequest` if a PR URL is provided, `RepositoryListing`
    /// if both owner and repo are provided, or `Interactive` otherwise.
    #[must_use]
    pub const fn operation_mode(&self) -> OperationMode {
        if self.pr_url.is_some() {
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
mod tests {
    use ortho_config::MergeComposer;
    use rstest::rstest;
    use serde_json::{Value, json};

    use super::{FrankieConfig, OperationMode};

    /// Applies a configuration layer to the composer based on the layer type.
    fn apply_layer(composer: &mut MergeComposer, layer_type: &str, value: Value) {
        match layer_type {
            "defaults" => composer.push_defaults(value),
            "file" => composer.push_file(value, None),
            "environment" => composer.push_environment(value),
            "cli" => composer.push_cli(value),
            _ => panic!("unknown layer type: {layer_type}"),
        }
    }

    #[rstest]
    #[case::file_overrides_defaults(
        vec![("defaults", json!({"pr_url": "default-url"})), ("file", json!({"pr_url": "file-url"}))],
        "pr_url",
        "file-url",
        "file should override default"
    )]
    #[case::environment_overrides_file(
        vec![("file", json!({"token": "file-token"})), ("environment", json!({"token": "env-token"}))],
        "token",
        "env-token",
        "environment should override file"
    )]
    #[case::cli_overrides_environment(
        vec![("environment", json!({"pr_url": "env-url"})), ("cli", json!({"pr_url": "cli-url"}))],
        "pr_url",
        "cli-url",
        "CLI should override environment"
    )]
    fn test_layer_precedence(
        #[case] layers: Vec<(&str, Value)>,
        #[case] field: &str,
        #[case] expected: &str,
        #[case] message: &str,
    ) {
        let mut composer = MergeComposer::new();

        for (layer_type, value) in layers {
            apply_layer(&mut composer, layer_type, value);
        }

        let config =
            FrankieConfig::merge_from_layers(composer.layers()).expect("merge should succeed");

        let actual = match field {
            "pr_url" => config.pr_url.as_deref(),
            "token" => config.token.as_deref(),
            _ => panic!("unknown field: {field}"),
        };

        assert_eq!(actual, Some(expected), "{message}");
    }

    #[rstest]
    fn defaults_are_none_when_no_sources_provided() {
        let mut composer = MergeComposer::new();
        composer.push_defaults(json!({"pr_url": null, "token": null}));

        let config = FrankieConfig::merge_from_layers(composer.layers())
            .expect("merge should succeed with empty defaults");

        assert!(config.pr_url.is_none(), "pr_url should be None");
        assert!(config.token.is_none(), "token should be None");
    }

    #[rstest]
    fn full_precedence_chain() {
        let mut composer = MergeComposer::new();
        composer.push_defaults(json!({"pr_url": "default", "token": "default-token"}));
        composer.push_file(json!({"pr_url": "file", "token": "file-token"}), None);
        composer.push_environment(json!({"pr_url": "env"}));
        composer.push_cli(json!({"pr_url": "cli"}));

        let config =
            FrankieConfig::merge_from_layers(composer.layers()).expect("merge should succeed");

        assert_eq!(config.pr_url.as_deref(), Some("cli"), "CLI wins for pr_url");
        assert_eq!(
            config.token.as_deref(),
            Some("file-token"),
            "file wins for token (no env/cli override)"
        );
    }

    #[rstest]
    fn partial_overrides_preserve_lower_values() {
        let mut composer = MergeComposer::new();
        composer.push_defaults(json!({"pr_url": "default-url", "token": "default-token"}));
        composer.push_cli(json!({"pr_url": "cli-url"}));

        let config =
            FrankieConfig::merge_from_layers(composer.layers()).expect("merge should succeed");

        assert_eq!(
            config.pr_url.as_deref(),
            Some("cli-url"),
            "CLI should override pr_url"
        );
        assert_eq!(
            config.token.as_deref(),
            Some("default-token"),
            "default token should be preserved"
        );
    }

    #[rstest]
    #[case::pr_url(
        FrankieConfig { pr_url: Some("https://github.com/owner/repo/pull/1".to_owned()), ..Default::default() },
        "https://github.com/owner/repo/pull/1",
        false
    )]
    #[case::token(
        FrankieConfig { token: Some("my-token".to_owned()), ..Default::default() },
        "my-token",
        true
    )]
    fn returns_value_when_field_present(
        #[case] config: FrankieConfig,
        #[case] expected: &str,
        #[case] is_token: bool,
    ) {
        if is_token {
            let result = config.resolve_token();
            assert_eq!(
                result.ok(),
                Some(expected.to_owned()),
                "should return the token"
            );
        } else {
            let result = config.require_pr_url();
            assert_eq!(result.ok(), Some(expected), "should return the URL");
        }
    }

    #[rstest]
    #[case::pr_url(false)]
    #[case::token(true)]
    fn returns_error_when_field_none(#[case] is_token: bool) {
        let config = FrankieConfig::default();

        if is_token {
            // Lock and clear GITHUB_TOKEN to ensure test isolation
            let _guard = env_lock::lock_env([("GITHUB_TOKEN", None::<&str>)]);
            let result = config.resolve_token();
            assert!(result.is_err(), "should return error when token is None");
        } else {
            let result = config.require_pr_url();
            assert!(result.is_err(), "should return error when pr_url is None");
        }
    }

    #[rstest]
    fn operation_mode_single_pr_when_pr_url_present() {
        let config = FrankieConfig {
            pr_url: Some("https://github.com/owner/repo/pull/1".to_owned()),
            ..Default::default()
        };

        assert_eq!(
            config.operation_mode(),
            OperationMode::SinglePullRequest,
            "should be SinglePullRequest when pr_url is set"
        );
    }

    #[rstest]
    fn operation_mode_repository_listing_when_owner_and_repo_present() {
        let config = FrankieConfig {
            owner: Some("octocat".to_owned()),
            repo: Some("hello-world".to_owned()),
            ..Default::default()
        };

        assert_eq!(
            config.operation_mode(),
            OperationMode::RepositoryListing,
            "should be RepositoryListing when owner and repo are set"
        );
    }

    #[rstest]
    fn operation_mode_interactive_when_no_fields_set() {
        let config = FrankieConfig::default();

        assert_eq!(
            config.operation_mode(),
            OperationMode::Interactive,
            "should be Interactive when no fields are set"
        );
    }

    #[rstest]
    fn pr_url_takes_precedence_over_owner_repo() {
        let config = FrankieConfig {
            pr_url: Some("https://github.com/owner/repo/pull/1".to_owned()),
            owner: Some("octocat".to_owned()),
            repo: Some("hello-world".to_owned()),
            ..Default::default()
        };

        assert_eq!(
            config.operation_mode(),
            OperationMode::SinglePullRequest,
            "pr_url should take precedence over owner/repo"
        );
    }

    #[rstest]
    fn require_repository_info_returns_error_when_owner_missing() {
        let config = FrankieConfig {
            repo: Some("hello-world".to_owned()),
            ..Default::default()
        };

        let result = config.require_repository_info();
        assert!(result.is_err(), "should return error when owner is missing");
    }

    #[rstest]
    fn require_repository_info_returns_error_when_repo_missing() {
        let config = FrankieConfig {
            owner: Some("octocat".to_owned()),
            ..Default::default()
        };

        let result = config.require_repository_info();
        assert!(result.is_err(), "should return error when repo is missing");
    }

    #[rstest]
    fn require_repository_info_returns_values_when_present() {
        let config = FrankieConfig {
            owner: Some("octocat".to_owned()),
            repo: Some("hello-world".to_owned()),
            ..Default::default()
        };

        let result = config.require_repository_info();
        assert_eq!(
            result.ok(),
            Some(("octocat", "hello-world")),
            "should return owner and repo"
        );
    }
}
