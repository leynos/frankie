//! Error mapping helpers for the Octocrab GitHub gateway implementations.

use http::StatusCode;

use crate::github::error::IntakeError;
use crate::persistence::PersistenceError;

/// Checks if a GitHub error status indicates an authentication failure.
pub(super) const fn is_auth_failure(status: StatusCode) -> bool {
    matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
}

/// Checks if an octocrab error represents a network/transport issue.
pub(super) const fn is_network_error(error: &octocrab::Error) -> bool {
    matches!(
        error,
        octocrab::Error::Http { .. }
            | octocrab::Error::Hyper { .. }
            | octocrab::Error::Service { .. }
    )
}

/// Checks whether the GitHub error represents a rate limit error based on the
/// HTTP status and message / documentation URL content.
pub(super) fn is_rate_limit_error(source: &octocrab::GitHubError) -> bool {
    let is_rate_limit_status = matches!(
        source.status_code,
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
    );

    let message_indicates_rate_limit = source.message.to_lowercase().contains("rate limit")
        || source
            .documentation_url
            .as_deref()
            .is_some_and(|url| url.contains("rate-limit"));

    is_rate_limit_status && message_indicates_rate_limit
}

pub(super) fn map_octocrab_error(operation: &str, error: &octocrab::Error) -> IntakeError {
    if let octocrab::Error::GitHub { source, .. } = error {
        return if is_auth_failure(source.status_code) {
            IntakeError::Authentication {
                message: format!(
                    "{operation} failed: GitHub returned {status} {message}",
                    status = source.status_code,
                    message = source.message
                ),
            }
        } else {
            IntakeError::Api {
                message: format!(
                    "{operation} failed with status {status}: {message}",
                    status = source.status_code,
                    message = source.message
                ),
            }
        };
    }

    if is_network_error(error) {
        return IntakeError::Network {
            message: format!("{operation} failed: {error}"),
        };
    }

    IntakeError::Api {
        message: format!("{operation} failed: {error}"),
    }
}

pub(super) fn map_http_error(
    operation: &str,
    status: StatusCode,
    maybe_message: Option<String>,
) -> IntakeError {
    let message = maybe_message.unwrap_or_else(|| "unknown error".to_owned());
    if is_auth_failure(status) {
        IntakeError::Authentication {
            message: format!("{operation} failed: GitHub returned {status} {message}"),
        }
    } else {
        IntakeError::Api {
            message: format!("{operation} failed with status {status}: {message}"),
        }
    }
}

pub(super) fn map_persistence_error(operation: &str, error: &PersistenceError) -> IntakeError {
    match error {
        PersistenceError::MissingDatabaseUrl
        | PersistenceError::BlankDatabaseUrl
        | PersistenceError::SchemaNotInitialised => IntakeError::Configuration {
            message: format!("{operation}: {error}"),
        },
        _ => IntakeError::Io {
            message: format!("{operation}: {error}"),
        },
    }
}
