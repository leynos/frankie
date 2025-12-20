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
pub mod pagination;
pub mod rate_limit;
pub mod repository_intake;

pub use error::IntakeError;
pub use gateway::{
    ListPullRequestsParams, OctocrabCachingGateway, OctocrabGateway, OctocrabRepositoryGateway,
    PaginatedPullRequests, PullRequestGateway, PullRequestState, RepositoryGateway,
};
pub use intake::PullRequestIntake;
pub use locator::{
    PersonalAccessToken, PullRequestLocator, PullRequestNumber, RepositoryLocator, RepositoryName,
    RepositoryOwner,
};
pub use models::{PullRequestComment, PullRequestDetails, PullRequestMetadata, PullRequestSummary};
pub use pagination::PageInfo;
pub use rate_limit::RateLimitInfo;
pub use repository_intake::RepositoryIntake;

#[cfg(test)]
pub use gateway::{MockPullRequestGateway, MockRepositoryGateway};

#[cfg(test)]
mod tests;
