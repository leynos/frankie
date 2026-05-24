//! Shared orchestration services for time-travel loading and navigation.
//!
//! These functions keep Git-backed time-travel orchestration in the library
//! layer so hosts can materialize and navigate historical snapshots without
//! depending on Bubble Tea, Tokio, or TUI-only storage.

mod line_mapping;
mod loading;
mod navigation;

pub use loading::load_time_travel_state;
pub use navigation::{TimeTravelNavigationDirection, navigate_time_travel_state};

use crate::local::GitOperationError;

pub(super) const fn git_error_type(error: &GitOperationError) -> &'static str {
    match error {
        GitOperationError::CommitNotFound { .. } => "commit_not_found",
        GitOperationError::FileNotFound { .. } => "file_not_found",
        GitOperationError::CommitAccessFailed { .. } => "commit_access_failed",
        GitOperationError::DiffComputationFailed { .. } => "diff_computation_failed",
        GitOperationError::RepositoryNotAvailable { .. } => "repository_not_available",
        GitOperationError::Git { .. } => "git",
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
