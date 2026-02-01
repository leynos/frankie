//! JSONL (JSON Lines) formatter for exported comments.
//!
//! Generates machine-readable output with one JSON object per line,
//! suitable for processing by AI tools or other automated pipelines.

use std::io::Write;

use crate::github::IntakeError;

use super::model::ExportedComment;

/// Writes comments in JSONL format to the given writer.
///
/// Each comment is serialized as a single JSON object on its own line,
/// with no trailing comma. Empty fields are omitted from the output.
///
/// # Errors
///
/// Returns [`IntakeError::Io`] if writing to the output fails, or if
/// JSON serialization fails (which should not happen for valid comments).
pub fn write_jsonl<W: Write>(
    writer: &mut W,
    comments: &[ExportedComment],
) -> Result<(), IntakeError> {
    for comment in comments {
        serde_json::to_writer(&mut *writer, comment).map_err(|e| IntakeError::Io {
            message: format!("JSON serialization failed: {e}"),
        })?;
        writeln!(writer).map_err(|e| io_error(&e))?;
    }
    Ok(())
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

    fn build(self) -> ExportedComment {
        ExportedComment {
            id: self.id,
            author: self.author,
            file_path: self.file_path,
            line_number: self.line_number,
            original_line_number: None,
            body: self.body,
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn write_to_string(comments: &[ExportedComment]) -> String {
        let mut buffer = Vec::new();
        write_jsonl(&mut buffer, comments).expect("should write JSONL");
        String::from_utf8(buffer).expect("valid UTF-8")
    }

    fn assert_json_field_eq(
        parsed: &serde_json::Value,
        field: &str,
        expected: impl Into<serde_json::Value>,
    ) {
        let actual = parsed.get(field);
        let expected_val = expected.into();
        assert_eq!(actual, Some(&expected_val), "field '{field}' mismatch");
    }

    #[rstest]
    fn writes_single_comment_as_json_line() {
        let comments = vec![
            CommentBuilder::new(123)
                .author("alice")
                .file_path("src/lib.rs")
                .line_number(42)
                .body("Fix this")
                .build(),
        ];

        let output = write_to_string(&comments);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);

        // Verify it's valid JSON
        let first_line = lines.first().expect("at least one line");
        let parsed: serde_json::Value =
            serde_json::from_str(first_line).expect("should be valid JSON");
        assert_json_field_eq(&parsed, "id", 123_u64);
        assert_json_field_eq(&parsed, "author", "alice");
        assert_json_field_eq(&parsed, "file_path", "src/lib.rs");
        assert_json_field_eq(&parsed, "line_number", 42_u64);
        assert_json_field_eq(&parsed, "body", "Fix this");
    }

    #[rstest]
    fn writes_multiple_comments_one_per_line() {
        let comments = vec![
            CommentBuilder::new(1)
                .author("alice")
                .file_path("a.rs")
                .line_number(10)
                .body("First")
                .build(),
            CommentBuilder::new(2)
                .author("bob")
                .file_path("b.rs")
                .line_number(20)
                .body("Second")
                .build(),
            CommentBuilder::new(3)
                .author("charlie")
                .file_path("c.rs")
                .line_number(30)
                .body("Third")
                .build(),
        ];

        let output = write_to_string(&comments);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3);

        // Verify each line is valid JSON with correct ID
        for (i, line) in lines.iter().enumerate() {
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("should be valid JSON");
            assert_json_field_eq(&parsed, "id", (i + 1) as u64);
        }
    }

    #[rstest]
    fn empty_comments_produces_empty_output() {
        let comments: Vec<ExportedComment> = vec![];

        let output = write_to_string(&comments);
        assert!(output.is_empty());
    }

    #[rstest]
    fn omits_none_fields() {
        let comments = vec![CommentBuilder::new(42).build()];

        let output = write_to_string(&comments);
        let parsed: serde_json::Value =
            serde_json::from_str(output.trim()).expect("should be valid JSON");

        assert_json_field_eq(&parsed, "id", 42_u64);
        assert!(parsed.get("author").is_none());
        assert!(parsed.get("file_path").is_none());
        assert!(parsed.get("line_number").is_none());
        assert!(parsed.get("body").is_none());
    }

    #[rstest]
    fn escapes_special_characters_in_body() {
        let comments = vec![
            CommentBuilder::new(1)
                .body("Quote: \"hello\" and newline:\nend")
                .build(),
        ];

        let output = write_to_string(&comments);
        // The JSON should be on a single line with escaped characters
        assert_eq!(output.lines().count(), 1);

        let parsed: serde_json::Value =
            serde_json::from_str(output.trim()).expect("should be valid JSON");
        assert_json_field_eq(&parsed, "body", "Quote: \"hello\" and newline:\nend");
    }

    #[rstest]
    fn handles_unicode_in_body() {
        let comments = vec![
            CommentBuilder::new(1)
                .author("田中")
                .file_path("日本語.rs")
                .body("コメント: 🎉")
                .build(),
        ];

        let output = write_to_string(&comments);
        let parsed: serde_json::Value =
            serde_json::from_str(output.trim()).expect("should be valid JSON");

        assert_json_field_eq(&parsed, "author", "田中");
        assert_json_field_eq(&parsed, "file_path", "日本語.rs");
        assert_json_field_eq(&parsed, "body", "コメント: 🎉");
    }

    #[rstest]
    fn each_line_ends_with_newline() {
        let comments = vec![
            CommentBuilder::new(1).build(),
            CommentBuilder::new(2).build(),
        ];

        let output = write_to_string(&comments);
        assert!(output.ends_with('\n'));
        // Count newlines
        assert_eq!(output.chars().filter(|&c| c == '\n').count(), 2);
    }
}
