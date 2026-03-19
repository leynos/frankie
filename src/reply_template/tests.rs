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
#[case(
    "{{ reviewer",
    "invalid syntax should return an error",
    (|error| matches!(error, ReplyTemplateError::InvalidSyntax { .. }))
        as fn(&ReplyTemplateError) -> bool,
    "invalid reply template syntax:"
)]
#[case(
    "{{ reviewer() }}",
    "calling a string as a function should fail during render",
    (|error| matches!(error, ReplyTemplateError::RenderFailed { .. }))
        as fn(&ReplyTemplateError) -> bool,
    "reply template rendering failed:"
)]
fn render_reply_template_reports_errors(
    sample_comment: ReviewComment,
    #[case] template: &str,
    #[case] expect_err_msg: &str,
    #[case] is_expected_variant: fn(&ReplyTemplateError) -> bool,
    #[case] expected_prefix: &str,
) {
    let error = render_reply_template(template, &sample_comment).expect_err(expect_err_msg);

    assert!(is_expected_variant(&error));
    assert!(error.to_string().starts_with(expected_prefix));
}
