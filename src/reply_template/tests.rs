//! Unit tests for the shared reply-template renderer.

use rstest::{fixture, rstest};

use super::{ReplyTemplateContext, ReplyTemplateError, render_reply_template};
use crate::github::models::ReviewComment;
use crate::reply_template::test_support::{review_comment_with_body, sample_review_comment};

#[fixture]
fn sample_comment() -> ReviewComment {
    sample_review_comment()
}

#[fixture]
fn sample_context() -> ReplyTemplateContext {
    ReplyTemplateContext::from(&sample_review_comment())
}

#[rstest]
fn render_reply_template_includes_context_fields(sample_context: ReplyTemplateContext) {
    let rendered = render_reply_template("{{ reviewer }} {{ file }}:{{ line }}", &sample_context)
        .expect("template should render");

    assert_eq!(rendered, "alice src/lib.rs:12");
}

#[test]
fn render_reply_template_uses_defaults_for_missing_fields() {
    let context = ReplyTemplateContext::from(&ReviewComment {
        id: 42,
        ..ReviewComment::default()
    });
    let rendered =
        render_reply_template("{{ reviewer }} {{ file }}:{{ line }} {{ body }}", &context)
            .expect("template should render missing-field defaults");

    assert_eq!(rendered, "reviewer (unknown file): ");
}

#[rstest]
fn render_reply_template_preserves_literal_braces(sample_context: ReplyTemplateContext) {
    let rendered = render_reply_template(
        "{% raw %}{{ reviewer }}{% endraw %} => {{ reviewer }}",
        &sample_context,
    )
    .expect("template should render escaped braces");

    assert_eq!(rendered, "{{ reviewer }} => alice");
}

#[test]
fn render_reply_template_does_not_recurse_into_comment_data() {
    let context = ReplyTemplateContext::from(&review_comment_with_body(
        "Please keep {{ nested }} literal.",
    ));
    let rendered = render_reply_template("{{ body }}", &context)
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
    #[case] template: &str,
    #[case] expect_err_msg: &str,
    #[case] is_expected_variant: fn(&ReplyTemplateError) -> bool,
    #[case] expected_prefix: &str,
) {
    let sample_context = ReplyTemplateContext::from(&sample_review_comment());
    let error = render_reply_template(template, &sample_context).expect_err(expect_err_msg);

    assert!(is_expected_variant(&error));
    assert!(error.to_string().starts_with(expected_prefix));
}

#[rstest]
#[case(
    sample_review_comment(),
    ReplyTemplateContext {
        comment_id: 42,
        reviewer: "alice".to_owned(),
        file: "src/lib.rs".to_owned(),
        line: "12".to_owned(),
        body: "Please split this into smaller functions.".to_owned(),
    }
)]
#[case(
    ReviewComment {
        id: 99,
        ..ReviewComment::default()
    },
    ReplyTemplateContext {
        comment_id: 99,
        reviewer: "reviewer".to_owned(),
        file: "(unknown file)".to_owned(),
        line: String::new(),
        body: String::new(),
    }
)]
#[case(
    ReviewComment {
        id: 100,
        file_path: Some("src/main.rs".to_owned()),
        body: Some("Needs a follow-up".to_owned()),
        ..ReviewComment::default()
    },
    ReplyTemplateContext {
        comment_id: 100,
        reviewer: "reviewer".to_owned(),
        file: "src/main.rs".to_owned(),
        line: String::new(),
        body: "Needs a follow-up".to_owned(),
    }
)]
fn reply_template_context_from_review_comment_normalizes_fields(
    #[case] comment: ReviewComment,
    #[case] expected: ReplyTemplateContext,
) {
    assert_eq!(ReplyTemplateContext::from(&comment), expected);
}
