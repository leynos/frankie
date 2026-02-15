//! Behavioural tests for Codex session resumption.

#[path = "codex_session_resume_bdd/mod.rs"]
mod codex_session_resume_bdd_support;

use bubbletea_rs::Model;
use codex_session_resume_bdd_support::{
    ResumeScenarioState, StubResumePlan, app_with_resume_plan, state::sample_interrupted_session,
};
use frankie::ai::{CodexExecutionOutcome, CodexExecutionUpdate, CodexProgressEvent};
use frankie::tui::messages::AppMsg;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn codex_state() -> ResumeScenarioState {
    ResumeScenarioState::default()
}

fn success_plan(delay_ms: u64) -> StubResumePlan {
    StubResumePlan::ResumeUpdates(vec![(
        delay_ms,
        CodexExecutionUpdate::Finished(CodexExecutionOutcome::Succeeded {
            transcript_path: camino::Utf8PathBuf::from("/tmp/frankie-bdd-resume.jsonl"),
        }),
    )])
}

fn fresh_success_plan(delay_ms: u64) -> StubResumePlan {
    StubResumePlan::FreshUpdates(vec![
        (
            0,
            CodexExecutionUpdate::Progress(CodexProgressEvent::Status {
                message: "event: turn.started".to_owned(),
            }),
        ),
        (
            delay_ms,
            CodexExecutionUpdate::Finished(CodexExecutionOutcome::Succeeded {
                transcript_path: camino::Utf8PathBuf::from("/tmp/frankie-bdd-fresh.jsonl"),
            }),
        ),
    ])
}

#[given("an interrupted Codex session is detected")]
fn given_interrupted_session(
    codex_state: &ResumeScenarioState,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = success_plan(120);
    let mut app = app_with_resume_plan(plan)?;

    let session = sample_interrupted_session();
    app.handle_message(&AppMsg::ResumePromptShown(Box::new(session)));

    codex_state.app.set(app);
    Ok(())
}

#[given("a Codex run that streams progress and completes successfully")]
fn given_successful_run(
    codex_state: &ResumeScenarioState,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = fresh_success_plan(120);
    codex_state.app.set(app_with_resume_plan(plan)?);
    Ok(())
}

#[when("Codex execution is started from the review TUI")]
fn when_start_codex(codex_state: &ResumeScenarioState) -> Result<(), Box<dyn std::error::Error>> {
    codex_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::StartCodexExecution);
        })
        .ok_or("app must be initialised")?;
    Ok(())
}

#[when("the user accepts the resume prompt")]
fn when_accept_resume(codex_state: &ResumeScenarioState) -> Result<(), Box<dyn std::error::Error>> {
    codex_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::ResumeAccepted);
        })
        .ok_or("app must be initialised")?;
    Ok(())
}

#[when("the user declines the resume prompt")]
fn when_decline_resume(
    codex_state: &ResumeScenarioState,
) -> Result<(), Box<dyn std::error::Error>> {
    codex_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::ResumeDeclined);
        })
        .ok_or("app must be initialised")?;
    Ok(())
}

#[when("the Codex poll tick is processed")]
fn when_poll_tick(codex_state: &ResumeScenarioState) -> Result<(), Box<dyn std::error::Error>> {
    codex_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::CodexPollTick);
        })
        .ok_or("app must be initialised")?;
    Ok(())
}

#[when("I wait {millis:u64} milliseconds")]
fn when_wait_ms(millis: u64) {
    std::thread::sleep(std::time::Duration::from_millis(millis));
}

#[then("the status bar shows a resume prompt")]
fn then_resume_prompt_shown(
    codex_state: &ResumeScenarioState,
) -> Result<(), Box<dyn std::error::Error>> {
    let rendered = codex_state
        .app
        .with_ref(frankie::tui::app::ReviewApp::view)
        .ok_or("app must be initialised")?;
    if !rendered.contains("Resume?") {
        return Err(format!("expected 'Resume?' in view, got:\n{rendered}").into());
    }

    Ok(())
}

#[then("the status bar contains {text}")]
fn then_status_contains(
    codex_state: &ResumeScenarioState,
    text: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let rendered = codex_state
        .app
        .with_ref(frankie::tui::app::ReviewApp::view)
        .ok_or("app must be initialised")?;
    let expected = text.trim_matches('"');
    if !rendered.contains(expected) {
        return Err(format!("expected status text '{expected}', got:\n{rendered}").into());
    }

    Ok(())
}

#[then("no TUI error is shown")]
fn then_no_error(codex_state: &ResumeScenarioState) -> Result<(), Box<dyn std::error::Error>> {
    let has_error = codex_state
        .app
        .with_ref(|app| app.error_message().is_some())
        .ok_or("app must be initialised")?;
    if has_error {
        let msg = codex_state
            .app
            .with_ref(|app| app.error_message().map(ToOwned::to_owned))
            .ok_or("app must be initialised")?;
        return Err(format!("expected no TUI error, but found: {msg:?}").into());
    }

    Ok(())
}

#[scenario(path = "tests/features/codex_session_resume.feature", index = 0)]
fn resume_prompt_is_shown(codex_state: ResumeScenarioState) {
    let _ = codex_state;
}

#[scenario(path = "tests/features/codex_session_resume.feature", index = 1)]
fn accepting_resume_starts_resumed_execution(codex_state: ResumeScenarioState) {
    let _ = codex_state;
}

#[scenario(path = "tests/features/codex_session_resume.feature", index = 2)]
fn declining_resume_starts_fresh_execution(codex_state: ResumeScenarioState) {
    let _ = codex_state;
}

#[scenario(path = "tests/features/codex_session_resume.feature", index = 3)]
fn no_resume_prompt_without_interrupted_session(codex_state: ResumeScenarioState) {
    let _ = codex_state;
}
