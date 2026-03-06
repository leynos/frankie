//! Tests for review TUI CLI orchestration helpers.

use std::sync::Arc;

use super::*;

#[test]
fn review_app_can_be_created_empty() {
    let app = ReviewApp::empty();
    assert_eq!(app.filtered_count(), 0);
}

#[test]
fn bare_number_rejected_when_local_discovery_disabled() {
    let result = resolve_from_identifier("42", true);

    assert!(
        matches!(result, Err(IntakeError::Configuration { .. })),
        "bare number with no_local_discovery should fail, got {result:?}"
    );
}

#[test]
fn url_identifier_allowed_when_local_discovery_disabled() {
    let result = resolve_from_identifier("https://github.com/octo/repo/pull/42", true);

    assert!(
        result.is_ok(),
        "URL identifier should succeed even with no_local_discovery, got {result:?}"
    );
}

/// Verifies that `StderrJsonlTelemetrySink` implements `TelemetrySink`
/// and can be used with `set_telemetry_sink`, demonstrating the CLI
/// telemetry wiring pattern used in the `run` function.
///
/// This test covers the CLI side of telemetry wiring. For the full
/// end-to-end integration test demonstrating events flowing from TUI
/// sync handlers through to the telemetry sink, see the BDD scenario
/// "Sync latency is logged to telemetry" in `tests/review_sync_bdd.rs`.
#[test]
fn cli_telemetry_wiring_pattern_is_valid() {
    use frankie::telemetry::TelemetrySink;

    // Create the sink exactly as done in run() at line 56
    let sink: Arc<dyn TelemetrySink> = Arc::new(StderrJsonlTelemetrySink);

    // Verify it implements TelemetrySink and can record events without panic
    sink.record(frankie::telemetry::TelemetryEvent::SyncLatencyRecorded {
        latency_ms: 42,
        comment_count: 5,
        incremental: true,
    });

    // Wire it to the TUI module (same call as in run())
    // The call may fail due to OnceLock if already set by another test,
    // but we verify the wiring pattern compiles and the sink is usable.
    let _ = set_telemetry_sink(sink);
}

#[rstest::rstest]
#[case::matching_repo(
    "https://github.com/octocat/hello-world/pull/42",
    "octocat",
    "hello-world",
    true
)]
#[case::case_insensitive_matching(
    "https://github.com/OctoCat/Hello-World/pull/1",
    "octocat",
    "hello-world",
    true
)]
#[case::mismatched_repo(
    "https://github.com/octocat/hello-world/pull/1",
    "octocat",
    "other-repo",
    false
)]
#[case::mismatched_owner(
    "https://github.com/alice/hello-world/pull/1",
    "bob",
    "hello-world",
    false
)]
fn validate_repo_matches_locator_cases(
    #[case] locator_url: &str,
    #[case] origin_owner: &str,
    #[case] origin_repo: &str,
    #[case] should_succeed: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let locator = PullRequestLocator::parse(locator_url)?;
    let origin = GitHubOrigin::GitHubCom {
        owner: origin_owner.to_owned(),
        repository: origin_repo.to_owned(),
    };

    let result = validate_repo_matches_locator(&origin, &locator);
    if result.is_ok() != should_succeed {
        return Err(format!("expected is_ok={should_succeed}, got {result:?}").into());
    }

    Ok(())
}

#[test]
fn validate_repo_rejects_mismatched_enterprise_host() -> Result<(), Box<dyn std::error::Error>> {
    let locator = PullRequestLocator::parse("https://ghe.corp.com/octocat/hello-world/pull/1")?;

    let origin = GitHubOrigin::Enterprise {
        host: "ghe.other.com".to_owned(),
        port: None,
        owner: "octocat".to_owned(),
        repository: "hello-world".to_owned(),
    };

    let result = validate_repo_matches_locator(&origin, &locator);
    let err = result
        .err()
        .ok_or("expected Err for mismatched enterprise host")?;
    if !err.contains("ghe.other.com") {
        return Err(format!("error should mention local host: {err}").into());
    }
    if !err.contains("ghe.corp.com") {
        return Err(format!("error should mention PR host: {err}").into());
    }

    Ok(())
}
