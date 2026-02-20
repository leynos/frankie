//! Tests for reply-drafting configuration fields.

use rstest::rstest;
use serde_json::json;

use super::helpers::build_config_from_layers;
use crate::FrankieConfig;

#[rstest]
fn reply_max_length_defaults_to_expected_value() {
    let config = FrankieConfig::default();

    assert_eq!(
        config.reply_max_length, 500,
        "reply_max_length should default to 500"
    );
}

#[rstest]
fn reply_templates_default_list_is_non_empty() {
    let config = FrankieConfig::default();

    assert!(
        !config.reply_templates.is_empty(),
        "default reply_templates should not be empty"
    );
}

#[rstest]
fn reply_max_length_precedence_defaults_file_environment_cli() {
    let config = build_config_from_layers(&[
        ("defaults", json!({"reply_max_length": 300})),
        ("file", json!({"reply_max_length": 250})),
        ("environment", json!({"reply_max_length": 200})),
        ("cli", json!({"reply_max_length": 150})),
    ]);

    assert_eq!(
        config.reply_max_length, 150,
        "CLI should win for reply_max_length"
    );
}

#[rstest]
fn reply_templates_load_from_file_layer() {
    let config = build_config_from_layers(&[(
        "file",
        json!({
            "reply_templates": [
                "Thanks {{ reviewer }}",
                "Applied fix for {{ file }}"
            ]
        }),
    )]);

    assert_eq!(
        config.reply_templates,
        vec![
            "Thanks {{ reviewer }}".to_owned(),
            "Applied fix for {{ file }}".to_owned()
        ],
        "file-layer templates should be loaded"
    );
}
