//! Shared test utilities for export tests.
//!
//! This module provides common test helpers used across export-related test
//! modules to reduce duplication and ensure consistent testing patterns.

use std::fmt;

use super::ExportedComment;

// Re-export PrUrl from model for test convenience.
pub use super::PrUrl;

/// Error type for test assertions that implements `std::error::Error`.
///
/// This allows test helpers to return errors compatible with `?` operator
/// in test functions returning `Result<(), Box<dyn std::error::Error>>`.
#[derive(Debug)]
pub struct TestError(String);

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TestError {}

impl From<String> for TestError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Test data constants to reduce string argument repetition.
pub mod test_data {
    use super::PrUrl;

    /// Default PR URL for tests that don't need a specific URL.
    pub const DEFAULT_PR_URL: PrUrl<'static> = PrUrl::new("https://example.com/pr/1");
    /// A realistic GitHub PR URL for testing header output.
    pub const GITHUB_PR_URL: PrUrl<'static> = PrUrl::new("https://github.com/owner/repo/pull/123");
    /// Sample author name for comprehensive tests.
    pub const SAMPLE_AUTHOR: &str = "alice";
    /// Sample file path for comprehensive tests.
    pub const SAMPLE_FILE_PATH: &str = "src/lib.rs";
    /// Sample comment body for comprehensive tests.
    pub const SAMPLE_BODY: &str = "Consider using a constant here.";
    /// Sample diff hunk for comprehensive tests.
    pub const SAMPLE_DIFF_HUNK: &str = "@@ -40,3 +40,5 @@\n let x = 1;";
    /// Sample timestamp for comprehensive tests.
    pub const SAMPLE_TIMESTAMP: &str = "2025-01-15T10:00:00Z";
}

/// Builder for creating test [`ExportedComment`] instances with a fluent API.
#[must_use]
pub struct CommentBuilder {
    id: u64,
    author: Option<String>,
    file_path: Option<String>,
    line_number: Option<u32>,
    original_line_number: Option<u32>,
    body: Option<String>,
    diff_hunk: Option<String>,
    commit_sha: Option<String>,
    in_reply_to_id: Option<u64>,
    created_at: Option<String>,
}

impl CommentBuilder {
    /// Creates a new builder with the given comment ID.
    pub const fn new(id: u64) -> Self {
        Self {
            id,
            author: None,
            file_path: None,
            line_number: None,
            original_line_number: None,
            body: None,
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
        }
    }

    /// Sets the comment author.
    pub fn author(mut self, author: &str) -> Self {
        self.author = Some(author.to_owned());
        self
    }

    /// Sets the file path.
    pub fn file_path(mut self, file_path: &str) -> Self {
        self.file_path = Some(file_path.to_owned());
        self
    }

    /// Sets the line number.
    pub const fn line_number(mut self, line_number: u32) -> Self {
        self.line_number = Some(line_number);
        self
    }

    /// Sets the comment body.
    pub fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_owned());
        self
    }

    /// Sets the diff hunk.
    pub fn diff_hunk(mut self, diff_hunk: &str) -> Self {
        self.diff_hunk = Some(diff_hunk.to_owned());
        self
    }

    /// Sets the creation timestamp.
    pub fn created_at(mut self, created_at: &str) -> Self {
        self.created_at = Some(created_at.to_owned());
        self
    }

    /// Sets the original line number before changes.
    pub const fn original_line_number(mut self, original_line_number: u32) -> Self {
        self.original_line_number = Some(original_line_number);
        self
    }

    /// Sets the commit SHA this comment was made against.
    pub fn commit_sha(mut self, commit_sha: &str) -> Self {
        self.commit_sha = Some(commit_sha.to_owned());
        self
    }

    /// Sets the ID of the parent comment if this is a reply.
    pub const fn in_reply_to_id(mut self, in_reply_to_id: u64) -> Self {
        self.in_reply_to_id = Some(in_reply_to_id);
        self
    }

    /// Sets the author to [`test_data::SAMPLE_AUTHOR`].
    pub fn with_sample_author(self) -> Self {
        self.author(test_data::SAMPLE_AUTHOR)
    }

    /// Sets the file path to [`test_data::SAMPLE_FILE_PATH`].
    pub fn with_sample_file_path(self) -> Self {
        self.file_path(test_data::SAMPLE_FILE_PATH)
    }

    /// Sets the body to [`test_data::SAMPLE_BODY`].
    pub fn with_sample_body(self) -> Self {
        self.body(test_data::SAMPLE_BODY)
    }

    /// Sets the diff hunk to [`test_data::SAMPLE_DIFF_HUNK`].
    pub fn with_sample_diff_hunk(self) -> Self {
        self.diff_hunk(test_data::SAMPLE_DIFF_HUNK)
    }

    /// Sets the creation timestamp to [`test_data::SAMPLE_TIMESTAMP`].
    pub fn with_sample_created_at(self) -> Self {
        self.created_at(test_data::SAMPLE_TIMESTAMP)
    }

    /// Builds the [`ExportedComment`] with configured values.
    #[must_use]
    pub fn build(self) -> ExportedComment {
        ExportedComment {
            id: self.id,
            author: self.author,
            file_path: self.file_path,
            line_number: self.line_number,
            original_line_number: self.original_line_number,
            body: self.body,
            diff_hunk: self.diff_hunk,
            commit_sha: self.commit_sha,
            in_reply_to_id: self.in_reply_to_id,
            created_at: self.created_at,
        }
    }
}

/// Asserts that `haystack` contains `needle`, returning an error if not.
///
/// # Errors
///
/// Returns a [`TestError`] if the `needle` is not found in `haystack`.
pub fn assert_contains(haystack: &str, needle: &str) -> Result<(), TestError> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!("expected output to contain '{needle}', got:\n{haystack}").into())
    }
}

/// Asserts that `haystack` does NOT contain `needle`, returning an error if it does.
///
/// # Errors
///
/// Returns a [`TestError`] if `needle` is found in `haystack`.
pub fn assert_not_contains(haystack: &str, needle: &str) -> Result<(), TestError> {
    if haystack.contains(needle) {
        Err(format!("expected output to NOT contain '{needle}', got:\n{haystack}").into())
    } else {
        Ok(())
    }
}
