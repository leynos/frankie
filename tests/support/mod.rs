//! Shared test utilities.

use tempfile::TempDir;

/// Creates a temporary directory for database tests.
///
/// # Errors
///
/// Returns an error if the temporary directory cannot be created.
pub fn create_temp_dir() -> Result<TempDir, std::io::Error> {
    TempDir::new()
}
