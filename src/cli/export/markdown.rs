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
struct CommentBuilder {
    id: u64,
    author: Option<String>,
    file_path: Option<String>,
    line_number: Option<u32>,
    body: Option<String>,
    diff_hunk: Option<String>,
    created_at: Option<String>,
}

#[cfg(test)]
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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn write_markdown_to_string(
        comments: &[ExportedComment],
        pr_url: &str,
    ) -> Result<String, IntakeError> {
        let mut buffer = Vec::new();
        write_markdown(&mut buffer, comments, pr_url)?;
        Ok(String::from_utf8(buffer).expect("valid UTF-8"))
    }

    fn assert_single_comment_output_contains(comment: ExportedComment, expected_substring: &str) {
        let comments = vec![comment];
        let output = write_markdown_to_string(&comments, "https://example.com/pr/1")
            .expect("should write markdown");
        assert!(
            output.contains(expected_substring),
            "expected output to contain '{expected_substring}', got:\n{output}"
        );
    }

    #[rstest]
    fn writes_header_with_pr_url() {
        let comments: Vec<ExportedComment> = vec![];

        let output = write_markdown_to_string(&comments, "https://github.com/owner/repo/pull/123")
            .expect("should write markdown");

        assert!(output.contains("# Review Comments Export"));
        assert!(output.contains("PR: https://github.com/owner/repo/pull/123"));
    }

    #[rstest]
    fn writes_comment_with_all_fields() {
        let comments = vec![
            CommentBuilder::new(1)
                .author("alice")
                .file_path("src/lib.rs")
                .line_number(42)
                .body("Consider using a constant here.")
                .diff_hunk("@@ -40,3 +40,5 @@\n let x = 1;")
                .created_at("2025-01-15T10:00:00Z")
                .build(),
        ];

        let output = write_markdown_to_string(&comments, "https://example.com/pr/1")
            .expect("should write markdown");

        assert!(output.contains("## src/lib.rs:42"));
        assert!(output.contains("**Reviewer:** alice"));
        assert!(output.contains("**Created:** 2025-01-15T10:00:00Z"));
        assert!(output.contains("Consider using a constant here."));
        assert!(output.contains("```rust"));
        assert!(output.contains("@@ -40,3 +40,5 @@"));
    }

    #[rstest]
    fn handles_missing_file_path() {
        let comment = CommentBuilder::new(1)
            .author("bob")
            .line_number(10)
            .body("Fix this")
            .build();

        assert_single_comment_output_contains(comment, "## (unknown file):10");
    }

    #[rstest]
    fn handles_missing_line_number() {
        let comment = CommentBuilder::new(1)
            .author("charlie")
            .file_path("README.md")
            .body("Update docs")
            .build();

        assert_single_comment_output_contains(comment, "## README.md");
    }

    #[rstest]
    fn handles_completely_missing_location() {
        let comment = CommentBuilder::new(1).body("General comment").build();

        assert_single_comment_output_contains(comment, "## (unknown location)");
    }

    #[rstest]
    fn empty_comments_produces_header_only() {
        let comments: Vec<ExportedComment> = vec![];

        let output = write_markdown_to_string(&comments, "https://example.com/pr/1")
            .expect("should write markdown");

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
        let comment = CommentBuilder::new(1)
            .file_path("config.unknown")
            .line_number(1)
            .diff_hunk("some code")
            .build();

        assert_single_comment_output_contains(comment, "```diff");
    }

    #[rstest]
    fn uses_diff_language_when_no_file_path() {
        let comment = CommentBuilder::new(1).diff_hunk("+ added line").build();

        assert_single_comment_output_contains(comment, "```diff");
    }

    #[rstest]
    fn multiple_comments_have_separators() {
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

        let output = write_markdown_to_string(&comments, "https://example.com/pr/1")
            .expect("should write markdown");

        let separator_count = output.matches("---").count();
        assert_eq!(separator_count, 2); // One per comment
    }
}
