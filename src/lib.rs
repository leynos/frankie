//! Frankie library crate providing GitHub pull request intake.
//!
//! The library wraps Octocrab to parse pull request URLs, validate tokens,
//! retrieve pull request metadata, and surface friendly errors that can be
//! displayed in the CLI.

pub mod config;
pub mod export;
pub mod github;
pub mod local;
pub mod persistence;
pub mod telemetry;
pub mod tui;

pub use config::{FrankieConfig, OperationMode};
pub use export::{ExportFormat, ExportedComment, sort_comments, write_jsonl, write_markdown};
pub use github::{
    IntakeError, ListPullRequestsParams, OctocrabCachingGateway, OctocrabGateway,
    OctocrabRepositoryGateway, OctocrabReviewCommentGateway, PageInfo, PaginatedPullRequests,
    PersonalAccessToken, PullRequestDetails, PullRequestIntake, PullRequestLocator,
    PullRequestState, PullRequestSummary, RateLimitInfo, RepositoryIntake, RepositoryLocator,
    ReviewComment, ReviewCommentGateway,
};
pub use local::{GitHubOrigin, LocalDiscoveryError, LocalRepository, discover_repository};
