//! Local Git repository operations.
//!
//! This module provides functionality to detect whether the current working
//! directory is inside a Git repository, extract GitHub origin information
//! from configured remotes, and perform Git operations for time-travel
//! navigation across PR history.
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use frankie::local::{discover_repository, GitHubOrigin};
//!
//! let result = discover_repository(Path::new("."));
//! match result {
//!     Ok(local_repo) => {
//!         println!("Found repository: {}/{}", local_repo.owner(), local_repo.repository());
//!     }
//!     Err(e) => eprintln!("Discovery failed: {e}"),
//! }
//! ```

mod commit;
mod discovery;
mod error;
mod git_ops;
mod remote;

pub use commit::{CommitSnapshot, LineMappingStatus, LineMappingVerification};
pub use discovery::{LocalRepository, discover_repository};
pub use error::{GitOperationError, LocalDiscoveryError};
pub use git_ops::{Git2Operations, GitOperations, create_git_ops};
pub use remote::GitHubOrigin;

#[cfg(test)]
mod tests;
