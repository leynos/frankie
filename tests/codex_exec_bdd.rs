//! Behavioural tests for Codex execution integration.

#[path = "codex_exec_bdd/mod.rs"]
mod codex_exec_bdd_support;

use bubbletea_rs::Model;
use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use codex_exec_bdd_support::{CodexExecState, StubPlan, app_with_plan};
use frankie::ai::{CodexExecutionOutcome, CodexExecutionUpdate, CodexProgressEvent};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn codex_state() -> CodexExecState {
    CodexExecState::default()
}

/// Helper to set up a Codex execution scenario with a transcript file.
/// The `plan_builder` closure receives the transcript path and returns the plan.
fn setup_codex_scenario_with_plan<F>(
    codex_state: &CodexExecState,
    filename: &str,
    initial_content: &str,
    plan_builder: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(&str) -> StubPlan,
{
    let temp_dir = tempfile::TempDir::new()?;
    let temp_utf8 = camino::Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .map_err(|_| "temporary directory path is not valid UTF-8")?;
    let dir = Dir::open_ambient_dir(&temp_utf8, ambient_authority())?;
    dir.write(filename, initial_content)?;

    let transcript_path_str = temp_utf8.join(filename).to_string();

    let plan = plan_builder(&transcript_path_str);

    codex_state.app.set(app_with_plan(plan)?);
    codex_state.temp_dir.set(temp_dir);
    codex_state.transcript_path.set(transcript_path_str);
    Ok(())
}

/// Helper to build a plan with a single progress event followed by successful completion.
fn plan_with_progress_then_success(
    progress_event: CodexProgressEvent,
    success_delay_ms: u64,
    transcript_path: &str,
) -> StubPlan {
    StubPlan::TimedUpdates(vec![
        (0, CodexExecutionUpdate::Progress(progress_event)),
        (
            success_delay_ms,
            CodexExecutionUpdate::Finished(CodexExecutionOutcome::Succeeded {
                transcript_path: camino::Utf8PathBuf::from(transcript_path),
            }),
        ),
    ])
}

#[given("a Codex run that streams progress and completes successfully")]
fn given_successful_run(codex_state: &CodexExecState) -> Result<(), Box<dyn std::error::Error>> {
    setup_codex_scenario_with_plan(
        codex_state,
        "success.jsonl",
        "{\"type\":\"turn.started\"}\n",
        |transcript_path| {
            plan_with_progress_then_success(
                CodexProgressEvent::Status {
                    message: "event: turn.started".to_owned(),
                },
                120,
                transcript_path,
            )
        },
    )
}

#[given("a Codex run that exits non-zero with transcript")]
fn given_non_zero_exit(codex_state: &CodexExecState) -> Result<(), Box<dyn std::error::Error>> {
    setup_codex_scenario_with_plan(
        codex_state,
        "failure.jsonl",
        "{\"type\":\"turn.started\"}\n",
        |transcript_path| {
            StubPlan::TimedUpdates(vec![(
                40,
                CodexExecutionUpdate::Finished(CodexExecutionOutcome::Failed {
                    message: "codex exited with a non-zero status".to_owned(),
                    exit_code: Some(17),
                    transcript_path: Some(camino::Utf8PathBuf::from(transcript_path)),
                }),
            )])
        },
    )
}

#[given("a Codex run that emits a malformed stream line")]
fn given_malformed_line(codex_state: &CodexExecState) -> Result<(), Box<dyn std::error::Error>> {
    setup_codex_scenario_with_plan(
        codex_state,
        "malformed.jsonl",
        "not-json\n",
        |transcript_path| {
            plan_with_progress_then_success(
                CodexProgressEvent::ParseWarning {
                    raw_line: "not-json".to_owned(),
                },
                200,
                transcript_path,
            )
        },
    )
}

#[given("a Codex run that fails because transcript writing failed")]
fn given_transcript_failure(
    codex_state: &CodexExecState,
) -> Result<(), Box<dyn std::error::Error>> {
    let plan = StubPlan::TimedUpdates(vec![(
        40,
        CodexExecutionUpdate::Finished(CodexExecutionOutcome::Failed {
            message: "failed to write transcript: permission denied".to_owned(),
            exit_code: None,
            transcript_path: None,
        }),
    )]);

    codex_state.app.set(app_with_plan(plan)?);
    Ok(())
}

#[when("Codex execution is started from the review TUI")]
fn when_start_codex(codex_state: &CodexExecState) -> Result<(), Box<dyn std::error::Error>> {
    codex_state
        .app
        .with_mut(|app| {
            app.handle_message(&frankie::tui::messages::AppMsg::StartCodexExecution);
        })
        .ok_or("app must be initialised")?;
    Ok(())
}

#[when("the Codex poll tick is processed")]
fn when_poll_tick(codex_state: &CodexExecState) -> Result<(), Box<dyn std::error::Error>> {
    codex_state
        .app
        .with_mut(|app| {
            app.handle_message(&frankie::tui::messages::AppMsg::CodexPollTick);
        })
        .ok_or("app must be initialised")?;
    Ok(())
}

#[when("I wait {millis:u64} milliseconds")]
fn when_wait_ms(millis: u64) {
    std::thread::sleep(std::time::Duration::from_millis(millis));
}

#[then("the status bar contains {text}")]
fn then_status_contains(
    codex_state: &CodexExecState,
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
fn then_no_error(codex_state: &CodexExecState) -> Result<(), Box<dyn std::error::Error>> {
    let has_error = codex_state
        .app
        .with_ref(|app| app.error_message().is_some())
        .ok_or("app must be initialised")?;
    if has_error {
        return Err("expected no TUI error".into());
    }

    Ok(())
}

#[then("the TUI error contains {text}")]
fn then_error_contains(
    codex_state: &CodexExecState,
    text: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected = text.trim_matches('"');
    let error_text = codex_state
        .app
        .with_ref(|app| app.error_message().map(ToOwned::to_owned))
        .ok_or("app must be initialised")?
        .ok_or("expected TUI error")?;

    if !error_text.contains(expected) {
        return Err(format!("expected error to contain '{expected}', got '{error_text}'").into());
    }

    Ok(())
}

#[then("the transcript file exists")]
fn then_transcript_exists(codex_state: &CodexExecState) -> Result<(), Box<dyn std::error::Error>> {
    let path = codex_state
        .transcript_path
        .with_ref(Clone::clone)
        .ok_or("expected transcript path")?;
    let utf8_path = Utf8Path::new(&path);
    let parent = utf8_path.parent().ok_or("transcript path has no parent")?;
    let file_name = utf8_path
        .file_name()
        .ok_or("transcript path has no file name")?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())?;
    if !dir.exists(file_name) {
        return Err(format!("expected transcript file '{path}' to exist").into());
    }

    Ok(())
}

#[then("the transcript file contains {text}")]
fn then_transcript_contains(
    codex_state: &CodexExecState,
    text: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = codex_state
        .transcript_path
        .with_ref(Clone::clone)
        .ok_or("expected transcript path")?;
    let expected = text.trim_matches('"');
    let utf8_path = Utf8Path::new(&path);
    let parent = utf8_path.parent().ok_or("transcript path has no parent")?;
    let file_name = utf8_path
        .file_name()
        .ok_or("transcript path has no file name")?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())?;
    let content = dir.read_to_string(file_name)?;

    if !content.contains(expected) {
        return Err(format!("expected transcript content to contain '{expected}'").into());
    }

    Ok(())
}

#[scenario(path = "tests/features/codex_exec.feature", index = 0)]
fn codex_success_streams_and_persists(codex_state: CodexExecState) {
    let _ = codex_state;
}

#[scenario(path = "tests/features/codex_exec.feature", index = 1)]
fn codex_non_zero_is_surfaced(codex_state: CodexExecState) {
    let _ = codex_state;
}

#[scenario(path = "tests/features/codex_exec.feature", index = 2)]
fn codex_malformed_stream_line(codex_state: CodexExecState) {
    let _ = codex_state;
}

#[scenario(path = "tests/features/codex_exec.feature", index = 3)]
fn codex_transcript_failure_is_surfaced(codex_state: CodexExecState) {
    let _ = codex_state;
}
