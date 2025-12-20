//! Octocrab client construction helpers for gateway implementations.

use http::Uri;
use octocrab::Octocrab;

use crate::github::error::IntakeError;
use crate::github::locator::PersonalAccessToken;

use super::error_mapping::map_octocrab_error;

/// Builds an Octocrab client for the given token and API base URL.
///
/// This helper consolidates the shared logic for parsing the base URI and
/// constructing an authenticated Octocrab client.
///
/// # Errors
///
/// Returns `IntakeError::InvalidUrl` when the base URI cannot be parsed or
/// `IntakeError::Api` when Octocrab fails to construct a client.
pub(super) fn build_octocrab_client(
    token: &PersonalAccessToken,
    api_base: &str,
) -> Result<Octocrab, IntakeError> {
    let base_uri: Uri = api_base
        .parse::<Uri>()
        .map_err(|error| IntakeError::InvalidUrl(error.to_string()))?;

    Octocrab::builder()
        .personal_token(token.as_ref())
        .base_uri(base_uri)
        .map_err(|error| IntakeError::Api {
            message: format!("build client failed: {error}"),
        })?
        .build()
        .map_err(|error| map_octocrab_error("build client", &error))
}
