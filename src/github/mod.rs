//! GitHub pull request intake and token validation.
//!
//! This module wraps Octocrab to parse pull request URLs, validate personal
//! access tokens, and retrieve pull request metadata alongside discussion
//! comments. Errors are mapped into user-friendly variants so that callers can
//! surface precise failures without exposing Octocrab internals.

pub mod error;
pub mod gateway;
pub mod intake;
pub mod locator;
pub mod models;

pub use error::IntakeError;
pub use gateway::{OctocrabGateway, PullRequestGateway};
pub use intake::PullRequestIntake;
pub use locator::{
    PersonalAccessToken, PullRequestLocator, PullRequestNumber, RepositoryName, RepositoryOwner,
};
pub use models::{PullRequestComment, PullRequestDetails, PullRequestMetadata};

#[cfg(test)]
pub use gateway::MockPullRequestGateway;

#[cfg(test)]
mod tests;
