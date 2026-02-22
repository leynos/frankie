//! Tests for TUI startup context storage helpers.

use std::sync::Arc;

use mockall::mock;

use crate::local::{
    CommitSha, CommitSnapshot, GitOperationError, GitOperations, LineMappingRequest,
    LineMappingVerification, RepoFilePath,
};
use crate::telemetry::test_support::RecordingTelemetrySink;
use crate::telemetry::{NoopTelemetrySink, TelemetryEvent, TelemetrySink};

use super::*;

mock! {
    pub GitOps {}

    impl std::fmt::Debug for GitOps {
        fn fmt<'a>(&self, f: &mut std::fmt::Formatter<'a>) -> std::fmt::Result;
    }

    impl GitOperations for GitOps {
        fn get_commit_snapshot<'a>(
            &self,
            sha: &'a CommitSha,
            file_path: Option<&'a RepoFilePath>,
        ) -> Result<CommitSnapshot, GitOperationError>;

        fn get_file_at_commit<'a>(
            &self,
            sha: &'a CommitSha,
            file_path: &'a RepoFilePath,
        ) -> Result<String, GitOperationError>;

        fn verify_line_mapping<'a>(
            &self,
            request: &'a LineMappingRequest,
        ) -> Result<LineMappingVerification, GitOperationError>;

        fn get_parent_commits<'a>(
            &self,
            sha: &'a CommitSha,
            limit: usize,
        ) -> Result<Vec<CommitSha>, GitOperationError>;

        fn commit_exists<'a>(&self, sha: &'a CommitSha) -> bool;
    }
}

#[test]
fn get_telemetry_sink_returns_usable_sink() {
    // OnceLock may return Noop or a previously-set sink; verify no panic.
    record_sync_telemetry(100, 5, true);
}

#[test]
fn noop_telemetry_sink_can_record_without_panic() {
    let sink = NoopTelemetrySink;
    sink.record(TelemetryEvent::SyncLatencyRecorded {
        latency_ms: 42,
        comment_count: 3,
        incremental: false,
    });
}

#[test]
fn recording_sink_captures_sync_latency_event() {
    let sink = RecordingTelemetrySink::default();
    sink.record(TelemetryEvent::SyncLatencyRecorded {
        latency_ms: 150,
        comment_count: 10,
        incremental: false,
    });

    let events = sink.events();
    assert_eq!(events.len(), 1);

    let TelemetryEvent::SyncLatencyRecorded {
        latency_ms,
        comment_count,
        incremental,
    } = events.first().expect("events should not be empty")
    else {
        panic!(
            "expected SyncLatencyRecorded event, got {:?}",
            events.first()
        );
    };

    assert_eq!(*latency_ms, 150);
    assert_eq!(*comment_count, 10);
    assert!(!*incremental);
}

#[test]
fn set_telemetry_sink_wires_sink_for_record_sync_telemetry() {
    // OnceLock: only verify events if our sink was first to be set.
    let sink = Arc::new(RecordingTelemetrySink::default());
    let was_set = set_telemetry_sink(Arc::clone(&sink) as Arc<dyn TelemetrySink>);
    record_sync_telemetry(200, 15, true);
    if was_set {
        let events = sink.events();
        assert_eq!(events.len(), 1);
        let first_event = events.first().expect("events should not be empty");
        assert!(matches!(
            first_event,
            TelemetryEvent::SyncLatencyRecorded {
                latency_ms: 200,
                comment_count: 15,
                incremental: true,
            }
        ));
    }
}

/// Creates a `MockGitOps` with default expectations that return
/// deterministic errors, so accidental calls produce predictable
/// results instead of panicking.
fn default_mock_git_ops() -> MockGitOps {
    let mut mock = MockGitOps::new();
    mock.expect_get_commit_snapshot().returning(|_, _| {
        Err(GitOperationError::RepositoryNotAvailable {
            message: "stub".to_owned(),
        })
    });
    mock.expect_get_file_at_commit().returning(|_, _| {
        Err(GitOperationError::RepositoryNotAvailable {
            message: "stub".to_owned(),
        })
    });
    mock.expect_verify_line_mapping().returning(|_| {
        Err(GitOperationError::RepositoryNotAvailable {
            message: "stub".to_owned(),
        })
    });
    mock.expect_get_parent_commits().returning(|_, _| {
        Err(GitOperationError::RepositoryNotAvailable {
            message: "stub".to_owned(),
        })
    });
    mock.expect_commit_exists().returning(|_| false);
    mock
}

#[test]
fn set_git_ops_context_wires_ops_for_get() {
    let ops: Arc<dyn GitOperations> = Arc::new(default_mock_git_ops());
    let was_set = set_git_ops_context(ops, "abc123".to_owned());

    let retrieved = get_git_ops_context();
    assert!(retrieved.is_some(), "context should always be available");

    if was_set {
        let (_, head_sha) = retrieved.expect("already asserted Some");
        assert_eq!(head_sha, "abc123");
    }
}

#[test]
fn set_time_travel_context_wires_context_for_get() {
    let ctx = TimeTravelContext {
        host: "github.com".to_owned(),
        owner: "octocat".to_owned(),
        repo: "hello-world".to_owned(),
        pr_number: 42,
        discovery_failure: Some("no repo found".to_owned()),
    };
    let was_set = set_time_travel_context(ctx);

    let retrieved = get_time_travel_context();
    assert!(retrieved.is_some(), "context should always be available");

    if was_set {
        let stored = retrieved.expect("already asserted Some");
        assert_eq!(stored.owner, "octocat");
        assert_eq!(stored.repo, "hello-world");
        assert_eq!(stored.pr_number, 42);
        assert_eq!(stored.discovery_failure.as_deref(), Some("no repo found"));
    }
}

#[test]
fn reply_draft_config_falls_back_to_defaults() {
    let config = get_reply_draft_config();
    assert!(
        config.max_length.as_usize() >= 1,
        "default reply max_length should be positive"
    );
    assert!(
        !config.templates.is_empty(),
        "default reply templates should not be empty"
    );
}

#[test]
fn set_reply_draft_config_normalises_zero_max_length() {
    let custom = ReplyDraftConfig::new(ReplyDraftMaxLength::new(0), vec!["Template".to_owned()]);
    let was_set = set_reply_draft_config(custom);
    let config = get_reply_draft_config();
    assert!(
        config.max_length.as_usize() >= 1,
        "max_length should be normalised"
    );
    if was_set {
        assert_eq!(config.templates, vec!["Template".to_owned()]);
    }
}
