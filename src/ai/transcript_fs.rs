//! Capability-based filesystem plumbing for transcript persistence.
//!
//! Resolves parent directories through `cap-std` handles so transcript
//! files can be created without ambient path traversal at write time.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;

use crate::github::IntakeError;

/// Opens the current working directory as an ambient capability handle.
fn open_current_dir(path_type: &str) -> Result<Dir, IntakeError> {
    Dir::open_ambient_dir(".", ambient_authority()).map_err(|error| IntakeError::Io {
        message: format!("failed to open current directory for {path_type}s: {error}"),
    })
}

/// Resolves `parent` to a base directory handle and a path relative to it.
fn resolve_parent_dir<'a>(
    parent: &'a Utf8Path,
    path_type: &str,
) -> Result<(Dir, &'a Utf8Path), IntakeError> {
    if parent == Utf8Path::new(".") || parent.as_str().is_empty() {
        return Ok((open_current_dir(path_type)?, Utf8Path::new(".")));
    }

    if parent.is_absolute() {
        let root =
            Dir::open_ambient_dir("/", ambient_authority()).map_err(|error| IntakeError::Io {
                message: format!("failed to open root directory for {path_type}s: {error}"),
            })?;
        let rel = parent.strip_prefix("/").map_err(|_| IntakeError::Io {
            message: format!("failed to normalise {path_type} directory '{parent}'"),
        })?;
        return Ok((root, rel));
    }

    Ok((open_current_dir(path_type)?, parent))
}

/// Ensures `rel_parent` exists under `dir` and returns a handle to it.
fn open_target_dir(
    dir: Dir,
    rel_parent: &Utf8Path,
    parent: &Utf8Path,
    path_type: &str,
) -> Result<Dir, IntakeError> {
    if rel_parent.as_str().is_empty() || rel_parent == Utf8Path::new(".") {
        return Ok(dir);
    }

    dir.create_dir_all(rel_parent)
        .map_err(|error| IntakeError::Io {
            message: format!("failed to create {path_type} directory '{parent}': {error}"),
        })?;
    dir.open_dir(rel_parent).map_err(|error| IntakeError::Io {
        message: format!("failed to open {path_type} directory '{parent}': {error}"),
    })
}

/// Creates a file at `path`, ensuring parent directories exist first.
///
/// Returns the canonical path and the opened file handle.  All directory
/// and path logic is centralised here so callers do not need to coordinate
/// `ensure_parent_dirs` and `open_dir_for_path` separately.
pub(super) fn create_file_with_parents(
    path: &Utf8Path,
    path_type: &str,
) -> Result<(Utf8PathBuf, cap_std::fs_utf8::File), IntakeError> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = path.file_name().ok_or_else(|| IntakeError::Io {
        message: format!("invalid {path_type} path '{path}': no file name"),
    })?;

    let (dir, rel_parent) = resolve_parent_dir(parent, path_type)?;
    let target_dir = open_target_dir(dir, rel_parent, parent, path_type)?;

    let file = target_dir
        .create(file_name)
        .map_err(|error| IntakeError::Io {
            message: format!("failed to create {path_type} file '{path}': {error}"),
        })?;

    Ok((path.to_path_buf(), file))
}
