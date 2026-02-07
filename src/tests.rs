//! Tests for the positional PR identifier extraction in the CLI entrypoint.

use std::ffi::OsString;

use super::extract_positional_pr_identifier;

/// Helper to build an `OsString` argument vector from string slices.
fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn extracts_bare_pr_number() {
    let (id, remaining) = extract_positional_pr_identifier(args(&["frankie", "123"]));

    assert_eq!(id.as_deref(), Some("123"), "should extract bare PR number");
    assert_eq!(
        remaining,
        args(&["frankie"]),
        "positional should be removed"
    );
}

#[test]
fn extracts_pr_url() {
    let url = "https://github.com/owner/repo/pull/42";
    let (id, remaining) = extract_positional_pr_identifier(args(&["frankie", url]));

    assert_eq!(id.as_deref(), Some(url), "should extract PR URL");
    assert_eq!(
        remaining,
        args(&["frankie"]),
        "positional should be removed"
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
fn skips_value_of_preceding_flag() {
    let (id, remaining) =
        extract_positional_pr_identifier(args(&["frankie", "--token", "abc", "123"]));

    assert_eq!(
        id.as_deref(),
        Some("123"),
        "should skip token value and extract 123"
    );
    assert_eq!(
        remaining,
        args(&["frankie", "--token", "abc"]),
        "token flag and value should be preserved"
    );
}

#[test]
fn skips_value_of_short_flag() {
    let (id, remaining) = extract_positional_pr_identifier(args(&["frankie", "-t", "abc", "42"]));

    assert_eq!(
        id.as_deref(),
        Some("42"),
        "should skip -t value and extract 42"
    );
    assert_eq!(
        remaining,
        args(&["frankie", "-t", "abc"]),
        "short flag and value should be preserved"
    );
}

#[test]
fn does_not_skip_value_for_equals_syntax() {
    let (id, remaining) = extract_positional_pr_identifier(args(&["frankie", "--token=abc", "99"]));

    assert_eq!(
        id.as_deref(),
        Some("99"),
        "equals syntax is self-contained; next arg is positional"
    );
    assert_eq!(
        remaining,
        args(&["frankie", "--token=abc"]),
        "equals-style flag should be preserved"
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
