//! Unit tests for the shared reply-template renderer.

use rstest::{fixture, rstest};

use super::{ReplyTemplateError, render_reply_template};
use crate::github::models::ReviewComment;
use crate::reply_template::test_support::{review_comment_with_body, sample_review_comment};

#[fixture]
fn sample_comment() -> ReviewComment {
    sample_review_comment()
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
        &review_comment_with_body("Please keep {{ nested }} literal."),
    )
    .expect("comment body should be rendered as data");

    assert_eq!(rendered, "Please keep {{ nested }} literal.");
}

#[rstest]
fn render_reply_template_reports_invalid_syntax(sample_comment: ReviewComment) {
    let error = render_reply_template("{{ reviewer", &sample_comment)
        .expect_err("invalid syntax should return an error");

    assert!(matches!(error, ReplyTemplateError::InvalidSyntax { .. }));
    assert!(
        error
            .to_string()
            .starts_with("invalid reply template syntax:")
    );
}

#[rstest]
fn render_reply_template_reports_render_failures(sample_comment: ReviewComment) {
    let error = render_reply_template("{{ reviewer() }}", &sample_comment)
        .expect_err("calling a string as a function should fail during render");

    assert!(matches!(error, ReplyTemplateError::RenderFailed { .. }));
    assert!(
        error
            .to_string()
            .starts_with("reply template rendering failed:")
    );
}
