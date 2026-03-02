//! Unit tests for diff replay resolution verification.

use std::collections::HashMap;

use rstest::rstest;

use crate::github::models::ReviewComment;
use crate::local::{
    CommitMetadata, CommitSha, CommitSnapshot, GitOperationError, GitOperations,
    LineMappingRequest, LineMappingVerification, RepoFilePath,
};

use super::{DiffReplayResolutionVerifier, ResolutionVerificationService};
use crate::verification::{CommentVerificationEvidenceKind, CommentVerificationStatus};

#[derive(Debug)]
struct StubGitOperations {
    line_mapping: LineMappingVerification,
    files: HashMap<(String, String), String>,
}

impl StubGitOperations {
    fn new(line_mapping: LineMappingVerification) -> Self {
        Self {
            line_mapping,
            files: HashMap::new(),
        }
    }

    fn with_file(mut self, sha: &str, path: &str, content: &str) -> Self {
        self.files
            .insert((sha.to_owned(), path.to_owned()), content.to_owned());
        self
    }
}

impl GitOperations for StubGitOperations {
    fn get_commit_snapshot(
        &self,
        sha: &CommitSha,
        file_path: Option<&RepoFilePath>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        let timestamp = chrono::Utc::now();
        let metadata = CommitMetadata::new(
            sha.to_string(),
            "message".to_owned(),
            "author".to_owned(),
            timestamp,
        );
        if let Some(path) = file_path {
            let content = self
                .files
                .get(&(sha.as_str().to_owned(), path.as_str().to_owned()))
                .cloned()
                .unwrap_or_default();
            Ok(CommitSnapshot::with_file_content(
                metadata,
                path.as_str().to_owned(),
                content,
            ))
        } else {
            Ok(CommitSnapshot::new(metadata))
        }
    }

    fn get_file_at_commit(
        &self,
        sha: &CommitSha,
        file_path: &RepoFilePath,
    ) -> Result<String, GitOperationError> {
        self.files
            .get(&(sha.as_str().to_owned(), file_path.as_str().to_owned()))
            .cloned()
            .ok_or_else(|| GitOperationError::FileNotFound {
                sha: sha.clone(),
                path: file_path.clone(),
            })
    }

    fn verify_line_mapping(
        &self,
        request: &LineMappingRequest,
    ) -> Result<LineMappingVerification, GitOperationError> {
        Ok(match self.line_mapping.status() {
            crate::local::LineMappingStatus::Exact => LineMappingVerification::exact(request.line),
            crate::local::LineMappingStatus::Moved => self.line_mapping.clone(),
            crate::local::LineMappingStatus::Deleted => {
                LineMappingVerification::deleted(request.line)
            }
            crate::local::LineMappingStatus::NotFound => {
                LineMappingVerification::not_found(request.line)
            }
        })
    }

    fn get_parent_commits(
        &self,
        _sha: &CommitSha,
        _limit: usize,
    ) -> Result<Vec<CommitSha>, GitOperationError> {
        Ok(Vec::new())
    }

    fn commit_exists(&self, _sha: &CommitSha) -> bool {
        true
    }
}

fn base_comment() -> ReviewComment {
    ReviewComment {
        id: 1,
        body: Some("body".to_owned()),
        author: Some("alice".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(2),
        original_line_number: None,
        diff_hunk: None,
        commit_sha: Some("old".to_owned()),
        in_reply_to_id: None,
        created_at: None,
        updated_at: None,
    }
}

#[test]
fn missing_metadata_returns_unverified() {
    let git_ops = StubGitOperations::new(LineMappingVerification::exact(2));
    let verifier = DiffReplayResolutionVerifier::new(std::sync::Arc::new(git_ops));
    let comment = ReviewComment {
        commit_sha: None,
        ..base_comment()
    };
    let result = verifier.verify_comment(&comment, "head");
    assert_eq!(result.status(), CommentVerificationStatus::Unverified);
    assert_eq!(
        result.evidence().kind,
        CommentVerificationEvidenceKind::InsufficientMetadata
    );
}

#[test]
fn deleted_line_is_verified() {
    let git_ops = StubGitOperations::new(LineMappingVerification::deleted(2));
    let verifier = DiffReplayResolutionVerifier::new(std::sync::Arc::new(git_ops));
    let result = verifier.verify_comment(&base_comment(), "head");
    assert_eq!(result.status(), CommentVerificationStatus::Verified);
    assert_eq!(
        result.evidence().kind,
        CommentVerificationEvidenceKind::LineRemoved
    );
}

#[rstest]
#[case::changed(
    "let x = 1;\n",
    "let x = 2;\n",
    CommentVerificationStatus::Verified,
    CommentVerificationEvidenceKind::LineChanged
)]
#[case::unchanged(
    "let x = 1;\n",
    "let x = 1;\n",
    CommentVerificationStatus::Unverified,
    CommentVerificationEvidenceKind::LineUnchanged
)]
fn exact_mapping_compares_line_content(
    #[case] old_content: &str,
    #[case] new_content: &str,
    #[case] expected_status: CommentVerificationStatus,
    #[case] expected_kind: CommentVerificationEvidenceKind,
) {
    let git_ops = StubGitOperations::new(LineMappingVerification::exact(2))
        .with_file(
            "old",
            "src/main.rs",
            &format!("fn main() {{\n{old_content}}}\n"),
        )
        .with_file(
            "head",
            "src/main.rs",
            &format!("fn main() {{\n{new_content}}}\n"),
        );
    let verifier = DiffReplayResolutionVerifier::new(std::sync::Arc::new(git_ops));
    let result = verifier.verify_comment(&base_comment(), "head");
    assert_eq!(result.status(), expected_status);
    assert_eq!(result.evidence().kind, expected_kind);
}

#[test]
fn out_of_bounds_line_is_unverified() {
    let git_ops = StubGitOperations::new(LineMappingVerification::exact(200))
        .with_file("old", "src/main.rs", "only one line\n")
        .with_file("head", "src/main.rs", "only one line\n");
    let verifier = DiffReplayResolutionVerifier::new(std::sync::Arc::new(git_ops));
    let result = verifier.verify_comment(&base_comment(), "head");
    assert_eq!(result.status(), CommentVerificationStatus::Unverified);
    assert_eq!(
        result.evidence().kind,
        CommentVerificationEvidenceKind::LineOutOfBounds
    );
}
