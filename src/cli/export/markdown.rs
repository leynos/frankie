//! Markdown formatter for exported comments.
//!
//! Generates human-readable Markdown output with syntax-highlighted code
//! blocks for diff context.

use std::io::Write;
use std::path::Path;

use frankie::IntakeError;

use super::model::ExportedComment;

/// Writes comments in Markdown format to the given writer.
///
/// The output includes a header with the PR URL, followed by each comment
/// with its file location, author, timestamp, body text, and code context
/// in a fenced code block.
///
/// # Errors
///
/// Returns [`IntakeError::Io`] if writing to the output fails.
pub fn write_markdown<W: Write>(
    writer: &mut W,
    comments: &[ExportedComment],
    pr_url: &str,
) -> Result<(), IntakeError> {
    write_header(writer, pr_url)?;

    for comment in comments {
        write_comment_section(writer, comment)?;
    }

    Ok(())
}

/// Writes the Markdown header with PR URL.
fn write_header<W: Write>(writer: &mut W, pr_url: &str) -> Result<(), IntakeError> {
    writeln!(writer, "# Review Comments Export").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;
    writeln!(writer, "PR: {pr_url}").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;
    Ok(())
}

/// Writes a single comment section.
fn write_comment_section<W: Write>(
    writer: &mut W,
    comment: &ExportedComment,
) -> Result<(), IntakeError> {
    writeln!(writer, "---").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;

    // File location heading
    write_location_heading(writer, comment)?;

    // Metadata (author and timestamp)
    write_metadata(writer, comment)?;

    // Comment body
    if let Some(body) = &comment.body {
        writeln!(writer).map_err(|e| io_error(&e))?;
        writeln!(writer, "{body}").map_err(|e| io_error(&e))?;
    }

    // Code context
    if let Some(diff_hunk) = &comment.diff_hunk {
        write_code_block(writer, comment.file_path.as_deref(), diff_hunk)?;
    }

    writeln!(writer).map_err(|e| io_error(&e))?;
    Ok(())
}

/// Writes the file location heading.
fn write_location_heading<W: Write>(
    writer: &mut W,
    comment: &ExportedComment,
) -> Result<(), IntakeError> {
    let location = match (&comment.file_path, comment.line_number) {
        (Some(path), Some(line)) => format!("{path}:{line}"),
        (Some(path), None) => path.clone(),
        (None, Some(line)) => format!("(unknown file):{line}"),
        (None, None) => "(unknown location)".to_owned(),
    };
    writeln!(writer, "## {location}").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;
    Ok(())
}

/// Writes comment metadata (author and timestamp).
fn write_metadata<W: Write>(writer: &mut W, comment: &ExportedComment) -> Result<(), IntakeError> {
    if let Some(author) = &comment.author {
        writeln!(writer, "**Reviewer:** {author}").map_err(|e| io_error(&e))?;
    }
    if let Some(created_at) = &comment.created_at {
        writeln!(writer, "**Created:** {created_at}").map_err(|e| io_error(&e))?;
    }
    Ok(())
}

/// Writes a fenced code block with language hint from file extension.
fn write_code_block<W: Write>(
    writer: &mut W,
    file_path: Option<&str>,
    diff_hunk: &str,
) -> Result<(), IntakeError> {
    let language = file_path
        .and_then(|p| Path::new(p).extension())
        .and_then(|ext| ext.to_str())
        .map_or("diff", extension_to_language);

    writeln!(writer).map_err(|e| io_error(&e))?;
    writeln!(writer, "```{language}").map_err(|e| io_error(&e))?;
    writeln!(writer, "{diff_hunk}").map_err(|e| io_error(&e))?;
    writeln!(writer, "```").map_err(|e| io_error(&e))?;
    Ok(())
}

/// Maps file extensions to Markdown code block language hints.
fn extension_to_language(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "jsx" => "jsx",
        "tsx" => "tsx",
        "rb" => "ruby",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" => "c",
        "cpp" | "cc" | "cxx" | "h" | "hpp" => "cpp",
        "cs" => "csharp",
        "php" => "php",
        "sh" | "bash" => "bash",
        "zsh" => "zsh",
        "fish" => "fish",
        "ps1" => "powershell",
        "sql" => "sql",
        "md" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "less" => "less",
        _ => "diff",
    }
}

/// Converts an I/O error to an [`IntakeError::Io`].
fn io_error(error: &std::io::Error) -> IntakeError {
    IntakeError::Io {
        message: error.to_string(),
    }
}

#[cfg(test)]
#[expect(
    clippy::too_many_arguments,
    reason = "test helper mirrors ExportedComment structure"
)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn make_comment(
        id: u64,
        author: Option<&str>,
        file_path: Option<&str>,
        line_number: Option<u32>,
        body: Option<&str>,
        diff_hunk: Option<&str>,
        created_at: Option<&str>,
    ) -> ExportedComment {
        ExportedComment {
            id,
            author: author.map(String::from),
            file_path: file_path.map(String::from),
            line_number,
            original_line_number: None,
            body: body.map(String::from),
            diff_hunk: diff_hunk.map(String::from),
            commit_sha: None,
            in_reply_to_id: None,
            created_at: created_at.map(String::from),
        }
    }

    #[rstest]
    fn writes_header_with_pr_url() {
        let mut buffer = Vec::new();
        let comments: Vec<ExportedComment> = vec![];

        write_markdown(
            &mut buffer,
            &comments,
            "https://github.com/owner/repo/pull/123",
        )
        .expect("should write markdown");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        assert!(output.contains("# Review Comments Export"));
        assert!(output.contains("PR: https://github.com/owner/repo/pull/123"));
    }

    #[rstest]
    fn writes_comment_with_all_fields() {
        let mut buffer = Vec::new();
        let comments = vec![make_comment(
            1,
            Some("alice"),
            Some("src/lib.rs"),
            Some(42),
            Some("Consider using a constant here."),
            Some("@@ -40,3 +40,5 @@\n let x = 1;"),
            Some("2025-01-15T10:00:00Z"),
        )];

        write_markdown(&mut buffer, &comments, "https://example.com/pr/1")
            .expect("should write markdown");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        assert!(output.contains("## src/lib.rs:42"));
        assert!(output.contains("**Reviewer:** alice"));
        assert!(output.contains("**Created:** 2025-01-15T10:00:00Z"));
        assert!(output.contains("Consider using a constant here."));
        assert!(output.contains("```rust"));
        assert!(output.contains("@@ -40,3 +40,5 @@"));
    }

    #[rstest]
    fn handles_missing_file_path() {
        let mut buffer = Vec::new();
        let comments = vec![make_comment(
            1,
            Some("bob"),
            None,
            Some(10),
            Some("Fix this"),
            None,
            None,
        )];

        write_markdown(&mut buffer, &comments, "https://example.com/pr/1")
            .expect("should write markdown");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        assert!(output.contains("## (unknown file):10"));
    }

    #[rstest]
    fn handles_missing_line_number() {
        let mut buffer = Vec::new();
        let comments = vec![make_comment(
            1,
            Some("charlie"),
            Some("README.md"),
            None,
            Some("Update docs"),
            None,
            None,
        )];

        write_markdown(&mut buffer, &comments, "https://example.com/pr/1")
            .expect("should write markdown");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        assert!(output.contains("## README.md"));
        // Should not have a colon and line number
        assert!(!output.contains("README.md:"));
    }

    #[rstest]
    fn handles_completely_missing_location() {
        let mut buffer = Vec::new();
        let comments = vec![make_comment(
            1,
            None,
            None,
            None,
            Some("General comment"),
            None,
            None,
        )];

        write_markdown(&mut buffer, &comments, "https://example.com/pr/1")
            .expect("should write markdown");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        assert!(output.contains("## (unknown location)"));
    }

    #[rstest]
    fn empty_comments_produces_header_only() {
        let mut buffer = Vec::new();
        let comments: Vec<ExportedComment> = vec![];

        write_markdown(&mut buffer, &comments, "https://example.com/pr/1")
            .expect("should write markdown");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        assert!(output.contains("# Review Comments Export"));
        assert!(!output.contains("---")); // No comment separators
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
    fn uses_diff_language_for_unknown_extension() {
        let mut buffer = Vec::new();
        let comments = vec![make_comment(
            1,
            None,
            Some("config.unknown"),
            Some(1),
            None,
            Some("some code"),
            None,
        )];

        write_markdown(&mut buffer, &comments, "https://example.com/pr/1")
            .expect("should write markdown");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        assert!(output.contains("```diff"));
    }

    #[rstest]
    fn uses_diff_language_when_no_file_path() {
        let mut buffer = Vec::new();
        let comments = vec![make_comment(
            1,
            None,
            None,
            None,
            None,
            Some("+ added line"),
            None,
        )];

        write_markdown(&mut buffer, &comments, "https://example.com/pr/1")
            .expect("should write markdown");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        assert!(output.contains("```diff"));
    }

    #[rstest]
    fn multiple_comments_have_separators() {
        let mut buffer = Vec::new();
        let comments = vec![
            make_comment(
                1,
                Some("alice"),
                Some("a.rs"),
                Some(1),
                Some("First"),
                None,
                None,
            ),
            make_comment(
                2,
                Some("bob"),
                Some("b.rs"),
                Some(2),
                Some("Second"),
                None,
                None,
            ),
        ];

        write_markdown(&mut buffer, &comments, "https://example.com/pr/1")
            .expect("should write markdown");

        let output = String::from_utf8(buffer).expect("valid UTF-8");
        let separator_count = output.matches("---").count();
        assert_eq!(separator_count, 2); // One per comment
    }
}
