//! Shared test utilities.

use tempfile::TempDir;

pub mod pr_metadata_cache_helpers;

/// Creates a temporary directory for database tests.
///
/// # Panics
///
/// Panics if the temporary directory cannot be created.
pub fn create_temp_dir() -> TempDir {
    TempDir::new().unwrap_or_else(|error| panic!("failed to create temporary directory: {error}"))
}
