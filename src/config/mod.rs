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
use std::ffi::OsString;

use ortho_config::OrthoConfig;

use crate::github::error::IntakeError;

mod model;
mod summarize_mode;

pub(crate) use model::DEFAULT_REPLY_MAX_LENGTH;
pub use model::{DEFAULT_COMMIT_HISTORY_LIMIT, FrankieConfig};

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
    /// AI-powered draft rewrite mode.
    AiRewrite,
    /// Verify comment resolutions against local git state.
    VerifyResolutions,
    /// Generate an AI summary for PR-level discussions.
    SummarizeDiscussions,
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
        "--ai-rewrite-mode",
        "--ai-rewrite-text",
        "--ai-base-url",
        "--ai-model",
        "--ai-api-key",
        "--ai-timeout-seconds",
        "--commit-history-limit",
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

    /// Resolves the AI API key from configuration or `OPENAI_API_KEY`.
    #[must_use]
    pub fn resolve_ai_api_key(&self) -> Option<String> {
        self.ai_api_key
            .clone()
            .or_else(|| env::var("OPENAI_API_KEY").ok())
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

    fn non_empty_trimmed(value: Option<&str>) -> bool {
        value.is_some_and(|text| !text.trim().is_empty())
    }

    fn rewrite_mode_present(&self) -> bool {
        Self::non_empty_trimmed(self.ai_rewrite_mode.as_deref())
    }

    fn rewrite_text_present(&self) -> bool {
        Self::non_empty_trimmed(self.ai_rewrite_text.as_deref())
    }

    fn should_ai_rewrite(&self) -> bool {
        self.rewrite_mode_present() || self.rewrite_text_present()
    }

    const fn is_verify_resolutions_mode(&self) -> bool {
        self.verify_resolutions
    }

    const fn is_summarize_discussions_mode(&self) -> bool {
        summarize_mode::is_summarize_discussions_mode(self)
    }

    fn is_ai_rewrite_mode(&self) -> bool {
        self.should_ai_rewrite()
    }

    const fn is_export_comments_mode(&self) -> bool {
        self.export.is_some()
    }

    const fn is_review_tui_mode(&self) -> bool {
        self.has_pr_identifier() || (self.tui && self.has_pr_url())
    }

    const fn is_single_pull_request_mode(&self) -> bool {
        self.has_pr_url()
    }

    const fn is_repository_listing_mode(&self) -> bool {
        self.has_repo()
    }

    const fn has_pr_identifier(&self) -> bool {
        self.pr_identifier.is_some()
    }

    fn resolve_operation_mode(&self) -> OperationMode {
        if self.is_verify_resolutions_mode() {
            OperationMode::VerifyResolutions
        } else if self.is_summarize_discussions_mode() {
            OperationMode::SummarizeDiscussions
        } else if self.is_ai_rewrite_mode() {
            OperationMode::AiRewrite
        } else if self.is_export_comments_mode() {
            OperationMode::ExportComments
        } else if self.is_review_tui_mode() {
            OperationMode::ReviewTui
        } else if self.is_single_pull_request_mode() {
            OperationMode::SinglePullRequest
        } else if self.is_repository_listing_mode() {
            OperationMode::RepositoryListing
        } else {
            OperationMode::Interactive
        }
    }

    /// Determines the operation mode based on provided configuration.
    ///
    /// Returns `AiRewrite` if AI rewrite fields are set, `ExportComments` if
    /// export format is set (PR URL validation is deferred to
    /// `export_comments::run`), `ReviewTui` if a positional PR identifier is
    /// present or TUI mode is enabled with a PR URL, `SinglePullRequest` if a
    /// PR URL is provided without TUI or export, `RepositoryListing` if both
    /// owner and repo are provided, or `Interactive` otherwise.
    #[must_use]
    pub fn operation_mode(&self) -> OperationMode {
        self.resolve_operation_mode()
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

    /// Normalizes configuration values to ensure valid ranges.
    ///
    /// Should be called immediately after loading configuration and before
    /// validation. This method clamps values that would otherwise be invalid
    /// but can be safely corrected (for example, a commit history limit of 0
    /// is clamped to 1).
    pub fn normalize(&mut self) {
        self.commit_history_limit = self.commit_history_limit.max(1);
    }

    /// Loads configuration from CLI arguments, environment variables, and
    /// configuration files.
    ///
    /// This method delegates to the OrthoConfig-derived loader and
    /// automatically calls `normalize()` on the result before returning.
    ///
    /// # Errors
    ///
    /// Returns an error when argument parsing fails or configuration files
    /// cannot be loaded.
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config =
            <Self as OrthoConfig>::load().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        config.normalize();
        Ok(config)
    }

    /// Loads configuration from the given iterator of arguments, along with
    /// environment variables and configuration files.
    ///
    /// This method delegates to the OrthoConfig-derived loader and
    /// automatically calls `normalize()` on the result before returning.
    ///
    /// # Errors
    ///
    /// Returns an error when argument parsing fails or configuration files
    /// cannot be loaded.
    pub fn load_from_iter(
        iter: impl IntoIterator<Item = impl Into<OsString> + Clone>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = <Self as OrthoConfig>::load_from_iter(iter)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        config.normalize();
        Ok(config)
    }

    /// Validates that the configuration is internally consistent.
    ///
    /// Checks that:
    /// - Positional PR identifier and `--pr-url` are not both provided
    /// - AI rewrite mode and text are both present when either is specified
    /// - Verify resolutions mode has compatible configuration
    /// - Summary mode has compatible configuration
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::Configuration`] when:
    /// - Both `pr_identifier` and `pr_url` are provided (mutually exclusive)
    /// - AI rewrite mode is specified without text, or vice versa
    /// - Verify resolutions mode is incompatible with current configuration
    /// - Summary mode is incompatible with current configuration
    pub fn validate(&self) -> Result<(), IntakeError> {
        self.validate_pr_identifier_exclusivity()?;
        self.validate_ai_rewrite_completeness()?;
        self.validate_verify_resolutions_compatibility()?;
        self.validate_summary_mode_compatibility()?;
        Ok(())
    }

    fn validate_pr_identifier_exclusivity(&self) -> Result<(), IntakeError> {
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

    fn validate_summary_mode_compatibility(&self) -> Result<(), IntakeError> {
        summarize_mode::validate_summary_mode_compatibility(self)
    }

    fn validate_ai_rewrite_completeness(&self) -> Result<(), IntakeError> {
        let mode_present = self.rewrite_mode_present();
        let text_present = self.rewrite_text_present();
        let mode_without_text = mode_present && !text_present;
        let text_without_mode = !mode_present && text_present;
        if mode_without_text || text_without_mode {
            return Err(IntakeError::Configuration {
                message: concat!(
                    "--ai-rewrite-mode and --ai-rewrite-text must be provided ",
                    "together"
                )
                .to_owned(),
            });
        }

        Ok(())
    }

    fn validate_verify_resolutions_compatibility(&self) -> Result<(), IntakeError> {
        if self.verify_resolutions && self.should_ai_rewrite() {
            return Err(IntakeError::Configuration {
                message: concat!(
                    "--verify-resolutions cannot be combined with AI rewrite ",
                    "flags; remove --ai-rewrite-mode/--ai-rewrite-text"
                )
                .to_owned(),
            });
        }

        if self.verify_resolutions && self.should_export_comments() {
            return Err(IntakeError::Configuration {
                message: "--verify-resolutions cannot be combined with --export".to_owned(),
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
