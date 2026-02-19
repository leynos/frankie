//! Tests for Codex TUI handlers.

use std::sync::Arc;

use bubbletea_rs::Model;
use camino::Utf8PathBuf;
use mockall::mock;
use rstest::{fixture, rstest};

use crate::ai::{
    CodexExecutionHandle, CodexExecutionOutcome, CodexExecutionRequest, CodexExecutionService,
    CodexExecutionUpdate, CodexProgressEvent, CodexResumeRequest, SessionState, SessionStatus,
};
use crate::github::models::test_support::minimal_review;
use crate::github::{IntakeError, PersonalAccessToken, PullRequestLocator};
use crate::tui::messages::AppMsg;

use super::ReviewApp;

const SAMPLE_FILE_PATH: &str = "src/main.rs";
const SAMPLE_LINE_NUMBER: u32 = 12;
const SAMPLE_COMMENT_BODY: &str = "Fix this branch";
const SAMPLE_REVIEWER: &str = "alice";

mock! {
    pub CodexService {}

    impl std::fmt::Debug for CodexService {
        fn fmt<'a>(&self, formatter: &mut std::fmt::Formatter<'a>) -> std::fmt::Result;
    }

    impl CodexExecutionService for CodexService {
        fn start(
            &self,
            request: CodexExecutionRequest,
        ) -> Result<CodexExecutionHandle, IntakeError>;
        fn resume(
            &self,
            request: CodexResumeRequest,
        ) -> Result<CodexExecutionHandle, IntakeError>;
    }
}

fn execution_handle_from_updates(updates: Vec<CodexExecutionUpdate>) -> CodexExecutionHandle {
    let (sender, receiver) = std::sync::mpsc::channel();
    for update in updates {
        drop(sender.send(update));
    }
    drop(sender);
    CodexExecutionHandle::new(receiver)
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
        file_path: Some(SAMPLE_FILE_PATH.to_owned()),
        line_number: Some(SAMPLE_LINE_NUMBER),
        body: Some(SAMPLE_COMMENT_BODY.to_owned()),
        ..minimal_review(1, SAMPLE_COMMENT_BODY, SAMPLE_REVIEWER)
    }]
}

#[rstest]
fn start_codex_execution_requires_filtered_comments(
    refresh_context: Result<(), IntakeError>,
) -> Result<(), Box<dyn std::error::Error>> {
    refresh_context?;

    let mut service_mock = MockCodexService::new();
    service_mock.expect_start().times(0);
    let service = Arc::new(service_mock);
    let mut app = ReviewApp::empty().with_codex_service(service);

    app.handle_message(&AppMsg::StartCodexExecution);

    let error = app.error_message().ok_or("expected error")?;
    if !error.contains("no filtered comments available") {
        return Err(format!("expected 'no filtered comments available', got: {error:?}").into());
    }

    Ok(())
}

#[rstest]
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

    let mut service_mock = MockCodexService::new();
    service_mock
        .expect_start()
        .times(1)
        .return_once(move |_| Ok(execution_handle_from_updates(updates)));
    let service = Arc::new(service_mock);
    let mut app = ReviewApp::new(sample_reviews).with_codex_service(service);

    app.handle_message(&AppMsg::StartCodexExecution);
    app.handle_message(&AppMsg::CodexPollTick);

    if let Some(error) = app.error_message() {
        return Err(format!("unexpected error: {error:?}").into());
    }

    let rendered = app.view();
    if !rendered.contains("Codex execution completed") {
        return Err("expected 'Codex execution completed' in view".into());
    }
    if !rendered.contains(transcript_path.as_str()) {
        return Err(format!("expected transcript path in view: {transcript_path}").into());
    }

    Ok(())
}

#[rstest]
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

    let mut service_mock = MockCodexService::new();
    service_mock
        .expect_start()
        .times(1)
        .return_once(move |_| Ok(execution_handle_from_updates(updates)));
    let service = Arc::new(service_mock);
    let mut app = ReviewApp::new(sample_reviews).with_codex_service(service);

    app.handle_message(&AppMsg::StartCodexExecution);
    app.handle_message(&AppMsg::CodexPollTick);

    let error = app.error_message().ok_or("expected Codex failure error")?;
    if !error.contains("exit code: 7") {
        return Err(format!("expected 'exit code: 7' in error, got: {error:?}").into());
    }
    if !error.contains("Transcript:") {
        return Err(format!("expected 'Transcript:' in error, got: {error:?}").into());
    }

    Ok(())
}

#[rstest]
fn start_failure_is_surfaced_as_error(
    refresh_context: Result<(), IntakeError>,
    sample_reviews: Vec<crate::github::models::ReviewComment>,
) -> Result<(), Box<dyn std::error::Error>> {
    refresh_context?;

    let mut service_mock = MockCodexService::new();
    service_mock.expect_start().times(1).return_once(|_| {
        Err(IntakeError::Api {
            message: "codex not found".to_owned(),
        })
    });
    let service = Arc::new(service_mock);
    let mut app = ReviewApp::new(sample_reviews).with_codex_service(service);

    app.handle_message(&AppMsg::StartCodexExecution);

    let error = app.error_message().ok_or("expected start failure")?;
    if !error.contains("codex not found") {
        return Err(format!("expected 'codex not found' in error, got: {error:?}").into());
    }

    Ok(())
}

#[tokio::test]
async fn codex_poll_timer_emits_poll_tick_message() -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::pause();

    let app = ReviewApp::empty();
    let cmd = app.arm_codex_poll_timer();

    tokio::time::advance(std::time::Duration::from_millis(200)).await;

    let result = cmd.await;
    let msg = result.ok_or("expected poll message")?;
    let app_msg = msg.downcast_ref::<AppMsg>();
    if !matches!(app_msg, Some(AppMsg::CodexPollTick)) {
        return Err(format!("expected CodexPollTick, got {app_msg:?}").into());
    }

    Ok(())
}

#[fixture]
fn interrupted_session() -> SessionState {
    SessionState {
        status: SessionStatus::Interrupted,
        transcript_path: Utf8PathBuf::from("/tmp/frankie-interrupted.jsonl"),
        thread_id: Some("thr_test123".to_owned()),
        owner: "owner".to_owned(),
        repository: "repo".to_owned(),
        pr_number: 42,
        started_at: chrono::Utc::now(),
        finished_at: Some(chrono::Utc::now()),
    }
}

#[rstest]
fn resume_prompt_shown_sets_resume_prompt_state(
    refresh_context: Result<(), IntakeError>,
    sample_reviews: Vec<crate::github::models::ReviewComment>,
    interrupted_session: SessionState,
) -> Result<(), Box<dyn std::error::Error>> {
    refresh_context?;

    let service = Arc::new(MockCodexService::new());
    let mut app = ReviewApp::new(sample_reviews).with_codex_service(service);

    app.handle_message(&AppMsg::ResumePromptShown(Box::new(interrupted_session)));

    let rendered = app.view();
    if !rendered.contains("Resume?") {
        return Err(format!("expected 'Resume?' in view, got: {rendered}").into());
    }

    Ok(())
}

#[rstest]
fn resume_accepted_starts_resumed_execution(
    refresh_context: Result<(), IntakeError>,
    sample_reviews: Vec<crate::github::models::ReviewComment>,
    interrupted_session: SessionState,
) -> Result<(), Box<dyn std::error::Error>> {
    refresh_context?;

    let transcript_path = Utf8PathBuf::from("/tmp/frankie-resumed.jsonl");
    let updates = vec![CodexExecutionUpdate::Finished(
        CodexExecutionOutcome::Succeeded {
            transcript_path: transcript_path.clone(),
        },
    )];

    let mut service_mock = MockCodexService::new();
    service_mock.expect_start().times(0);
    service_mock
        .expect_resume()
        .times(1)
        .return_once(move |_| Ok(execution_handle_from_updates(updates)));
    let service = Arc::new(service_mock);
    let mut app = ReviewApp::new(sample_reviews).with_codex_service(service);

    // Set the resume prompt first.
    app.handle_message(&AppMsg::ResumePromptShown(Box::new(interrupted_session)));
    app.handle_message(&AppMsg::ResumeAccepted);

    if !app.is_codex_running() {
        // Handle may have already drained; poll to consume the finish event.
        app.handle_message(&AppMsg::CodexPollTick);
    }

    let rendered = app.view();
    if rendered.contains("Resume?") {
        return Err("resume prompt should be cleared after acceptance".into());
    }

    Ok(())
}

#[rstest]
fn resume_declined_starts_fresh_execution(
    refresh_context: Result<(), IntakeError>,
    sample_reviews: Vec<crate::github::models::ReviewComment>,
    interrupted_session: SessionState,
) -> Result<(), Box<dyn std::error::Error>> {
    refresh_context?;

    let transcript_path = Utf8PathBuf::from("/tmp/frankie-fresh.jsonl");
    let updates = vec![CodexExecutionUpdate::Finished(
        CodexExecutionOutcome::Succeeded {
            transcript_path: transcript_path.clone(),
        },
    )];

    let mut service_mock = MockCodexService::new();
    service_mock.expect_resume().times(0);
    service_mock
        .expect_start()
        .times(1)
        .return_once(move |_| Ok(execution_handle_from_updates(updates)));
    let service = Arc::new(service_mock);
    let mut app = ReviewApp::new(sample_reviews).with_codex_service(service);

    // Set the resume prompt, then decline.
    app.handle_message(&AppMsg::ResumePromptShown(Box::new(interrupted_session)));
    app.handle_message(&AppMsg::ResumeDeclined);

    let rendered = app.view();
    if rendered.contains("Resume?") {
        return Err("resume prompt should be cleared after decline".into());
    }

    // The fresh execution should have been started.
    app.handle_message(&AppMsg::CodexPollTick);
    let rendered_after = app.view();
    if !rendered_after.contains("Codex execution completed") {
        return Err(format!(
            "expected 'Codex execution completed' after decline, got: {rendered_after}"
        )
        .into());
    }

    Ok(())
}
