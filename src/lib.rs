//! Frankie library crate providing GitHub pull request intake.
//!
//! The library wraps Octocrab to parse pull request URLs, validate tokens,
//! retrieve pull request metadata, and surface friendly errors that can be
//! displayed in the CLI.

pub mod config;
pub mod github;

pub use config::FrankieConfig;
pub use github::{
    IntakeError, OctocrabGateway, PersonalAccessToken, PullRequestDetails, PullRequestIntake,
    PullRequestLocator,
};
