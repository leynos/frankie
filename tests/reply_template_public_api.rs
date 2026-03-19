//! Integration tests that prove reply templating is available from `frankie`.

use frankie::{ReplyTemplateError, render_reply_template};
use rstest::rstest;

#[path = "support/reply_template.rs"]
mod reply_template_support;

use reply_template_support::{review_comment_with_body, sample_review_comment};

#[rstest]
fn crate_root_re_export_renders_reply_templates() {
    let rendered = render_reply_template("Thanks {{ reviewer }}", &sample_review_comment())
        .expect("crate-root reply template API should render");

    assert_eq!(rendered, "Thanks alice");
}

#[rstest]
fn crate_root_re_export_includes_comment_body_fields() {
    let rendered = render_reply_template("Body: {{ body }}", &review_comment_with_body("LGTM"))
        .expect("crate-root reply template API should render comment body fields");

    assert_eq!(rendered, "Body: LGTM");
}

#[rstest]
fn module_path_exposes_reply_template_errors() {
    let error =
        frankie::reply_template::render_reply_template("{{ reviewer", &sample_review_comment())
            .expect_err("invalid syntax should be surfaced through the public module path");

    assert!(matches!(error, ReplyTemplateError::InvalidSyntax { .. }));
    assert!(
        error
            .to_string()
            .starts_with("invalid reply template syntax:")
    );
}
