//! Unit tests for the shared reply-template renderer.

use rstest::{fixture, rstest};

use super::{ReplyTemplateError, render_reply_template};
use crate::github::models::ReviewComment;

#[fixture]
fn sample_comment() -> ReviewComment {
    ReviewComment {
        id: 42,
        author: Some("alice".to_owned()),
        file_path: Some("src/lib.rs".to_owned()),
        line_number: Some(12),
        body: Some("Please split this into smaller functions.".to_owned()),
        ..ReviewComment::default()
    }
}

#[rstest]
fn render_reply_template_includes_comment_fields(sample_comment: ReviewComment) {
    let rendered = render_reply_template("{{ reviewer }} {{ file }}:{{ line }}", &sample_comment)
        .expect("template should render");

    assert_eq!(rendered, "alice src/lib.rs:12");
}

#[test]
fn render_reply_template_uses_defaults_for_missing_fields() {
    let rendered = render_reply_template(
        "{{ reviewer }} {{ file }}:{{ line }} {{ body }}",
        &ReviewComment {
            id: 42,
            ..ReviewComment::default()
        },
    )
    .expect("template should render missing-field defaults");

    assert_eq!(rendered, "reviewer (unknown file): ");
}

#[rstest]
fn render_reply_template_preserves_literal_braces(sample_comment: ReviewComment) {
    let rendered = render_reply_template(
        "{% raw %}{{ reviewer }}{% endraw %} => {{ reviewer }}",
        &sample_comment,
    )
    .expect("template should render escaped braces");

    assert_eq!(rendered, "{{ reviewer }} => alice");
}

#[test]
fn render_reply_template_does_not_recurse_into_comment_data() {
    let rendered = render_reply_template(
        "{{ body }}",
        &ReviewComment {
            id: 7,
            body: Some("Please keep {{ nested }} literal.".to_owned()),
            ..ReviewComment::default()
        },
    )
    .expect("comment body should be rendered as data");

    assert_eq!(rendered, "Please keep {{ nested }} literal.");
}

#[rstest]
fn render_reply_template_reports_invalid_syntax(sample_comment: ReviewComment) {
    let result = render_reply_template("{{ reviewer", &sample_comment);

    assert!(
        matches!(result, Err(ReplyTemplateError::InvalidSyntax { .. })),
        "expected invalid syntax error, got {result:?}"
    );
}

#[rstest]
fn render_reply_template_reports_render_failures(sample_comment: ReviewComment) {
    let result = render_reply_template("{{ reviewer() }}", &sample_comment);

    assert!(
        matches!(result, Err(ReplyTemplateError::RenderFailed { .. })),
        "expected render failure, got {result:?}"
    );
}
