//! Behavioural tests for structured comment export.

#[path = "comment_export_bdd/mod.rs"]
mod comment_export_bdd_support;

use comment_export_bdd_support::{
    CommentCount, ExportState, ensure_runtime_and_server, generate_ordered_comments,
    generate_review_comments,
};
use frankie::{
    ExportFormat, ExportedComment, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken,
    PullRequestLocator, ReviewCommentGateway, sort_comments, write_jsonl, write_markdown,
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
fn seed_server_with_comments(
    export_state: &ExportState,
    count: CommentCount,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = ensure_runtime_and_server(export_state)?;

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
        .ok_or("mock server not initialised")?;

    Ok(())
}

#[given("a mock GitHub API server with comments in random order for owner/repo/pull/42")]
fn seed_server_with_ordered_comments(
    export_state: &ExportState,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = ensure_runtime_and_server(export_state)?;

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
        .ok_or("mock server not initialised")?;

    Ok(())
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

fn get_output(export_state: &ExportState) -> Result<String, Box<dyn std::error::Error>> {
    export_state
        .output
        .with_ref(Clone::clone)
        .ok_or_else(|| "output missing".into())
}

fn get_error(export_state: &ExportState) -> Result<IntakeError, Box<dyn std::error::Error>> {
    export_state
        .error
        .with_ref(Clone::clone)
        .ok_or_else(|| "expected error".into())
}

fn trim_quotes(text: &str) -> &str {
    text.trim_matches('"')
}

fn assert_output_contains(
    export_state: &ExportState,
    expected: &str,
    context: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = get_output(export_state)?;
    if !output.contains(expected) {
        return Err(
            format!("expected output to contain {context} '{expected}', got:\n{output}").into(),
        );
    }
    Ok(())
}

fn assert_count_equals(
    actual: usize,
    expected: usize,
    item_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if actual != expected {
        return Err(format!("expected {expected} {item_type}, found {actual}").into());
    }
    Ok(())
}

#[then("the output has header {text}")]
fn assert_output_has_header(
    export_state: &ExportState,
    text: String,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_output_contains(export_state, trim_quotes(&text), "header")
}

#[then("the output has PR URL containing {text}")]
fn assert_output_has_pr_url(
    export_state: &ExportState,
    text: String,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_output_contains(export_state, trim_quotes(&text), "PR URL")
}

#[then("the output has {count:CommentCount} comment sections")]
fn assert_comment_section_count(
    export_state: &ExportState,
    count: CommentCount,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = get_output(export_state)?;
    let section_count = output.matches("---").count();
    assert_count_equals(section_count, count.value() as usize, "comment sections")
}

#[then("the output has {count:CommentCount} JSON lines")]
fn assert_json_line_count(
    export_state: &ExportState,
    count: CommentCount,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = get_output(export_state)?;
    let line_count = output.lines().filter(|line| !line.is_empty()).count();
    assert_count_equals(line_count, count.value() as usize, "JSON lines")
}

#[then("each JSON line is valid JSON with an id field")]
fn assert_valid_json_with_id(export_state: &ExportState) -> Result<(), Box<dyn std::error::Error>> {
    let output = get_output(export_state)?;

    for (i, line) in output.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let parsed: serde_json::Value =
            serde_json::from_str(line).map_err(|_| format!("line {i} should be valid JSON"))?;
        if parsed.get("id").is_none() {
            return Err(format!("line {i} should have an id field").into());
        }
    }
    Ok(())
}

#[then("the first comment is for file {file} line {line:CommentCount}")]
fn assert_first_comment_location(
    export_state: &ExportState,
    file: String,
    line: CommentCount,
) -> Result<(), Box<dyn std::error::Error>> {
    // Strip quotes from captured file if present
    let file_path = file.trim_matches('"');
    assert_comment_at_index(export_state, 0, file_path, line.value())
}

#[then("the second comment is for file {file} line {line:CommentCount}")]
fn assert_second_comment_location(
    export_state: &ExportState,
    file: String,
    line: CommentCount,
) -> Result<(), Box<dyn std::error::Error>> {
    // Strip quotes from captured file if present
    let file_path = file.trim_matches('"');
    assert_comment_at_index(export_state, 1, file_path, line.value())
}

#[then("the third comment is for file {file} line {line:CommentCount}")]
fn assert_third_comment_location(
    export_state: &ExportState,
    file: String,
    line: CommentCount,
) -> Result<(), Box<dyn std::error::Error>> {
    // Strip quotes from captured file if present
    let file_path = file.trim_matches('"');
    assert_comment_at_index(export_state, 2, file_path, line.value())
}

fn assert_comment_at_index(
    export_state: &ExportState,
    index: usize,
    file: &str,
    line: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = get_output(export_state)?;

    let lines: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
    let line_content = lines
        .get(index)
        .ok_or_else(|| format!("no output line at index {index}"))?;
    let parsed: serde_json::Value =
        serde_json::from_str(line_content).map_err(|_| "should be valid JSON")?;

    let actual_file = parsed.get("file_path").and_then(|v| v.as_str());
    if actual_file != Some(file) {
        return Err(
            format!("comment {index} should be for file {file}, got {actual_file:?}").into(),
        );
    }

    let actual_line = parsed
        .get("line_number")
        .and_then(serde_json::Value::as_u64);
    if actual_line != Some(u64::from(line)) {
        return Err(
            format!("comment {index} should be for line {line}, got {actual_line:?}").into(),
        );
    }

    Ok(())
}

#[then("the output is empty")]
fn assert_output_empty(export_state: &ExportState) -> Result<(), Box<dyn std::error::Error>> {
    let output = get_output(export_state)?;
    if !output.is_empty() {
        return Err(format!("expected empty output, got: {output}").into());
    }
    Ok(())
}

#[then("the error indicates unsupported export format")]
fn assert_unsupported_format_error(
    export_state: &ExportState,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = get_error(export_state)?;

    match error {
        IntakeError::Configuration { message } => {
            if !message.contains("unsupported export format") {
                return Err(format!("expected unsupported format error, got: {message}").into());
            }
            Ok(())
        }
        other => Err(format!("expected Configuration error, got {other:?}").into()),
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
