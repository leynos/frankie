//! Local Git repository discovery.
//!
//! This module provides functionality to detect whether the current working
//! directory is inside a Git repository and extract GitHub origin information
//! from configured remotes.
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

mod discovery;
mod error;
mod remote;

pub use discovery::{LocalRepository, discover_repository};
pub use error::LocalDiscoveryError;
pub use remote::GitHubOrigin;

#[cfg(test)]
mod tests;
