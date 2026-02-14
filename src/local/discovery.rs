//! Local Git repository discovery.
//!
//! This module provides functionality to discover the Git repository containing
//! the current working directory and extract GitHub origin information.

use std::path::{Path, PathBuf};

use git2::Repository;

use super::error::LocalDiscoveryError;
use super::remote::{GitHubOrigin, parse_github_remote};

/// Default remote name to look for when discovering GitHub origin.
const DEFAULT_REMOTE_NAME: &str = "origin";

/// Represents a discovered local Git repository with GitHub origin information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRepository {
    /// Path to the repository working directory.
    workdir: PathBuf,
    /// Parsed GitHub origin information.
    github_origin: GitHubOrigin,
    /// Name of the remote used (typically "origin").
    remote_name: String,
}

impl LocalRepository {
    /// Returns the path to the repository working directory.
    #[must_use]
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    /// Returns the parsed GitHub origin.
    #[must_use]
    pub const fn github_origin(&self) -> &GitHubOrigin {
        &self.github_origin
    }

    /// Returns the repository owner.
    #[must_use]
    pub fn owner(&self) -> &str {
        self.github_origin.owner()
    }

    /// Returns the repository name.
    #[must_use]
    pub fn repository(&self) -> &str {
        self.github_origin.repository()
    }

    /// Returns the name of the remote used for discovery.
    #[must_use]
    pub fn remote_name(&self) -> &str {
        &self.remote_name
    }

    /// Resolves the HEAD commit SHA from the repository working directory.
    ///
    /// Opens the repository from `self.workdir`, resolves HEAD, peels to the
    /// commit, and returns the OID as a hex string.
    ///
    /// # Errors
    ///
    /// Returns a descriptive error string if the repository cannot be opened,
    /// HEAD cannot be resolved, or the reference cannot be peeled to a commit.
    pub fn head_sha(&self) -> Result<String, String> {
        let repo = Repository::open(&self.workdir)
            .map_err(|e| format!("failed to open repository for HEAD: {e}"))?;
        let head = repo
            .head()
            .map_err(|e| format!("failed to resolve HEAD: {e}"))?;
        let oid = head
            .peel_to_commit()
            .map_err(|e| format!("failed to resolve HEAD commit: {e}"))?
            .id();
        Ok(oid.to_string())
    }
}

/// Discovers the local Git repository and extracts GitHub origin information.
///
/// Starting from `start_path`, searches upward for a Git repository. If found,
/// attempts to parse the "origin" remote URL as a GitHub origin.
///
/// # Arguments
///
/// * `start_path` - The directory from which to start searching for a Git
///   repository.
///
/// # Errors
///
/// Returns an error if:
/// - The path is not within a Git repository (`NotARepository`)
/// - The repository has no remotes configured (`NoRemotes`)
/// - The "origin" remote does not exist (`RemoteNotFound`)
/// - The remote URL cannot be parsed (`InvalidRemoteUrl`)
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use frankie::local::discover_repository;
///
/// let result = discover_repository(Path::new("."));
/// match result {
///     Ok(local_repo) => {
///         println!("Found: {}/{}", local_repo.owner(), local_repo.repository());
///     }
///     Err(e) => eprintln!("Discovery failed: {e}"),
/// }
/// ```
pub fn discover_repository(start_path: &Path) -> Result<LocalRepository, LocalDiscoveryError> {
    discover_repository_with_remote(start_path, DEFAULT_REMOTE_NAME)
}

/// Discovers the local Git repository using a specific remote name.
///
/// Like [`discover_repository`], but allows specifying which remote to use
/// instead of the default "origin".
///
/// # Errors
///
/// Returns the same errors as [`discover_repository`].
pub fn discover_repository_with_remote(
    start_path: &Path,
    remote_name: &str,
) -> Result<LocalRepository, LocalDiscoveryError> {
    let repo = open_repository(start_path)?;
    let workdir = get_workdir(&repo)?;
    let github_origin = get_github_origin(&repo, remote_name)?;

    Ok(LocalRepository {
        workdir,
        github_origin,
        remote_name: remote_name.to_owned(),
    })
}

/// Opens a Git repository starting from the given path.
fn open_repository(start_path: &Path) -> Result<Repository, LocalDiscoveryError> {
    Repository::discover(start_path).map_err(|error| {
        if error.code() == git2::ErrorCode::NotFound {
            LocalDiscoveryError::NotARepository
        } else {
            LocalDiscoveryError::from(error)
        }
    })
}

/// Gets the working directory of the repository.
fn get_workdir(repo: &Repository) -> Result<PathBuf, LocalDiscoveryError> {
    repo.workdir()
        .map(Path::to_path_buf)
        .ok_or(LocalDiscoveryError::NotARepository)
}

/// Gets the GitHub origin from the specified remote.
fn get_github_origin(
    repo: &Repository,
    remote_name: &str,
) -> Result<GitHubOrigin, LocalDiscoveryError> {
    // Check if there are any remotes
    let remotes = repo.remotes()?;
    if remotes.is_empty() {
        return Err(LocalDiscoveryError::NoRemotes);
    }

    // Try to get the specified remote
    let remote = repo.find_remote(remote_name).map_err(|error| {
        if error.code() == git2::ErrorCode::NotFound {
            LocalDiscoveryError::RemoteNotFound {
                name: remote_name.to_owned(),
            }
        } else {
            LocalDiscoveryError::from(error)
        }
    })?;

    // Get the remote URL
    let url = remote
        .url()
        .ok_or_else(|| LocalDiscoveryError::InvalidRemoteUrl {
            url: "(no URL)".to_owned(),
        })?;

    // Parse the URL as a GitHub origin
    parse_github_remote(url)
}
