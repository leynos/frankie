//! Integration tests that prove reply templating is available from `frankie`.

use frankie::reply_template::test_support::sample_review_comment;
use frankie::{ReplyTemplateError, render_reply_template};
use rstest::rstest;

#[rstest]
fn crate_root_re_export_renders_reply_templates() {
    let rendered = render_reply_template("Thanks {{ reviewer }}", &sample_review_comment())
        .expect("crate-root reply template API should render");

    assert_eq!(rendered, "Thanks alice");
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
