//! Shared test utilities for export tests.
//!
//! This module provides common test helpers used across export-related test
//! modules to reduce duplication and ensure consistent testing patterns.

use super::ExportedComment;

/// A newtype wrapper for pull request URLs in tests.
///
/// Provides semantic typing for PR URL parameters to reduce string argument
/// ratio and improve type safety.
#[derive(Debug, Clone, Copy)]
pub struct PrUrl<'a>(&'a str);

impl<'a> PrUrl<'a> {
    /// Creates a new `PrUrl` from a string slice.
    #[must_use]
    pub const fn new(url: &'a str) -> Self {
        Self(url)
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
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
    body: Option<String>,
    diff_hunk: Option<String>,
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
            body: None,
            diff_hunk: None,
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

    /// Builds the [`ExportedComment`] with configured values.
    #[must_use]
    pub fn build(self) -> ExportedComment {
        ExportedComment {
            id: self.id,
            author: self.author,
            file_path: self.file_path,
            line_number: self.line_number,
            original_line_number: None,
            body: self.body,
            diff_hunk: self.diff_hunk,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: self.created_at,
        }
    }
}

/// Asserts that `haystack` contains `needle`, returning an error if not.
pub fn assert_contains(haystack: &str, needle: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!(
            "expected output to contain '{needle}', got:\n{haystack}"
        ))
    }
}

/// Asserts that `haystack` does NOT contain `needle`, returning an error if it does.
pub fn assert_not_contains(haystack: &str, needle: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Err(format!(
            "expected output to NOT contain '{needle}', got:\n{haystack}"
        ))
    } else {
        Ok(())
    }
}
