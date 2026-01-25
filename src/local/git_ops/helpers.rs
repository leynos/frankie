//! Helper functions for Git operations, particularly diff analysis.

use std::path::Path;

use git2::{DiffOptions, Oid, Repository};

use crate::local::commit::LineMappingVerification;
use crate::local::error::GitOperationError;

/// Represents a range of lines in a diff hunk.
///
/// Encapsulates the old file line range and new file line count from a diff hunk,
/// providing semantic operations for line mapping calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HunkRange {
    /// Starting line number in the old file (1-indexed).
    pub start: u32,
    /// Number of lines in the old file's hunk.
    pub old_lines: u32,
    /// Number of lines in the new file's hunk.
    pub new_lines: u32,
}

impl HunkRange {
    /// Constructs a `HunkRange` from a git2 diff hunk.
    pub fn from_hunk(hunk: &git2::DiffHunk<'_>) -> Self {
        Self {
            start: hunk.old_start(),
            old_lines: hunk.old_lines(),
            new_lines: hunk.new_lines(),
        }
    }

    /// Checks if a line number is within this hunk's old range.
    pub const fn contains_line(self, line: u32) -> bool {
        line >= self.start && line < self.end_line()
    }

    /// Checks if a line was deleted in this hunk.
    ///
    /// A line is considered deleted if the hunk removed more lines than it added
    /// and the line falls within the removed section.
    pub const fn is_line_deleted(self, line: u32) -> bool {
        if self.old_lines > self.new_lines {
            let removed_start = self.start + self.new_lines;
            line >= removed_start
        } else {
            false
        }
    }

    /// Calculates the line offset contribution from this hunk.
    ///
    /// Returns `new_lines - old_lines` as a signed offset. Values exceeding
    /// `i32::MAX` are treated as zero, though this is unreachable in practice
    /// since diff hunks cannot contain billions of lines.
    pub fn offset(self) -> i32 {
        i32::try_from(self.new_lines).unwrap_or(0) - i32::try_from(self.old_lines).unwrap_or(0)
    }

    /// Returns the end line number (exclusive) of the old range.
    pub const fn end_line(self) -> u32 {
        self.start + self.old_lines
    }
}

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
            sha: sha.to_owned().into(),
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
                path: file_path.to_owned().into(),
                sha: commit.id().to_string().into(),
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
            let range = HunkRange::from_hunk(&hunk);

            if passed_line {
                return true;
            }

            if range.contains_line(target_line) {
                line_deleted = range.is_line_deleted(target_line);
                passed_line = true;
            } else if target_line >= range.end_line() {
                line_offset += range.offset();
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
