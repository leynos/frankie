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
    /// Optional configured line mapping response. If None, defaults to exact match.
    line_mapping_response: Option<LineMappingVerification>,
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
            line_mapping_response: None,
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

    /// Sets the line mapping verification response.
    ///
    /// Use this to test different line mapping scenarios:
    /// - `exact(line)` for unchanged lines
    /// - `moved(original, new)` for lines that moved positions
    /// - `deleted(line)` for lines that were removed
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Vec fields prevent const evaluation"
    )]
    pub(crate) fn with_line_mapping(mut self, mapping: LineMappingVerification) -> Self {
        self.line_mapping_response = Some(mapping);
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
        // Use configured response if available, otherwise default to exact match
        Ok(self
            .line_mapping_response
            .clone()
            .unwrap_or_else(|| LineMappingVerification::exact(request.line)))
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
