//! Integration tests that prove reply templating is available from `frankie`.

use frankie::{ReplyTemplateContext, ReplyTemplateError, render_reply_template};
use rstest::rstest;

#[path = "support/reply_template.rs"]
mod reply_template_support;

use reply_template_support::{review_comment_with_body, sample_review_comment};

#[rstest]
fn crate_root_re_export_renders_reply_templates() {
    let context = ReplyTemplateContext::from(&sample_review_comment());
    let rendered = render_reply_template("Thanks {{ reviewer }}", &context)
        .expect("crate-root reply template API should render");

    assert_eq!(rendered, "Thanks alice");
}

#[rstest]
fn crate_root_re_export_includes_comment_body_fields() {
    let context = ReplyTemplateContext::from(&review_comment_with_body("LGTM"));
    let rendered = render_reply_template("Body: {{ body }}", &context)
        .expect("crate-root reply template API should render comment body fields");

    assert_eq!(rendered, "Body: LGTM");
}

#[rstest]
fn module_path_exposes_reply_template_errors() {
    let context = ReplyTemplateContext::from(&sample_review_comment());
    let error = frankie::reply_template::render_reply_template("{{ reviewer", &context)
        .expect_err("invalid syntax should be surfaced through the public module path");

    assert!(matches!(error, ReplyTemplateError::InvalidSyntax { .. }));
    assert!(
        error
            .to_string()
            .starts_with("invalid reply template syntax:")
    );
}

#[rstest]
fn crate_root_re_export_exposes_reply_template_context_mapping() {
    let context = ReplyTemplateContext::from(&sample_review_comment());

    assert_eq!(
        context,
        ReplyTemplateContext {
            comment_id: 42,
            reviewer: "alice".to_owned(),
            file: "src/lib.rs".to_owned(),
            line: "12".to_owned(),
            body: "Please split this into smaller functions.".to_owned(),
        }
    );
}
