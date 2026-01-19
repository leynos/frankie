//! Mock implementation of `GitOperations` for BDD tests.

use frankie::local::{
    CommitMetadata, CommitSha, CommitSnapshot, GitOperationError, GitOperations,
    LineMappingRequest, LineMappingVerification, RepoFilePath,
};

/// Mock implementation of `GitOperations` for testing.
#[derive(Debug)]
pub(crate) struct MockGitOperations {
    /// Whether the commit should be found.
    commit_exists: bool,
    /// The commit history to return.
    commit_history: Vec<CommitSha>,
}

impl Default for MockGitOperations {
    fn default() -> Self {
        Self {
            commit_exists: true,
            commit_history: vec![
                CommitSha::new("abc1234567890".to_owned()),
                CommitSha::new("def5678901234".to_owned()),
                CommitSha::new("ghi9012345678".to_owned()),
            ],
        }
    }
}

impl MockGitOperations {
    /// Creates a new mock with default settings.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Sets whether the commit should be found.
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Vec fields prevent const evaluation"
    )]
    pub(crate) fn with_commit_exists(mut self, exists: bool) -> Self {
        self.commit_exists = exists;
        self
    }
}

impl GitOperations for MockGitOperations {
    fn get_commit_snapshot(
        &self,
        sha: &CommitSha,
        file_path: Option<&RepoFilePath>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        if !self.commit_exists {
            return Err(GitOperationError::CommitNotFound {
                sha: sha.to_string(),
            });
        }

        let timestamp = chrono::Utc::now();
        let metadata = CommitMetadata::new(
            sha.to_string(),
            "Fix login validation".to_owned(),
            "Alice".to_owned(),
            timestamp,
        );
        if let Some(path) = file_path {
            Ok(CommitSnapshot::with_file_content(
                metadata,
                path.to_string(),
                "fn login() {\n    // validation\n}".to_owned(),
            ))
        } else {
            Ok(CommitSnapshot::new(metadata))
        }
    }

    fn get_file_at_commit(
        &self,
        sha: &CommitSha,
        _file_path: &RepoFilePath,
    ) -> Result<String, GitOperationError> {
        if !self.commit_exists {
            return Err(GitOperationError::CommitNotFound {
                sha: sha.to_string(),
            });
        }
        Ok("fn login() {\n    // validation\n}".to_owned())
    }

    fn verify_line_mapping(
        &self,
        request: &LineMappingRequest,
    ) -> Result<LineMappingVerification, GitOperationError> {
        Ok(LineMappingVerification::exact(request.line))
    }

    fn get_parent_commits(
        &self,
        _sha: &CommitSha,
        limit: usize,
    ) -> Result<Vec<CommitSha>, GitOperationError> {
        Ok(self.commit_history.iter().take(limit).cloned().collect())
    }

    fn commit_exists(&self, _sha: &CommitSha) -> bool {
        self.commit_exists
    }
}
