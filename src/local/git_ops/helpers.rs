//! Helper functions for Git operations, particularly diff analysis.

use std::path::Path;

use git2::{DiffOptions, Oid, Repository};

use crate::local::commit::LineMappingVerification;
use crate::local::error::GitOperationError;

/// Parses a SHA string into an Oid using the repository.
///
/// Attempts to parse as a full SHA first, then tries as a short SHA or ref.
pub(super) fn parse_sha_with_repo(repo: &Repository, sha: &str) -> Result<Oid, GitOperationError> {
    // Try to parse as full SHA first
    if let Ok(oid) = Oid::from_str(sha) {
        return Ok(oid);
    }

    // Try as a short SHA or ref
    let obj = repo
        .revparse_single(sha)
        .map_err(|_| GitOperationError::CommitNotFound {
            sha: sha.to_owned(),
        })?;

    Ok(obj.id())
}

/// Gets the blob OID for a file at a specific commit.
pub(super) fn get_file_blob_oid(
    commit: &git2::Commit<'_>,
    file_path: &str,
) -> Result<Oid, GitOperationError> {
    let tree = commit.tree()?;
    let entry =
        tree.get_path(Path::new(file_path))
            .map_err(|_| GitOperationError::FileNotFound {
                path: file_path.to_owned(),
                sha: commit.id().to_string(),
            })?;
    Ok(entry.id())
}

/// Checks if a file was deleted in a tree.
pub(super) fn is_file_deleted(new_tree: &git2::Tree<'_>, file_path: &str) -> bool {
    new_tree.get_path(Path::new(file_path)).is_err()
}

/// Checks if two commit OIDs are the same.
pub(super) fn are_commits_same(old_oid: Oid, new_oid: Oid) -> bool {
    old_oid == new_oid
}

/// Creates a diff for a specific file between two trees.
pub(super) fn create_file_diff<'a>(
    repo: &'a Repository,
    old_tree: &git2::Tree<'_>,
    new_tree: &git2::Tree<'_>,
    file_path: &str,
) -> Result<git2::Diff<'a>, GitOperationError> {
    let mut diff_opts = DiffOptions::new();
    diff_opts.pathspec(file_path);

    repo.diff_tree_to_tree(Some(old_tree), Some(new_tree), Some(&mut diff_opts))
        .map_err(|e| GitOperationError::DiffComputationFailed {
            message: e.message().to_owned(),
        })
}

/// Checks if a diff has no changes.
pub(super) fn has_no_changes(diff: &git2::Diff<'_>) -> bool {
    diff.deltas().next().is_none()
}

/// Checks if a line is within a hunk's old range.
pub(super) const fn is_line_in_hunk(line: u32, old_start: u32, old_lines: u32) -> bool {
    line >= old_start && line < old_start + old_lines
}

/// Checks if a line was deleted in a hunk.
pub(super) const fn is_line_deleted_in_hunk(
    line: u32,
    old_start: u32,
    old_lines: u32,
    new_lines: u32,
) -> bool {
    if old_lines > new_lines {
        let removed_start = old_start + new_lines;
        line >= removed_start
    } else {
        false
    }
}

/// Calculates the offset contribution from a hunk.
pub(super) fn calculate_hunk_offset(old_lines: u32, new_lines: u32) -> i32 {
    i32::try_from(new_lines).unwrap_or(0) - i32::try_from(old_lines).unwrap_or(0)
}

/// Computes the line offset by processing diff hunks.
pub(super) fn compute_line_offset_from_hunks(
    diff: &git2::Diff<'_>,
    target_line: u32,
) -> Result<(i32, bool), GitOperationError> {
    let mut line_offset: i32 = 0;
    let mut line_deleted = false;
    let mut passed_line = false;

    diff.foreach(
        &mut |_, _| true,
        None,
        Some(&mut |_delta, hunk| {
            let old_start = hunk.old_start();
            let old_lines = hunk.old_lines();
            let new_lines = hunk.new_lines();

            if passed_line {
                return true;
            }

            if is_line_in_hunk(target_line, old_start, old_lines) {
                line_deleted =
                    is_line_deleted_in_hunk(target_line, old_start, old_lines, new_lines);
                passed_line = true;
            } else if target_line >= old_start + old_lines {
                line_offset += calculate_hunk_offset(old_lines, new_lines);
            } else {
                passed_line = true;
            }

            true
        }),
        None,
    )
    .map_err(|e| GitOperationError::DiffComputationFailed {
        message: e.message().to_owned(),
    })?;

    Ok((line_offset, line_deleted))
}

/// Creates the appropriate line mapping result from offset and deletion state.
pub(super) fn create_line_mapping_result(
    original_line: u32,
    line_offset: i32,
    line_deleted: bool,
) -> LineMappingVerification {
    if line_deleted {
        return LineMappingVerification::deleted(original_line);
    }

    let new_line = u32::try_from(i32::try_from(original_line).unwrap_or(0) + line_offset)
        .unwrap_or(original_line);

    if new_line == original_line {
        LineMappingVerification::exact(original_line)
    } else {
        LineMappingVerification::moved(original_line, new_line)
    }
}
