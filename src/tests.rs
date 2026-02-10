//! Tests for the positional PR identifier extraction in the CLI entrypoint.

use std::ffi::OsString;

use frankie::{FrankieConfig, IntakeError};
use ortho_config::OrthoConfig;
use rstest::rstest;

use super::extract_positional_pr_identifier;

/// Helper to build an `OsString` argument vector from string slices.
fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[rstest]
#[case::bare_pr_number(
    &["frankie", "123"],
    Some("123"),
    &["frankie"],
)]
#[case::pr_url(
    &["frankie", "https://github.com/owner/repo/pull/42"],
    Some("https://github.com/owner/repo/pull/42"),
    &["frankie"],
)]
#[case::skips_value_of_preceding_flag(
    &["frankie", "--token", "abc", "123"],
    Some("123"),
    &["frankie", "--token", "abc"],
)]
#[case::skips_value_of_short_flag(
    &["frankie", "-t", "abc", "42"],
    Some("42"),
    &["frankie", "-t", "abc"],
)]
#[case::equals_syntax_does_not_skip_value(
    &["frankie", "--token=abc", "99"],
    Some("99"),
    &["frankie", "--token=abc"],
)]
#[case::unknown_flag_does_not_consume_value(
    &["frankie", "--foo", "123"],
    Some("123"),
    &["frankie", "--foo"],
)]
#[case::grouped_short_flags_as_single_flag(
    &["frankie", "-Tn", "42"],
    Some("42"),
    &["frankie", "-Tn"],
)]
#[case::double_dash_treats_remainder_as_positional(
    &["frankie", "--token", "abc", "--", "77"],
    Some("77"),
    &["frankie", "--token", "abc"],
)]
#[case::double_dash_consumed_without_positional(
    &["frankie", "--tui", "--"],
    None,
    &["frankie", "--tui"],
)]
fn extracts_positional_pr_identifier_correctly(
    #[case] input: &[&str],
    #[case] expected_id: Option<&str>,
    #[case] expected_remaining: &[&str],
) {
    let (id, remaining) = extract_positional_pr_identifier(args(input));

    assert_eq!(id.as_deref(), expected_id, "unexpected extracted id");
    assert_eq!(
        remaining,
        args(expected_remaining),
        "unexpected remaining args"
    );
}

#[test]
fn returns_none_when_no_positional() {
    let (id, remaining) = extract_positional_pr_identifier(args(&["frankie", "--tui"]));

    assert_eq!(id, None, "no positional argument present");
    assert_eq!(
        remaining,
        args(&["frankie", "--tui"]),
        "flags should be preserved"
    );
}

#[test]
fn preserves_all_flags_around_positional() {
    let (id, remaining) = extract_positional_pr_identifier(args(&[
        "frankie",
        "--tui",
        "-t",
        "tok",
        "55",
        "--no-local-discovery",
    ]));

    assert_eq!(id.as_deref(), Some("55"), "should extract 55");
    assert_eq!(
        remaining,
        args(&["frankie", "--tui", "-t", "tok", "--no-local-discovery"]),
        "all flags should be preserved in order"
    );
}

#[test]
fn returns_none_with_only_flags() {
    let (id, remaining) = extract_positional_pr_identifier(args(&[
        "frankie",
        "--pr-url",
        "https://github.com/o/r/pull/1",
        "-T",
    ]));

    assert_eq!(
        id, None,
        "URL after --pr-url is a flag value, not positional"
    );
    assert_eq!(
        remaining,
        args(&["frankie", "--pr-url", "https://github.com/o/r/pull/1", "-T"]),
        "all args should be preserved"
    );
}

#[test]
fn empty_args_returns_none() {
    let (id, remaining) = extract_positional_pr_identifier(args(&["frankie"]));

    assert_eq!(id, None, "no positional with only program name");
    assert_eq!(remaining, args(&["frankie"]));
}

/// Exercises the full CLI → config → validation pipeline to verify that
/// supplying both a positional identifier and `--pr-url` surfaces a
/// `Configuration` error from `validate()`.
#[test]
fn load_config_rejects_positional_identifier_with_pr_url() {
    let raw_args = args(&[
        "frankie",
        "--pr-url",
        "https://github.com/o/r/pull/1",
        "123",
    ]);

    let (identifier, filtered) = extract_positional_pr_identifier(raw_args);

    let mut config = FrankieConfig::load_from_iter(filtered)
        .expect("ortho-config should parse the filtered args");

    if let Some(value) = identifier {
        config.set_pr_identifier(value);
    }

    let result = config.validate();

    assert!(
        matches!(result, Err(IntakeError::Configuration { .. })),
        "expected Configuration error for conflicting identifier and pr_url, got {result:?}"
    );
}
