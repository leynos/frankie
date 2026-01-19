//! Mock implementation of `GitOperations` for BDD tests.

use frankie::local::{CommitSnapshot, GitOperationError, GitOperations, LineMappingVerification};

/// Mock implementation of `GitOperations` for testing.
#[derive(Debug)]
pub(crate) struct MockGitOperations {
    /// Whether the commit should be found.
    commit_exists: bool,
    /// The commit history to return.
    commit_history: Vec<String>,
}

impl Default for MockGitOperations {
    fn default() -> Self {
        Self {
            commit_exists: true,
            commit_history: vec![
                "abc1234567890".to_owned(),
                "def5678901234".to_owned(),
                "ghi9012345678".to_owned(),
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
        sha: &str,
        file_path: Option<&str>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        if !self.commit_exists {
            return Err(GitOperationError::CommitNotFound {
                sha: sha.to_owned(),
            });
        }

        let timestamp = chrono::Utc::now();
        file_path.map_or_else(
            || {
                Ok(CommitSnapshot::new(
                    sha.to_owned(),
                    "Fix login validation".to_owned(),
                    "Alice".to_owned(),
                    timestamp,
                ))
            },
            |path| {
                Ok(CommitSnapshot::with_file_content(
                    sha.to_owned(),
                    "Fix login validation".to_owned(),
                    "Alice".to_owned(),
                    timestamp,
                    path.to_owned(),
                    "fn login() {\n    // validation\n}".to_owned(),
                ))
            },
        )
    }

    fn get_file_at_commit(&self, sha: &str, _file_path: &str) -> Result<String, GitOperationError> {
        if !self.commit_exists {
            return Err(GitOperationError::CommitNotFound {
                sha: sha.to_owned(),
            });
        }
        Ok("fn login() {\n    // validation\n}".to_owned())
    }

    fn verify_line_mapping(
        &self,
        _old_sha: &str,
        _new_sha: &str,
        _file_path: &str,
        line: u32,
    ) -> Result<LineMappingVerification, GitOperationError> {
        Ok(LineMappingVerification::exact(line))
    }

    fn get_parent_commits(
        &self,
        _sha: &str,
        limit: usize,
    ) -> Result<Vec<String>, GitOperationError> {
        Ok(self.commit_history.iter().take(limit).cloned().collect())
    }

    fn commit_exists(&self, _sha: &str) -> bool {
        self.commit_exists
    }
}
