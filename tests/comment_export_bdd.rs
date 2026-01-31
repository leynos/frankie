//! Behavioural tests for structured comment export.

#[path = "comment_export_bdd/mod.rs"]
mod comment_export_bdd_support;

// Export types defined inline since they're in the binary crate
mod export_types {
    use frankie::{IntakeError, ReviewComment};
    use serde::Serialize;
    use std::cmp::Ordering;
    use std::io::Write;
    use std::path::Path;
    use std::str::FromStr;

    #[derive(Debug, Clone, Serialize, PartialEq, Eq)]
    pub struct ExportedComment {
        pub id: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub author: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub file_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub line_number: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub original_line_number: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub diff_hunk: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub commit_sha: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub in_reply_to_id: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub created_at: Option<String>,
    }

    impl From<&ReviewComment> for ExportedComment {
        fn from(comment: &ReviewComment) -> Self {
            Self {
                id: comment.id,
                author: comment.author.clone(),
                file_path: comment.file_path.clone(),
                line_number: comment.line_number,
                original_line_number: comment.original_line_number,
                body: comment.body.clone(),
                diff_hunk: comment.diff_hunk.clone(),
                commit_sha: comment.commit_sha.clone(),
                in_reply_to_id: comment.in_reply_to_id,
                created_at: comment.created_at.clone(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ExportFormat {
        Markdown,
        Jsonl,
    }

    impl FromStr for ExportFormat {
        type Err = IntakeError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "markdown" | "md" => Ok(Self::Markdown),
                "jsonl" | "json-lines" | "jsonlines" => Ok(Self::Jsonl),
                _ => Err(IntakeError::Configuration {
                    message: format!(
                        "unsupported export format '{s}': valid options are 'markdown' or 'jsonl'"
                    ),
                }),
            }
        }
    }

    fn compare_options<T: Ord>(a: Option<&T>, b: Option<&T>) -> Ordering {
        match (a, b) {
            (Some(a_val), Some(b_val)) => a_val.cmp(b_val),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }

    pub fn sort_comments(comments: &mut [ExportedComment]) {
        comments.sort_by(|a, b| {
            let file_cmp = compare_options(a.file_path.as_ref(), b.file_path.as_ref());
            if file_cmp != Ordering::Equal {
                return file_cmp;
            }

            let line_cmp = compare_options(a.line_number.as_ref(), b.line_number.as_ref());
            if line_cmp != Ordering::Equal {
                return line_cmp;
            }

            a.id.cmp(&b.id)
        });
    }

    fn format_comment_location(file_path: Option<&String>, line_number: Option<u32>) -> String {
        match (file_path, line_number) {
            (Some(path), Some(line)) => format!("{path}:{line}"),
            (Some(path), None) => path.clone(),
            (None, Some(line)) => format!("(unknown file):{line}"),
            (None, None) => "(unknown location)".to_owned(),
        }
    }

    fn detect_language_from_path(file_path: Option<&String>) -> &'static str {
        file_path
            .and_then(|p| Path::new(p).extension())
            .and_then(|ext| ext.to_str())
            .map_or("diff", |ext| match ext {
                "rs" => "rust",
                "py" => "python",
                "js" => "javascript",
                "ts" => "typescript",
                _ => "diff",
            })
    }

    pub fn write_markdown<W: Write>(
        writer: &mut W,
        comments: &[ExportedComment],
        pr_url: &str,
    ) -> Result<(), IntakeError> {
        writeln!(writer, "# Review Comments Export").map_err(|e| io_error(&e))?;
        writeln!(writer).map_err(|e| io_error(&e))?;
        writeln!(writer, "PR: {pr_url}").map_err(|e| io_error(&e))?;
        writeln!(writer).map_err(|e| io_error(&e))?;

        for comment in comments {
            writeln!(writer, "---").map_err(|e| io_error(&e))?;
            writeln!(writer).map_err(|e| io_error(&e))?;

            let location = format_comment_location(comment.file_path.as_ref(), comment.line_number);
            writeln!(writer, "## {location}").map_err(|e| io_error(&e))?;
            writeln!(writer).map_err(|e| io_error(&e))?;

            if let Some(author) = &comment.author {
                writeln!(writer, "**Reviewer:** {author}").map_err(|e| io_error(&e))?;
            }
            if let Some(created_at) = &comment.created_at {
                writeln!(writer, "**Created:** {created_at}").map_err(|e| io_error(&e))?;
            }

            if let Some(body) = &comment.body {
                writeln!(writer).map_err(|e| io_error(&e))?;
                writeln!(writer, "{body}").map_err(|e| io_error(&e))?;
            }

            if let Some(diff_hunk) = &comment.diff_hunk {
                let language = detect_language_from_path(comment.file_path.as_ref());

                writeln!(writer).map_err(|e| io_error(&e))?;
                writeln!(writer, "```{language}").map_err(|e| io_error(&e))?;
                writeln!(writer, "{diff_hunk}").map_err(|e| io_error(&e))?;
                writeln!(writer, "```").map_err(|e| io_error(&e))?;
            }

            writeln!(writer).map_err(|e| io_error(&e))?;
        }

        Ok(())
    }

    pub fn write_jsonl<W: Write>(
        writer: &mut W,
        comments: &[ExportedComment],
    ) -> Result<(), IntakeError> {
        for comment in comments {
            serde_json::to_writer(&mut *writer, comment).map_err(|e| IntakeError::Io {
                message: format!("JSON serialisation failed: {e}"),
            })?;
            writeln!(writer).map_err(|e| io_error(&e))?;
        }
        Ok(())
    }

    fn io_error(error: &std::io::Error) -> IntakeError {
        IntakeError::Io {
            message: error.to_string(),
        }
    }
}

use comment_export_bdd_support::{
    CommentCount, ExportState, ensure_runtime_and_server, generate_ordered_comments,
    generate_review_comments,
};
use export_types::{ExportFormat, ExportedComment, sort_comments, write_jsonl, write_markdown};
use frankie::{
    IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken, PullRequestLocator,
    ReviewCommentGateway,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[fixture]
fn export_state() -> ExportState {
    ExportState::default()
}

#[given(
    "a mock GitHub API server with {count:CommentCount} review comments for owner/repo/pull/42"
)]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn seed_server_with_comments(export_state: &ExportState, count: CommentCount) {
    let runtime = ensure_runtime_and_server(export_state)
        .unwrap_or_else(|error| panic!("failed to create Tokio runtime: {error}"));

    let comments = generate_review_comments(count);
    let comments_path = "/api/v3/repos/owner/repo/pulls/42/comments";

    let mock = Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&comments));

    export_state
        .server
        .with_ref(|server| {
            runtime.block_on(mock.mount(server));
        })
        .expect("mock server not initialised");
}

#[given("a mock GitHub API server with comments in random order for owner/repo/pull/42")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn seed_server_with_ordered_comments(export_state: &ExportState) {
    let runtime = ensure_runtime_and_server(export_state)
        .unwrap_or_else(|error| panic!("failed to create Tokio runtime: {error}"));

    let comments = generate_ordered_comments();
    let comments_path = "/api/v3/repos/owner/repo/pulls/42/comments";

    let mock = Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&comments));

    export_state
        .server
        .with_ref(|server| {
            runtime.block_on(mock.mount(server));
        })
        .expect("mock server not initialised");
}

#[given("a personal access token {token}")]
fn remember_token(export_state: &ExportState, token: String) {
    export_state.token.set(token);
}

#[when("the client exports comments for {pr_url} in {format} format")]
fn export_comments(export_state: &ExportState, pr_url: String, format: String) {
    let result = run_export(export_state, &pr_url, &format);

    match result {
        Ok(output) => {
            drop(export_state.error.take());
            export_state.output.set(output);
        }
        Err(error) => {
            drop(export_state.output.take());
            export_state.error.set(error);
        }
    }
}

fn run_export(
    export_state: &ExportState,
    pr_url: &str,
    format: &str,
) -> Result<String, IntakeError> {
    let server_url = export_state
        .server
        .with_ref(MockServer::uri)
        .ok_or_else(|| IntakeError::Api {
            message: "mock server URL missing".to_owned(),
        })?;

    let resolved_url = resolve_mock_server_url(&server_url, pr_url);
    let locator = PullRequestLocator::parse(&resolved_url)?;

    let runtime = export_state.runtime.get().ok_or_else(|| IntakeError::Api {
        message: "runtime not initialised".to_owned(),
    })?;

    runtime.block_on(async {
        let token_value = export_state.token.get().ok_or(IntakeError::MissingToken)?;
        let token = PersonalAccessToken::new(token_value)?;

        let export_format: ExportFormat = format.parse()?;

        let gateway = OctocrabReviewCommentGateway::new(&token, locator.api_base().as_str())?;
        let reviews = gateway.list_review_comments(&locator).await?;

        let mut comments: Vec<ExportedComment> =
            reviews.iter().map(ExportedComment::from).collect();
        sort_comments(&mut comments);

        let mut buffer = Vec::new();
        match export_format {
            ExportFormat::Markdown => {
                write_markdown(&mut buffer, &comments, &resolved_url)?;
            }
            ExportFormat::Jsonl => {
                write_jsonl(&mut buffer, &comments)?;
            }
        }

        String::from_utf8(buffer).map_err(|e| IntakeError::Api {
            message: format!("invalid UTF-8 in output: {e}"),
        })
    })
}

fn resolve_mock_server_url(server_url: &str, pr_url: &str) -> String {
    let cleaned_url = pr_url.trim_matches('"');
    if cleaned_url.contains("://SERVER") {
        cleaned_url
            .replace("https://SERVER", server_url)
            .replace("http://SERVER", server_url)
    } else {
        cleaned_url.replace("SERVER", server_url)
    }
}

#[expect(clippy::expect_used, reason = "helper for integration test steps")]
fn get_output(export_state: &ExportState) -> String {
    export_state
        .output
        .with_ref(Clone::clone)
        .expect("output missing")
}

#[expect(clippy::expect_used, reason = "helper for integration test steps")]
fn get_error(export_state: &ExportState) -> IntakeError {
    export_state
        .error
        .with_ref(Clone::clone)
        .expect("expected error")
}

fn trim_quotes(text: &str) -> &str {
    text.trim_matches('"')
}

fn assert_output_contains(export_state: &ExportState, expected: &str, context: &str) {
    let output = get_output(export_state);
    assert!(
        output.contains(expected),
        "expected output to contain {context} '{expected}', got:\n{output}"
    );
}

fn assert_count_equals(actual: usize, expected: usize, item_type: &str) {
    assert_eq!(
        actual, expected,
        "expected {expected} {item_type}, found {actual}"
    );
}

#[then("the output has header {text}")]
fn assert_output_has_header(export_state: &ExportState, text: String) {
    assert_output_contains(export_state, trim_quotes(&text), "header");
}

#[then("the output has PR URL containing {text}")]
fn assert_output_has_pr_url(export_state: &ExportState, text: String) {
    assert_output_contains(export_state, trim_quotes(&text), "PR URL");
}

#[then("the output has {count:CommentCount} comment sections")]
fn assert_comment_section_count(export_state: &ExportState, count: CommentCount) {
    let output = get_output(export_state);
    let section_count = output.matches("---").count();
    assert_count_equals(section_count, count.value() as usize, "comment sections");
}

#[then("the output has {count:CommentCount} JSON lines")]
fn assert_json_line_count(export_state: &ExportState, count: CommentCount) {
    let output = get_output(export_state);
    let line_count = output.lines().filter(|line| !line.is_empty()).count();
    assert_count_equals(line_count, count.value() as usize, "JSON lines");
}

#[then("each JSON line is valid JSON with an id field")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_valid_json_with_id(export_state: &ExportState) {
    let output = export_state
        .output
        .with_ref(Clone::clone)
        .expect("output missing");

    for (i, line) in output.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let parsed: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|_| panic!("line {i} should be valid JSON"));
        assert!(
            parsed.get("id").is_some(),
            "line {i} should have an id field"
        );
    }
}

#[then("the first comment is for file {file} line {line:CommentCount}")]
fn assert_first_comment_location(export_state: &ExportState, file: String, line: CommentCount) {
    // Strip quotes from captured file if present
    let file_path = file.trim_matches('"');
    assert_comment_at_index(export_state, 0, file_path, line.value());
}

#[then("the second comment is for file {file} line {line:CommentCount}")]
fn assert_second_comment_location(export_state: &ExportState, file: String, line: CommentCount) {
    // Strip quotes from captured file if present
    let file_path = file.trim_matches('"');
    assert_comment_at_index(export_state, 1, file_path, line.value());
}

#[then("the third comment is for file {file} line {line:CommentCount}")]
fn assert_third_comment_location(export_state: &ExportState, file: String, line: CommentCount) {
    // Strip quotes from captured file if present
    let file_path = file.trim_matches('"');
    assert_comment_at_index(export_state, 2, file_path, line.value());
}

#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
#[expect(
    clippy::indexing_slicing,
    reason = "test code with known indices from feature file"
)]
fn assert_comment_at_index(export_state: &ExportState, index: usize, file: &str, line: u32) {
    let output = export_state
        .output
        .with_ref(Clone::clone)
        .expect("output missing");

    let lines: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
    let parsed: serde_json::Value =
        serde_json::from_str(lines[index]).expect("should be valid JSON");

    assert_eq!(
        parsed.get("file_path").and_then(|v| v.as_str()),
        Some(file),
        "comment {index} should be for file {file}"
    );
    assert_eq!(
        parsed
            .get("line_number")
            .and_then(serde_json::Value::as_u64),
        Some(u64::from(line)),
        "comment {index} should be for line {line}"
    );
}

#[then("the output is empty")]
fn assert_output_empty(export_state: &ExportState) {
    let output = get_output(export_state);
    assert!(output.is_empty(), "expected empty output, got: {output}");
}

#[then("the error indicates unsupported export format")]
fn assert_unsupported_format_error(export_state: &ExportState) {
    let error = get_error(export_state);

    match error {
        IntakeError::Configuration { message } => {
            assert!(
                message.contains("unsupported export format"),
                "expected unsupported format error, got: {message}"
            );
        }
        other => panic!("expected Configuration error, got {other:?}"),
    }
}

#[scenario(path = "tests/features/comment_export.feature", index = 0)]
fn export_markdown_format(export_state: ExportState) {
    let _ = export_state;
}

#[scenario(path = "tests/features/comment_export.feature", index = 1)]
fn export_jsonl_format(export_state: ExportState) {
    let _ = export_state;
}

#[scenario(path = "tests/features/comment_export.feature", index = 2)]
fn export_with_stable_ordering(export_state: ExportState) {
    let _ = export_state;
}

#[scenario(path = "tests/features/comment_export.feature", index = 3)]
fn export_empty_markdown(export_state: ExportState) {
    let _ = export_state;
}

#[scenario(path = "tests/features/comment_export.feature", index = 4)]
fn export_empty_jsonl(export_state: ExportState) {
    let _ = export_state;
}

#[scenario(path = "tests/features/comment_export.feature", index = 5)]
fn invalid_export_format(export_state: ExportState) {
    let _ = export_state;
}
