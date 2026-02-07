//! Tests for configuration consistency validation.

use rstest::rstest;

use crate::FrankieConfig;
use crate::github::error::IntakeError;

#[rstest]
fn validates_when_neither_identifier_nor_url_set() {
    let config = FrankieConfig::default();

    assert!(
        config.validate().is_ok(),
        "should pass with neither pr_identifier nor pr_url"
    );
}

#[rstest]
fn validates_when_only_pr_identifier_set() {
    let config = FrankieConfig {
        pr_identifier: Some("123".to_owned()),
        ..Default::default()
    };

    assert!(
        config.validate().is_ok(),
        "should pass with only pr_identifier"
    );
}

#[rstest]
fn validates_when_only_pr_url_set() {
    let config = FrankieConfig {
        pr_url: Some("https://github.com/o/r/pull/1".to_owned()),
        ..Default::default()
    };

    assert!(config.validate().is_ok(), "should pass with only pr_url");
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
