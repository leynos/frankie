//! Unit tests for template-driven export.

use rstest::rstest;

use super::*;
use crate::export::test_helpers::{CommentBuilder, assert_contains, assert_not_contains};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn render_template(
    comments: &[ExportedComment],
    pr_url: &str,
    template: &str,
) -> Result<String, IntakeError> {
    let mut buffer = Vec::new();
    write_template(&mut buffer, comments, pr_url, template)?;
    String::from_utf8(buffer).map_err(|e| IntakeError::Io {
        message: format!("invalid UTF-8 output: {e}"),
    })
}

#[rstest]
fn substitutes_file_placeholder() -> TestResult {
    let comments = vec![CommentBuilder::new(1).file_path("src/main.rs").build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.file }}{% endfor %}",
    )?;

    assert_contains(&output, "src/main.rs")?;
    Ok(())
}

#[rstest]
fn substitutes_line_placeholder() -> TestResult {
    let comments = vec![CommentBuilder::new(1).line_number(42).build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.line }}{% endfor %}",
    )?;

    assert_contains(&output, "42")?;
    Ok(())
}

#[rstest]
fn substitutes_reviewer_placeholder() -> TestResult {
    let comments = vec![CommentBuilder::new(1).author("alice").build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.reviewer }}{% endfor %}",
    )?;

    assert_contains(&output, "alice")?;
    Ok(())
}

#[rstest]
fn status_is_comment_for_root_comments() -> TestResult {
    let comments = vec![CommentBuilder::new(1).build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.status }}{% endfor %}",
    )?;

    assert_contains(&output, "comment")?;
    assert_not_contains(&output, "reply")?;
    Ok(())
}

#[rstest]
fn status_is_reply_for_threaded_comments() -> TestResult {
    let comments = vec![CommentBuilder::new(2).in_reply_to_id(1).build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.status }}{% endfor %}",
    )?;

    assert_contains(&output, "reply")?;
    assert_not_contains(&output, "comment")?;
    Ok(())
}

#[rstest]
fn substitutes_body_placeholder() -> TestResult {
    let comments = vec![CommentBuilder::new(1).body("Please fix this issue").build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.body }}{% endfor %}",
    )?;

    assert_contains(&output, "Please fix this issue")?;
    Ok(())
}

#[rstest]
fn substitutes_context_placeholder() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1)
            .diff_hunk("@@ -1,3 +1,4 @@\n+new line")
            .build(),
    ];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.context }}{% endfor %}",
    )?;

    assert_contains(&output, "@@ -1,3 +1,4 @@")?;
    assert_contains(&output, "+new line")?;
    Ok(())
}

#[rstest]
fn substitutes_commit_placeholder() -> TestResult {
    let comments = vec![CommentBuilder::new(1).commit_sha("abc123def").build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.commit }}{% endfor %}",
    )?;

    assert_contains(&output, "abc123def")?;
    Ok(())
}

#[rstest]
fn substitutes_timestamp_placeholder() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1)
            .created_at("2025-01-15T10:30:00Z")
            .build(),
    ];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.timestamp }}{% endfor %}",
    )?;

    assert_contains(&output, "2025-01-15T10:30:00Z")?;
    Ok(())
}

#[rstest]
fn substitutes_id_placeholder() -> TestResult {
    let comments = vec![CommentBuilder::new(12345).build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.id }}{% endfor %}",
    )?;

    assert_contains(&output, "12345")?;
    Ok(())
}

#[rstest]
fn substitutes_reply_to_placeholder() -> TestResult {
    let comments = vec![CommentBuilder::new(2).in_reply_to_id(999).build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.reply_to }}{% endfor %}",
    )?;

    assert_contains(&output, "999")?;
    Ok(())
}

#[rstest]
fn substitutes_pr_url_document_variable() -> TestResult {
    let comments: Vec<ExportedComment> = vec![];

    let output = render_template(
        &comments,
        "https://github.com/owner/repo/pull/42",
        "PR: {{ pr_url }}",
    )?;

    assert_contains(&output, "PR: https://github.com/owner/repo/pull/42")?;
    Ok(())
}

#[rstest]
fn substitutes_generated_at_document_variable() -> TestResult {
    let comments: Vec<ExportedComment> = vec![];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "Generated: {{ generated_at }}",
    )?;

    // Should contain ISO 8601 formatted timestamp (starts with year)
    assert_contains(&output, "Generated: 20")?;
    Ok(())
}

#[rstest]
fn length_filter_works() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1).build(),
        CommentBuilder::new(2).build(),
        CommentBuilder::new(3).build(),
    ];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "Total: {{ comments | length }}",
    )?;

    assert_contains(&output, "Total: 3")?;
    Ok(())
}

#[rstest]
fn for_loop_iterates_all_comments() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1).author("alice").build(),
        CommentBuilder::new(2).author("bob").build(),
        CommentBuilder::new(3).author("charlie").build(),
    ];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}[{{ c.reviewer }}]{% endfor %}",
    )?;

    assert_contains(&output, "[alice]")?;
    assert_contains(&output, "[bob]")?;
    assert_contains(&output, "[charlie]")?;
    Ok(())
}

#[rstest]
fn missing_values_render_as_empty_string() -> TestResult {
    // Comment with no optional fields set
    let comments = vec![CommentBuilder::new(1).build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}file:[{{ c.file }}]line:[{{ c.line }}]{% endfor %}",
    )?;

    assert_contains(&output, "file:[]")?;
    assert_contains(&output, "line:[]")?;
    Ok(())
}

#[rstest]
fn handles_unicode_in_values() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1)
            .author("田中太郎")
            .body("コメント: 🎉✨")
            .file_path("日本語/ファイル.rs")
            .build(),
    ];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.reviewer }}: {{ c.body }} ({{ c.file }}){% endfor %}",
    )?;

    assert_contains(&output, "田中太郎")?;
    assert_contains(&output, "コメント: 🎉✨")?;
    assert_contains(&output, "日本語/ファイル.rs")?;
    Ok(())
}

#[rstest]
fn handles_special_chars_in_body() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1)
            .body("Quote: \"hello\" and newline:\nand tab:\there")
            .build(),
    ];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.body }}{% endfor %}",
    )?;

    assert_contains(&output, "Quote: \"hello\"")?;
    assert_contains(&output, "newline:\n")?;
    assert_contains(&output, "tab:\there")?;
    Ok(())
}

#[rstest]
fn no_html_escaping_by_default() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1)
            .body("<script>alert('xss')</script>")
            .build(),
    ];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.body }}{% endfor %}",
    )?;

    // Should NOT be HTML-escaped
    assert_contains(&output, "<script>")?;
    assert_not_contains(&output, "&lt;script&gt;")?;
    Ok(())
}

#[rstest]
fn invalid_template_syntax_returns_error() {
    let comments: Vec<ExportedComment> = vec![];

    let result = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for x in %}broken{% endfor %}",
    );

    let err = result.expect_err("should fail with invalid syntax");
    assert!(
        matches!(err, IntakeError::Configuration { ref message } if message.contains("invalid template syntax")),
        "expected Configuration error, got: {err:?}"
    );
}

#[rstest]
fn complex_template_renders_correctly() -> TestResult {
    let comments = vec![
        CommentBuilder::new(1)
            .author("alice")
            .file_path("src/lib.rs")
            .line_number(10)
            .body("First comment")
            .build(),
        CommentBuilder::new(2)
            .author("bob")
            .file_path("src/main.rs")
            .line_number(20)
            .body("Second comment")
            .in_reply_to_id(1)
            .build(),
    ];

    let template = r"# Export for {{ pr_url }}

{% for c in comments %}
## {{ c.file }}:{{ c.line }} ({{ c.status }})
**Reviewer:** {{ c.reviewer }}

{{ c.body }}
---
{% endfor %}

Total: {{ comments | length }} comments";

    let output = render_template(&comments, "https://github.com/owner/repo/pull/42", template)?;

    assert_contains(
        &output,
        "# Export for https://github.com/owner/repo/pull/42",
    )?;
    assert_contains(&output, "## src/lib.rs:10 (comment)")?;
    assert_contains(&output, "## src/main.rs:20 (reply)")?;
    assert_contains(&output, "**Reviewer:** alice")?;
    assert_contains(&output, "**Reviewer:** bob")?;
    assert_contains(&output, "First comment")?;
    assert_contains(&output, "Second comment")?;
    assert_contains(&output, "Total: 2 comments")?;
    Ok(())
}

#[rstest]
fn empty_comments_produces_document_only() -> TestResult {
    let comments: Vec<ExportedComment> = vec![];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "Header\n{% for c in comments %}{{ c.body }}{% endfor %}\nFooter",
    )?;

    assert_contains(&output, "Header")?;
    assert_contains(&output, "Footer")?;
    Ok(())
}
