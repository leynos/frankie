//! Credential resolution: configuration first, then the legacy variables.
//!
//! Each resolver has a `_with` form that reads variables through an injected
//! lookup closure, so a test can name the variables it needs without changing
//! the process environment. The plain form passes [`process_lookup`], the
//! production composition root of that seam.

use std::env;

use super::FrankieConfig;
use crate::github::error::IntakeError;

/// Reads `name` from the process environment as text.
///
/// A variable that is unset, or whose value is not valid Unicode, reads as
/// absent.
fn process_lookup(name: &str) -> Option<String> {
    env::var(name).ok()
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
        self.resolve_token_with(process_lookup)
    }

    /// Resolves the token from configuration or the legacy `GITHUB_TOKEN`
    /// variable, read through `lookup`.
    ///
    /// [`resolve_token`](Self::resolve_token) passes the process environment;
    /// tests pass a closure over a fixed set of variables, so nothing reads or
    /// writes the process. `lookup` resolves a variable by name and returns
    /// `None` when it is unset or not valid Unicode.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::MissingToken`] when no token source provides a
    /// value.
    pub fn resolve_token_with(
        &self,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<String, IntakeError> {
        self.token
            .clone()
            .or_else(|| lookup("GITHUB_TOKEN"))
            .ok_or(IntakeError::MissingToken)
    }

    /// Resolves the AI API key from configuration or `OPENAI_API_KEY`.
    #[must_use]
    pub fn resolve_ai_api_key(&self) -> Option<String> {
        self.resolve_ai_api_key_with(process_lookup)
    }

    /// Resolves the AI API key from configuration or `OPENAI_API_KEY`, read
    /// through `lookup` (see [`resolve_token_with`](Self::resolve_token_with)).
    #[must_use]
    pub fn resolve_ai_api_key_with(
        &self,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Option<String> {
        self.ai_api_key.clone().or_else(|| lookup("OPENAI_API_KEY"))
    }
}
