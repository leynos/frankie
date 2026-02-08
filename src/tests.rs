//! Tests for the positional PR identifier extraction in the CLI entrypoint.

use std::ffi::OsString;

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

#[test]
fn extracts_positional_pr_identifier_correctly() {
    let cases = vec![
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
    ];

    for case in &cases {
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
