//! Tests for Codex TUI handlers.

use std::sync::{Mutex, MutexGuard};

use bubbletea_rs::Model;
use camino::Utf8PathBuf;
use rstest::{fixture, rstest};

use crate::ai::{
    CodexExecutionHandle, CodexExecutionOutcome, CodexExecutionRequest, CodexExecutionService,
    CodexExecutionUpdate, CodexProgressEvent,
};
use crate::github::models::test_support::minimal_review;
use crate::github::{IntakeError, PersonalAccessToken, PullRequestLocator};
use crate::tui::messages::AppMsg;

use super::ReviewApp;

#[derive(Debug)]
struct StubCodexService {
    behaviour: Mutex<Vec<StubBehaviour>>,
}

#[derive(Debug)]
enum StubBehaviour {
    StartError(IntakeError),
    Updates(Vec<CodexExecutionUpdate>),
}

impl StubCodexService {
    fn with_updates(updates: Vec<CodexExecutionUpdate>) -> Self {
        Self {
            behaviour: Mutex::new(vec![StubBehaviour::Updates(updates)]),
        }
    }

    fn with_start_error(error: IntakeError) -> Self {
        Self {
            behaviour: Mutex::new(vec![StubBehaviour::StartError(error)]),
        }
    }

    fn next_behaviour(
        lock: &mut MutexGuard<'_, Vec<StubBehaviour>>,
    ) -> Result<StubBehaviour, IntakeError> {
        if lock.is_empty() {
            return Err(IntakeError::Api {
                message: "stub behaviour queue is empty".to_owned(),
            });
        }

        Ok(lock.remove(0))
    }
}

impl CodexExecutionService for StubCodexService {
    fn start(&self, _request: CodexExecutionRequest) -> Result<CodexExecutionHandle, IntakeError> {
        let mut behaviour = self.behaviour.lock().map_err(|error| IntakeError::Api {
            message: format!("failed to lock stub behaviour: {error}"),
        })?;

        match Self::next_behaviour(&mut behaviour)? {
            StubBehaviour::StartError(error) => Err(error),
            StubBehaviour::Updates(updates) => {
                let (sender, receiver) = std::sync::mpsc::channel();
                for update in updates {
                    drop(sender.send(update));
                }
                drop(sender);
                Ok(CodexExecutionHandle::new(receiver))
            }
        }
    }
}

#[fixture]
fn refresh_context() -> Result<(), IntakeError> {
    let locator = PullRequestLocator::parse("https://github.com/owner/repo/pull/42")?;
    let token = PersonalAccessToken::new("test-token")?;
    // Returns false when already initialised; safe to ignore in tests
    // sharing a process-wide OnceLock.
    let _already_set = crate::tui::set_refresh_context(locator, token);
    Ok(())
}

#[fixture]
fn sample_reviews() -> Vec<crate::github::models::ReviewComment> {
    vec![crate::github::models::ReviewComment {
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(12),
        body: Some("Fix this branch".to_owned()),
        ..minimal_review(1, "Fix this branch", "alice")
    }]
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions are expected to panic on failure"
)]
fn start_codex_execution_requires_filtered_comments(
    refresh_context: Result<(), IntakeError>,
) -> Result<(), Box<dyn std::error::Error>> {
    refresh_context?;

    let service = std::sync::Arc::new(StubCodexService::with_updates(Vec::new()));
    let mut app = ReviewApp::empty().with_codex_service(service);

    app.handle_message(&AppMsg::StartCodexExecution);

    let error = app.error_message().ok_or("expected error")?;
    assert!(error.contains("no filtered comments available"));

    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions are expected to panic on failure"
)]
fn codex_progress_and_success_are_reflected_in_state(
    refresh_context: Result<(), IntakeError>,
    sample_reviews: Vec<crate::github::models::ReviewComment>,
) -> Result<(), Box<dyn std::error::Error>> {
    refresh_context?;

    let transcript_path = Utf8PathBuf::from("/tmp/frankie-codex-success.jsonl");
    let updates = vec![
        CodexExecutionUpdate::Progress(CodexProgressEvent::Status {
            message: "event: turn.started".to_owned(),
        }),
        CodexExecutionUpdate::Finished(CodexExecutionOutcome::Succeeded {
            transcript_path: transcript_path.clone(),
        }),
    ];

    let service = std::sync::Arc::new(StubCodexService::with_updates(updates));
    let mut app = ReviewApp::new(sample_reviews).with_codex_service(service);

    app.handle_message(&AppMsg::StartCodexExecution);
    app.handle_message(&AppMsg::CodexPollTick);

    assert!(
        app.error_message().is_none(),
        "unexpected error: {:?}",
        app.error_message()
    );

    let rendered = app.view();
    assert!(rendered.contains("Codex execution completed"));
    assert!(rendered.contains(transcript_path.as_str()));

    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions are expected to panic on failure"
)]
fn non_zero_exit_sets_tui_error_message(
    refresh_context: Result<(), IntakeError>,
    sample_reviews: Vec<crate::github::models::ReviewComment>,
) -> Result<(), Box<dyn std::error::Error>> {
    refresh_context?;

    let transcript_path = Utf8PathBuf::from("/tmp/frankie-codex-failure.jsonl");
    let updates = vec![CodexExecutionUpdate::Finished(
        CodexExecutionOutcome::Failed {
            message: "codex exited with a non-zero status".to_owned(),
            exit_code: Some(7),
            transcript_path: Some(transcript_path),
        },
    )];

    let service = std::sync::Arc::new(StubCodexService::with_updates(updates));
    let mut app = ReviewApp::new(sample_reviews).with_codex_service(service);

    app.handle_message(&AppMsg::StartCodexExecution);
    app.handle_message(&AppMsg::CodexPollTick);

    let error = app.error_message().ok_or("expected Codex failure error")?;
    assert!(error.contains("exit code: 7"));
    assert!(error.contains("Transcript:"));

    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions are expected to panic on failure"
)]
fn start_failure_is_surfaced_as_error(
    refresh_context: Result<(), IntakeError>,
    sample_reviews: Vec<crate::github::models::ReviewComment>,
) -> Result<(), Box<dyn std::error::Error>> {
    refresh_context?;

    let service = std::sync::Arc::new(StubCodexService::with_start_error(IntakeError::Api {
        message: "codex not found".to_owned(),
    }));
    let mut app = ReviewApp::new(sample_reviews).with_codex_service(service);

    app.handle_message(&AppMsg::StartCodexExecution);

    let error = app.error_message().ok_or("expected start failure")?;
    assert!(error.contains("codex not found"));

    Ok(())
}

#[tokio::test]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions are expected to panic on failure"
)]
async fn codex_poll_timer_emits_poll_tick_message() -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::pause();

    let cmd = ReviewApp::arm_codex_poll_timer();

    tokio::time::advance(std::time::Duration::from_millis(200)).await;

    let result = cmd.await;
    let msg = result.ok_or("expected poll message")?;
    let app_msg = msg.downcast_ref::<AppMsg>();
    assert!(matches!(app_msg, Some(AppMsg::CodexPollTick)));

    Ok(())
}
