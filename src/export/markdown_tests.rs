//! Tests for the Markdown comment export formatter.
//!
//! This module contains the test builder and tests for [`super::write_markdown`]
//! and related functions.

use rstest::rstest;

use super::*;
use crate::export::test_helpers::{
    CommentBuilder, PrUrl, assert_contains, assert_not_contains, test_data,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn write_markdown_to_string(
    comments: &[ExportedComment],
    pr_url: PrUrl<'_>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = Vec::new();
    write_markdown(&mut buffer, comments, pr_url.as_str())?;
    Ok(String::from_utf8(buffer)?)
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
    assert_contains(
        &output,
        &format!("PR: {}", test_data::GITHUB_PR_URL.as_str()),
    )?;
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
    if separator_count != 2 {
        return Err(format!("expected 2 separators, got {separator_count}").into());
    }
    Ok(())
}
