//! Diff replay verification service implementation.

use std::sync::Arc;

use crate::github::models::ReviewComment;
use crate::local::{
    CommitSha, GitOperations, LineMappingRequest, LineMappingStatus, LineMappingVerification,
    RepoFilePath,
};

use super::model::{
    CommentVerificationEvidence, CommentVerificationEvidenceKind, CommentVerificationResult,
    CommentVerificationStatus,
};

#[derive(Debug, Clone, Copy)]
struct CommentAnchor<'a> {
    old_sha: &'a str,
    file_path: &'a str,
    old_line: u32,
}

#[derive(Debug, Clone, Copy)]
struct VerificationContext<'a> {
    comment_id: u64,
    target_sha: &'a str,
}

#[derive(Debug)]
struct LineComparison<'a> {
    comment_id: u64,
    target_sha: &'a str,
    mapping: &'a LineMappingVerification,
    old_line_number: u32,
    old_file: &'a str,
    new_file: &'a str,
}

/// Service interface for automated comment resolution verification.
pub trait ResolutionVerificationService: Send + Sync + std::fmt::Debug {
    /// Verifies a single review comment against `target_sha`.
    fn verify_comment(
        &self,
        comment: &ReviewComment,
        target_sha: &str,
    ) -> CommentVerificationResult;

    /// Verifies a sequence of comments against `target_sha`.
    fn verify_comments(
        &self,
        comments: &[ReviewComment],
        target_sha: &str,
    ) -> Vec<CommentVerificationResult> {
        comments
            .iter()
            .map(|comment| self.verify_comment(comment, target_sha))
            .collect()
    }
}

/// Default verification implementation using `GitOperations` diff replay.
#[derive(Debug, Clone)]
pub struct DiffReplayResolutionVerifier {
    git_ops: Arc<dyn GitOperations>,
}

impl DiffReplayResolutionVerifier {
    /// Creates a verifier backed by the supplied git operations.
    #[must_use]
    pub fn new(git_ops: Arc<dyn GitOperations>) -> Self {
        Self { git_ops }
    }

    fn unverified_result(
        comment_id: u64,
        target_sha: &str,
        kind: CommentVerificationEvidenceKind,
        message: Option<String>,
    ) -> CommentVerificationResult {
        Self::result_for(
            comment_id,
            target_sha,
            CommentVerificationStatus::Unverified,
            CommentVerificationEvidence { kind, message },
        )
    }

    fn verified_result(
        comment_id: u64,
        target_sha: &str,
        kind: CommentVerificationEvidenceKind,
        message: Option<String>,
    ) -> CommentVerificationResult {
        Self::result_for(
            comment_id,
            target_sha,
            CommentVerificationStatus::Verified,
            CommentVerificationEvidence { kind, message },
        )
    }

    fn result_for(
        comment_id: u64,
        target_sha: &str,
        status: CommentVerificationStatus,
        evidence: CommentVerificationEvidence,
    ) -> CommentVerificationResult {
        CommentVerificationResult::new(comment_id, target_sha.to_owned(), status, evidence)
    }

    fn anchor_for_comment<'a>(
        comment: &'a ReviewComment,
        target_sha: &str,
    ) -> Result<CommentAnchor<'a>, CommentVerificationResult> {
        let comment_id = comment.id;
        let Some(old_sha) = comment.commit_sha.as_deref() else {
            return Err(Self::unverified_result(
                comment_id,
                target_sha,
                CommentVerificationEvidenceKind::InsufficientMetadata,
                Some("missing commit SHA".to_owned()),
            ));
        };
        let Some(file_path) = comment.file_path.as_deref() else {
            return Err(Self::unverified_result(
                comment_id,
                target_sha,
                CommentVerificationEvidenceKind::InsufficientMetadata,
                Some("missing file path".to_owned()),
            ));
        };
        let old_line = comment
            .original_line_number
            .or(comment.line_number)
            .unwrap_or(0);
        if old_line == 0 {
            return Err(Self::unverified_result(
                comment_id,
                target_sha,
                CommentVerificationEvidenceKind::InsufficientMetadata,
                Some("missing line number".to_owned()),
            ));
        }

        Ok(CommentAnchor {
            old_sha,
            file_path,
            old_line,
        })
    }

    fn verify_line_mapping(
        &self,
        comment_id: u64,
        target_sha: &str,
        anchor: CommentAnchor<'_>,
    ) -> Result<LineMappingVerification, CommentVerificationResult> {
        let request = LineMappingRequest::new(
            anchor.old_sha.to_owned(),
            target_sha.to_owned(),
            anchor.file_path.to_owned(),
            anchor.old_line,
        );

        self.git_ops.verify_line_mapping(&request).map_err(|error| {
            Self::unverified_result(
                comment_id,
                target_sha,
                CommentVerificationEvidenceKind::RepositoryDataUnavailable,
                Some(error.to_string()),
            )
        })
    }

    fn get_file_at_commit(
        &self,
        ctx: VerificationContext<'_>,
        commit: &CommitSha,
        path: &RepoFilePath,
    ) -> Result<String, CommentVerificationResult> {
        self.git_ops
            .get_file_at_commit(commit, path)
            .map_err(|error| {
                Self::unverified_result(
                    ctx.comment_id,
                    ctx.target_sha,
                    CommentVerificationEvidenceKind::RepositoryDataUnavailable,
                    Some(error.to_string()),
                )
            })
    }
}

impl ResolutionVerificationService for DiffReplayResolutionVerifier {
    fn verify_comment(
        &self,
        comment: &ReviewComment,
        target_sha: &str,
    ) -> CommentVerificationResult {
        let comment_id = comment.id;
        let ctx = VerificationContext {
            comment_id,
            target_sha,
        };
        let anchor = match Self::anchor_for_comment(comment, target_sha) {
            Ok(anchor) => anchor,
            Err(result) => return result,
        };

        let mapping = match self.verify_line_mapping(comment_id, target_sha, anchor) {
            Ok(mapping) => mapping,
            Err(result) => return result,
        };

        match mapping.status() {
            LineMappingStatus::Deleted | LineMappingStatus::NotFound => {
                return Self::verified_result(
                    comment_id,
                    target_sha,
                    CommentVerificationEvidenceKind::LineRemoved,
                    Some(mapping.display()),
                );
            }
            LineMappingStatus::Exact | LineMappingStatus::Moved => {}
        }

        let path = RepoFilePath::new(anchor.file_path.to_owned());
        let old_commit = CommitSha::new(anchor.old_sha.to_owned());
        let new_commit = CommitSha::new(target_sha.to_owned());

        let old_file = match self.get_file_at_commit(ctx, &old_commit, &path) {
            Ok(content) => content,
            Err(result) => return result,
        };
        let new_file = match self.get_file_at_commit(ctx, &new_commit, &path) {
            Ok(content) => content,
            Err(result) => return result,
        };

        Self::classify_line_comparison(&LineComparison {
            comment_id,
            target_sha,
            mapping: &mapping,
            old_line_number: anchor.old_line,
            old_file: &old_file,
            new_file: &new_file,
        })
    }
}

impl DiffReplayResolutionVerifier {
    fn classify_line_comparison(comparison: &LineComparison<'_>) -> CommentVerificationResult {
        let Some(old_line) = line_at(comparison.old_file, comparison.old_line_number) else {
            return Self::unverified_result(
                comparison.comment_id,
                comparison.target_sha,
                CommentVerificationEvidenceKind::LineOutOfBounds,
                Some(format!(
                    "old commit line {} is out of bounds",
                    comparison.old_line_number
                )),
            );
        };

        let Some(new_line_number) = comparison.mapping.current_line() else {
            return Self::unverified_result(
                comparison.comment_id,
                comparison.target_sha,
                CommentVerificationEvidenceKind::RepositoryDataUnavailable,
                Some("line mapping did not produce a new line number".to_owned()),
            );
        };

        let Some(new_line) = line_at(comparison.new_file, new_line_number) else {
            return Self::unverified_result(
                comparison.comment_id,
                comparison.target_sha,
                CommentVerificationEvidenceKind::LineOutOfBounds,
                Some(format!(
                    "new commit line {new_line_number} is out of bounds"
                )),
            );
        };

        if normalise_line(old_line) == normalise_line(new_line) {
            Self::unverified_result(
                comparison.comment_id,
                comparison.target_sha,
                CommentVerificationEvidenceKind::LineUnchanged,
                Some(comparison.mapping.display()),
            )
        } else {
            Self::verified_result(
                comparison.comment_id,
                comparison.target_sha,
                CommentVerificationEvidenceKind::LineChanged,
                Some(comparison.mapping.display()),
            )
        }
    }
}

fn normalise_line(input: &str) -> &str {
    input.strip_suffix('\r').unwrap_or(input)
}

fn line_at(content: &str, line: u32) -> Option<&str> {
    let index = usize::try_from(line).ok()?.saturating_sub(1);
    content.lines().nth(index)
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;
