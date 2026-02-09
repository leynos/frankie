//! Tests for the positional PR identifier extraction in the CLI entrypoint.

use std::ffi::OsString;

use frankie::{FrankieConfig, IntakeError};
use ortho_config::OrthoConfig;

use super::extract_positional_pr_identifier;

/// Helper to build an `OsString` argument vector from string slices.
fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

/// A single scenario for positional PR identifier extraction.
struct TestCase {
    name: &'static str,
    input: &'static [&'static str],
    expected_id: Option<&'static str>,
    expected_remaining: &'static [&'static str],
}

fn positional_extraction_cases() -> Vec<TestCase> {
    vec![
        TestCase {
            name: "extracts bare PR number",
            input: &["frankie", "123"],
            expected_id: Some("123"),
            expected_remaining: &["frankie"],
        },
        TestCase {
            name: "extracts PR URL",
            input: &["frankie", "https://github.com/owner/repo/pull/42"],
            expected_id: Some("https://github.com/owner/repo/pull/42"),
            expected_remaining: &["frankie"],
        },
        TestCase {
            name: "skips value of preceding flag",
            input: &["frankie", "--token", "abc", "123"],
            expected_id: Some("123"),
            expected_remaining: &["frankie", "--token", "abc"],
        },
        TestCase {
            name: "skips value of short flag",
            input: &["frankie", "-t", "abc", "42"],
            expected_id: Some("42"),
            expected_remaining: &["frankie", "-t", "abc"],
        },
        TestCase {
            name: "does not skip value for equals syntax",
            input: &["frankie", "--token=abc", "99"],
            expected_id: Some("99"),
            expected_remaining: &["frankie", "--token=abc"],
        },
        TestCase {
            name: "unknown flag does not consume following value",
            input: &["frankie", "--foo", "123"],
            expected_id: Some("123"),
            expected_remaining: &["frankie", "--foo"],
        },
        TestCase {
            name: "grouped short flags treated as single flag",
            input: &["frankie", "-Tn", "42"],
            expected_id: Some("42"),
            expected_remaining: &["frankie", "-Tn"],
        },
        TestCase {
            name: "double-dash separator treats remainder as positional",
            input: &["frankie", "--token", "abc", "--", "77"],
            expected_id: Some("77"),
            expected_remaining: &["frankie", "--token", "abc"],
        },
        TestCase {
            name: "double-dash consumed even when no positional follows",
            input: &["frankie", "--tui", "--"],
            expected_id: None,
            expected_remaining: &["frankie", "--tui"],
        },
    ]
}

#[test]
fn extracts_positional_pr_identifier_correctly() {
    for case in &positional_extraction_cases() {
        let (id, remaining) = extract_positional_pr_identifier(args(case.input));

        assert_eq!(
            id.as_deref(),
            case.expected_id,
            "{}: unexpected extracted id",
            case.name
        );
        assert_eq!(
            remaining,
            args(case.expected_remaining),
            "{}: unexpected remaining args",
            case.name
        );
    }
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
