//! Tests for TUI startup context storage helpers.

use std::error::Error;
use std::sync::Arc;
use std::sync::MutexGuard;

use mockall::mock;
use rstest::{fixture, rstest};

use crate::local::{
    CommitSha, CommitSnapshot, GitOperationError, GitOperations, LineMappingRequest,
    LineMappingVerification, RepoFilePath,
};
use crate::telemetry::test_support::RecordingTelemetrySink;
use crate::telemetry::{NoopTelemetrySink, TelemetryEvent, TelemetrySink};
use crate::tui::storage::storage_test_guard;

use super::*;

const SAMPLE_HEAD_SHA: &str = "abc123";

#[fixture]
fn storage_guard_fixture() -> MutexGuard<'static, ()> {
    storage_test_guard()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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

#[rstest]
fn set_telemetry_sink_wires_sink_for_record_sync_telemetry(
    storage_guard_fixture: MutexGuard<'static, ()>,
) -> Result<(), Box<dyn Error>> {
    let sink = Arc::new(RecordingTelemetrySink::default());
    let _ = set_telemetry_sink(Arc::clone(&sink) as Arc<dyn TelemetrySink>);
    record_sync_telemetry(200, 15, true);

    let events = sink.events();
    if events.len() != 1 {
        return Err(format!("expected exactly one telemetry event, got {}", events.len()).into());
    }
    let Some(first_event) = events.first() else {
        return Err("events should not be empty".into());
    };
    if !matches!(
        first_event,
        TelemetryEvent::SyncLatencyRecorded {
            latency_ms: 200,
            comment_count: 15,
            incremental: true,
        }
    ) {
        return Err(format!("unexpected telemetry event: {first_event:?}").into());
    }

    drop(storage_guard_fixture);
    Ok(())
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

#[rstest]
fn set_git_ops_context_wires_ops_for_get(
    storage_guard_fixture: MutexGuard<'static, ()>,
) -> Result<(), Box<dyn Error>> {
    let ops: Arc<dyn GitOperations> = Arc::new(default_mock_git_ops());
    let _ = set_git_ops_context(ops, SAMPLE_HEAD_SHA.to_owned());

    let Some((_, head_sha)) = get_git_ops_context() else {
        return Err("context should always be available".into());
    };
    if head_sha != SAMPLE_HEAD_SHA {
        return Err(format!("expected head SHA {SAMPLE_HEAD_SHA}, got {head_sha}").into());
    }

    drop(storage_guard_fixture);
    Ok(())
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

#[rstest]
fn set_reply_draft_config_normalises_zero_max_length(
    storage_guard_fixture: MutexGuard<'static, ()>,
) -> Result<(), Box<dyn Error>> {
    let custom = ReplyDraftConfig::new(ReplyDraftMaxLength::new(0), vec!["Template".to_owned()]);
    let _ = set_reply_draft_config(custom);
    let config = get_reply_draft_config();
    if config.max_length.as_usize() < 1 {
        return Err("max_length should be normalised".into());
    }
    if config.templates != vec!["Template".to_owned()] {
        return Err(format!("unexpected templates: {:?}", config.templates).into());
    }

    drop(storage_guard_fixture);
    Ok(())
}
