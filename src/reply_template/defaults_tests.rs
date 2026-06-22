//! Unit tests for the built-in default reply templates.

use rstest::rstest;

use super::{DEFAULT_REPLY_TEMPLATES, default_reply_templates};
use crate::reply_template::{ReplyTemplateContext, render_reply_template};

#[rstest]
fn default_reply_templates_constant_is_non_empty() {
    assert!(
        !DEFAULT_REPLY_TEMPLATES.is_empty(),
        "the built-in default reply templates must not be empty"
    );
}

#[rstest]
fn default_reply_templates_function_derives_from_constant() {
    let owned = default_reply_templates();
    let expected: Vec<String> = DEFAULT_REPLY_TEMPLATES
        .iter()
        .map(|template| (*template).to_owned())
        .collect();
    assert_eq!(owned, expected);
}

#[rstest]
fn default_reply_templates_are_deterministic() {
    assert_eq!(default_reply_templates(), default_reply_templates());
}

#[rstest]
fn default_reply_templates_preserve_configured_defaults_in_order() {
    assert_eq!(
        DEFAULT_REPLY_TEMPLATES,
        [
            "Thanks for the review on {{ file }}:{{ line }}. I will update this.",
            "Good catch, {{ reviewer }}. I will address this in the next commit.",
            "I have addressed this feedback and pushed an update.",
        ]
    );
}

#[rstest]
fn each_default_template_renders_against_a_representative_context() {
    let context = ReplyTemplateContext {
        comment_id: 7,
        reviewer: "alice".to_owned(),
        file: "src/lib.rs".to_owned(),
        line: "12".to_owned(),
        body: "Please tidy this up.".to_owned(),
    };

    for template in DEFAULT_REPLY_TEMPLATES {
        let rendered = render_reply_template(template, &context);
        assert!(
            rendered.is_ok(),
            "default reply template should render: {template}"
        );
    }
}
