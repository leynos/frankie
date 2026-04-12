//! Tests for TUI startup context storage helpers.

use std::sync::Arc;

use mockall::mock;

use crate::local::{
    CommitSha, CommitSnapshot, GitOperationError, GitOperations, LineMappingRequest,
    LineMappingVerification, RepoFilePath,
};
use crate::telemetry::test_support::RecordingTelemetrySink;
use crate::telemetry::{NoopTelemetrySink, TelemetryEvent, TelemetrySink};
use crate::tui::storage::storage_test_guard;

use super::*;

const SAMPLE_HEAD_SHA: &str = "abc123";
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
    let _guard = storage_test_guard()
        .lock()
        .expect("storage test guard should not be poisoned");
    let sink = Arc::new(RecordingTelemetrySink::default());
    assert!(
        set_telemetry_sink(Arc::clone(&sink) as Arc<dyn TelemetrySink>),
        "first set should populate the OnceLock"
    );
    record_sync_telemetry(200, 15, true);
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
    let _guard = storage_test_guard()
        .lock()
        .expect("storage test guard should not be poisoned");
    let ops: Arc<dyn GitOperations> = Arc::new(default_mock_git_ops());
    assert!(
        set_git_ops_context(ops, SAMPLE_HEAD_SHA.to_owned()),
        "first set should populate the OnceLock"
    );

    let retrieved = get_git_ops_context();
    assert!(retrieved.is_some(), "context should always be available");
    let (_, head_sha) = retrieved.expect("already asserted Some");
    assert_eq!(head_sha, SAMPLE_HEAD_SHA);
}

#[test]
fn time_travel_context_helpers_are_re_exported_from_tui() {
    let setter: fn(TimeTravelContext) -> bool = set_time_travel_context;
    let getter: fn() -> Option<TimeTravelContext> = get_time_travel_context;

    let _ = setter;
    let _ = getter;
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
    let _guard = storage_test_guard()
        .lock()
        .expect("storage test guard should not be poisoned");
    let custom = ReplyDraftConfig::new(ReplyDraftMaxLength::new(0), vec!["Template".to_owned()]);
    assert!(
        set_reply_draft_config(custom),
        "first set should populate the OnceLock"
    );
    let config = get_reply_draft_config();
    assert!(
        config.max_length.as_usize() >= 1,
        "max_length should be normalised"
    );
    assert_eq!(config.templates, vec!["Template".to_owned()]);
}
