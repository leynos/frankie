//! Integration tests that prove reply templating is available from `frankie`.

use frankie::{ReplyTemplateError, ReviewComment, render_reply_template};
use rstest::rstest;

fn sample_comment() -> ReviewComment {
    ReviewComment {
        id: 42,
        author: Some("alice".to_owned()),
        file_path: Some("src/lib.rs".to_owned()),
        line_number: Some(12),
        body: Some("Please keep {{ nested }} literal.".to_owned()),
        ..ReviewComment::default()
    }
}

#[rstest]
fn crate_root_re_export_renders_reply_templates() {
    let rendered = render_reply_template("Thanks {{ reviewer }}", &sample_comment())
        .expect("crate-root reply template API should render");

    assert_eq!(rendered, "Thanks alice");
}

#[rstest]
fn module_path_exposes_reply_template_errors() {
    let result = frankie::reply_template::render_reply_template("{{ reviewer", &sample_comment());

    assert!(
        matches!(result, Err(ReplyTemplateError::InvalidSyntax { .. })),
        "expected invalid syntax from public module path, got {result:?}"
    );
}
