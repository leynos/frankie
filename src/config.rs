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
//! ```

use std::env;

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

use crate::github::error::IntakeError;

/// Application configuration supporting CLI, environment, and file sources.
///
/// # Environment Variables
///
/// - `FRANKIE_PR_URL` or `--pr-url`: Pull request URL (required)
/// - `FRANKIE_TOKEN`, `GITHUB_TOKEN`, or `--token`: Authentication token
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
}

#[cfg(test)]
mod tests {
    use ortho_config::MergeComposer;
    use rstest::rstest;
    use serde_json::{Value, json};

    use super::FrankieConfig;

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
            match layer_type {
                "defaults" => composer.push_defaults(value),
                "file" => composer.push_file(value, None),
                "environment" => composer.push_environment(value),
                "cli" => composer.push_cli(value),
                _ => panic!("unknown layer type: {layer_type}"),
            }
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
    fn require_pr_url_returns_error_when_none() {
        let config = FrankieConfig {
            pr_url: None,
            token: None,
        };

        let result = config.require_pr_url();
        assert!(result.is_err(), "should return error when pr_url is None");
    }

    #[rstest]
    fn require_pr_url_returns_value_when_present() {
        let config = FrankieConfig {
            pr_url: Some("https://github.com/owner/repo/pull/1".to_owned()),
            token: None,
        };

        let result = config.require_pr_url();
        assert_eq!(
            result.ok(),
            Some("https://github.com/owner/repo/pull/1"),
            "should return the URL"
        );
    }

    #[rstest]
    fn resolve_token_returns_error_when_none() {
        let config = FrankieConfig {
            pr_url: None,
            token: None,
        };

        // Clear GITHUB_TOKEN if set to ensure test isolation
        // SAFETY: This test runs in isolation and does not rely on this
        // environment variable being set for any concurrent thread.
        unsafe { std::env::remove_var("GITHUB_TOKEN") };

        let result = config.resolve_token();
        assert!(result.is_err(), "should return error when token is None");
    }

    #[rstest]
    fn resolve_token_returns_value_when_present() {
        let config = FrankieConfig {
            pr_url: None,
            token: Some("my-token".to_owned()),
        };

        let result = config.resolve_token();
        assert_eq!(
            result.ok(),
            Some("my-token".to_owned()),
            "should return the token"
        );
    }
}
