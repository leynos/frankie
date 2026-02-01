//! Tests for the Markdown comment export formatter.
//!
//! This module contains the test builder and tests for [`super::write_markdown`]
//! and related functions.

use rstest::rstest;

use super::*;

/// Test data constants to reduce string argument repetition.
mod test_data {
    /// Default PR URL for tests that don't need a specific URL.
    pub const DEFAULT_PR_URL: &str = "https://example.com/pr/1";
    /// A realistic GitHub PR URL for testing header output.
    pub const GITHUB_PR_URL: &str = "https://github.com/owner/repo/pull/123";
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
struct CommentBuilder {
    id: u64,
    author: Option<String>,
    file_path: Option<String>,
    line_number: Option<u32>,
    body: Option<String>,
    diff_hunk: Option<String>,
    created_at: Option<String>,
}

impl CommentBuilder {
    fn new(id: u64) -> Self {
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

    fn author(mut self, author: &str) -> Self {
        self.author = Some(author.to_owned());
        self
    }

    fn file_path(mut self, file_path: &str) -> Self {
        self.file_path = Some(file_path.to_owned());
        self
    }

    fn line_number(mut self, line_number: u32) -> Self {
        self.line_number = Some(line_number);
        self
    }

    fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_owned());
        self
    }

    fn diff_hunk(mut self, diff_hunk: &str) -> Self {
        self.diff_hunk = Some(diff_hunk.to_owned());
        self
    }

    fn created_at(mut self, created_at: &str) -> Self {
        self.created_at = Some(created_at.to_owned());
        self
    }

    fn build(self) -> ExportedComment {
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

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn write_markdown_to_string(
    comments: &[ExportedComment],
    pr_url: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = Vec::new();
    write_markdown(&mut buffer, comments, pr_url)?;
    Ok(String::from_utf8(buffer)?)
}

fn assert_contains(haystack: &str, needle: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!(
            "expected output to contain '{needle}', got:\n{haystack}"
        ))
    }
}

fn assert_not_contains(haystack: &str, needle: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Err(format!(
            "expected output to NOT contain '{needle}', got:\n{haystack}"
        ))
    } else {
        Ok(())
    }
}

fn assert_eq_count(actual: usize, expected: usize, description: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{description}: expected {expected}, got {actual}"))
    }
}

fn assert_single_comment_output_contains(
    comment: ExportedComment,
    expected_substring: &str,
) -> TestResult {
    let comments = vec![comment];
    let output = write_markdown_to_string(&comments, test_data::DEFAULT_PR_URL)?;
    assert_contains(&output, expected_substring)?;
    Ok(())
}

#[rstest]
fn writes_header_with_pr_url() -> TestResult {
    let comments: Vec<ExportedComment> = vec![];

    let output = write_markdown_to_string(&comments, test_data::GITHUB_PR_URL)?;

    assert_contains(&output, "# Review Comments Export")?;
    assert_contains(&output, &format!("PR: {}", test_data::GITHUB_PR_URL))?;
    Ok(())
}

#[rstest]
fn writes_comment_with_all_fields() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1)
            .author(test_data::SAMPLE_AUTHOR)
            .file_path(test_data::SAMPLE_FILE_PATH)
            .line_number(42)
            .body(test_data::SAMPLE_BODY)
            .diff_hunk(test_data::SAMPLE_DIFF_HUNK)
            .created_at(test_data::SAMPLE_TIMESTAMP)
            .build(),
    ];

    let output = write_markdown_to_string(&comments, test_data::DEFAULT_PR_URL)?;

    assert_contains(&output, &format!("## {}:42", test_data::SAMPLE_FILE_PATH))?;
    assert_contains(
        &output,
        &format!("**Reviewer:** {}", test_data::SAMPLE_AUTHOR),
    )?;
    assert_contains(
        &output,
        &format!("**Created:** {}", test_data::SAMPLE_TIMESTAMP),
    )?;
    assert_contains(&output, test_data::SAMPLE_BODY)?;
    assert_contains(&output, "```rust")?;
    assert_contains(&output, "@@ -40,3 +40,5 @@")?;
    Ok(())
}

#[rstest]
fn handles_missing_file_path() -> TestResult {
    let comment = CommentBuilder::new(1)
        .author("bob")
        .line_number(10)
        .body("Fix this")
        .build();

    assert_single_comment_output_contains(comment, "## (unknown file):10")
}

#[rstest]
fn handles_missing_line_number() -> TestResult {
    let comment = CommentBuilder::new(1)
        .author("charlie")
        .file_path("README.md")
        .body("Update docs")
        .build();

    assert_single_comment_output_contains(comment, "## README.md")
}

#[rstest]
fn handles_completely_missing_location() -> TestResult {
    let comment = CommentBuilder::new(1).body("General comment").build();

    assert_single_comment_output_contains(comment, "## (unknown location)")
}

#[rstest]
fn empty_comments_produces_header_only() -> TestResult {
    let comments: Vec<ExportedComment> = vec![];

    let output = write_markdown_to_string(&comments, test_data::DEFAULT_PR_URL)?;

    assert_contains(&output, "# Review Comments Export")?;
    assert_not_contains(&output, "---")?; // No comment separators
    Ok(())
}

#[rstest]
#[case("rs", "rust")]
#[case("py", "python")]
#[case("js", "javascript")]
#[case("ts", "typescript")]
#[case("go", "go")]
#[case("java", "java")]
#[case("unknown", "diff")]
fn extension_maps_to_language(#[case] ext: &str, #[case] expected: &str) {
    assert_eq!(extension_to_language(ext), expected);
}

#[rstest]
fn uses_diff_language_for_unknown_extension() -> TestResult {
    let comment = CommentBuilder::new(1)
        .file_path("config.unknown")
        .line_number(1)
        .diff_hunk("some code")
        .build();

    assert_single_comment_output_contains(comment, "```diff")
}

#[rstest]
fn uses_diff_language_when_no_file_path() -> TestResult {
    let comment = CommentBuilder::new(1).diff_hunk("+ added line").build();

    assert_single_comment_output_contains(comment, "```diff")
}

#[rstest]
fn multiple_comments_have_separators() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1)
            .author("alice")
            .file_path("a.rs")
            .line_number(1)
            .body("First")
            .build(),
        CommentBuilder::new(2)
            .author("bob")
            .file_path("b.rs")
            .line_number(2)
            .body("Second")
            .build(),
    ];

    let output = write_markdown_to_string(&comments, test_data::DEFAULT_PR_URL)?;

    let separator_count = output.matches("---").count();
    assert_eq_count(separator_count, 2, "separator count")?; // One per comment
    Ok(())
}
