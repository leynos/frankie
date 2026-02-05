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
#[case::file("file", "src/main.rs", |b: CommentBuilder| b.file_path("src/main.rs"))]
#[case::line("line", "42", |b: CommentBuilder| b.line_number(42))]
#[case::reviewer("reviewer", "alice", |b: CommentBuilder| b.author("alice"))]
#[case::body("body", "Please fix this issue", |b: CommentBuilder| b.body("Please fix this issue"))]
#[case::commit("commit", "abc123def", |b: CommentBuilder| b.commit_sha("abc123def"))]
#[case::timestamp("timestamp", "2025-01-15T10:30:00Z", |b: CommentBuilder| b.created_at("2025-01-15T10:30:00Z"))]
#[case::id("id", "1", |b: CommentBuilder| b)]
#[case::reply_to("reply_to", "999", |b: CommentBuilder| b.in_reply_to_id(999))]
fn substitutes_placeholder(
    #[case] field: &str,
    #[case] expected: &str,
    #[case] configure: fn(CommentBuilder) -> CommentBuilder,
) -> TestResult {
    let comments = vec![configure(CommentBuilder::new(1)).build()];
    let template = format!("{{% for c in comments %}}{{{{ c.{field} }}}}{{% endfor %}}");

    let output = render_template(&comments, "https://example.com/pr/1", &template)?;

    assert_contains(&output, expected)?;
    Ok(())
}

#[rstest]
#[case::root_comment(None, "comment", "reply")]
#[case::threaded_reply(Some(1), "reply", "comment")]
fn status_field_reflects_comment_type(
    #[case] in_reply_to: Option<u64>,
    #[case] expected: &str,
    #[case] unexpected: &str,
) -> TestResult {
    let mut builder = CommentBuilder::new(2);
    if let Some(parent_id) = in_reply_to {
        builder = builder.in_reply_to_id(parent_id);
    }
    let comments = vec![builder.build()];

    let output = render_template(
        &comments,
        "https://example.com/pr/1",
        "{% for c in comments %}{{ c.status }}{% endfor %}",
    )?;

    assert_contains(&output, expected)?;
    assert_not_contains(&output, unexpected)?;
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
fn runtime_error_returns_configuration_error() {
    let comments: Vec<ExportedComment> = vec![];

    // Using an unknown filter causes a runtime error
    let result = render_template(
        &comments,
        "https://example.com/pr/1",
        "{{ pr_url | nonexistent_filter }}",
    );

    let err = result.expect_err("should fail with unknown filter");
    assert!(
        matches!(err, IntakeError::Configuration { ref message } if message.contains("template rendering failed")),
        "expected Configuration error about rendering failure, got: {err:?}"
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
