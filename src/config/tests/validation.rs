//! Tests for configuration consistency validation.

use rstest::rstest;

use crate::FrankieConfig;
use crate::github::error::IntakeError;

#[rstest]
#[case::neither_set(FrankieConfig::default(), "neither pr_identifier nor pr_url")]
#[case::only_identifier(
    FrankieConfig { pr_identifier: Some("123".to_owned()), ..Default::default() },
    "only pr_identifier"
)]
#[case::only_url(
    FrankieConfig { pr_url: Some("https://github.com/o/r/pull/1".to_owned()), ..Default::default() },
    "only pr_url"
)]
fn validates_with_various_pr_inputs(#[case] config: FrankieConfig, #[case] description: &str) {
    assert!(config.validate().is_ok(), "should pass with {description}");
}

#[rstest]
fn rejects_both_identifier_and_url() {
    let config = FrankieConfig {
        pr_identifier: Some("123".to_owned()),
        pr_url: Some("https://github.com/o/r/pull/1".to_owned()),
        ..Default::default()
    };

    let result = config.validate();

    assert!(
        matches!(result, Err(IntakeError::Configuration { .. })),
        "should reject conflicting pr_identifier and pr_url, got {result:?}"
    );
}

#[rstest]
fn validates_when_ai_rewrite_mode_and_text_are_both_set() {
    let config = FrankieConfig {
        ai_rewrite_mode: Some("expand".to_owned()),
        ai_rewrite_text: Some("hello".to_owned()),
        ..Default::default()
    };

    assert!(config.validate().is_ok());
}

#[rstest]
#[case(
    FrankieConfig { ai_rewrite_mode: Some("expand".to_owned()), ..Default::default() },
    "mode without text"
)]
#[case(
    FrankieConfig { ai_rewrite_text: Some("hello".to_owned()), ..Default::default() },
    "text without mode"
)]
#[case(
    FrankieConfig {
        ai_rewrite_mode: Some("   ".to_owned()),
        ai_rewrite_text: Some("hello".to_owned()),
        ..Default::default()
    },
    "blank mode with text"
)]
#[case(
    FrankieConfig {
        ai_rewrite_mode: Some("expand".to_owned()),
        ai_rewrite_text: Some("   ".to_owned()),
        ..Default::default()
    },
    "mode with blank text"
)]
fn rejects_incomplete_ai_rewrite_configuration(
    #[case] config: FrankieConfig,
    #[case] description: &str,
) {
    let result = config.validate();

    assert!(
        matches!(result, Err(IntakeError::Configuration { .. })),
        "should reject {description}, got {result:?}"
    );
}
