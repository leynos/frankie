//! Unit tests for Codex execution services.

use std::thread;
use std::time::{Duration, Instant};

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::*;
use crate::ai::codex_process::{build_command_spec, parse_progress_event};
use crate::export::{ExportedComment, write_jsonl};

/// Result type used by Codex execution tests.
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[fixture]
fn sample_comment() -> ExportedComment {
    ExportedComment {
        id: 1,
        author: Some("alice".to_owned()),
        file_path: Some("src/lib.rs".to_owned()),
        line_number: Some(42),
        original_line_number: Some(40),
        body: Some("Please simplify this branch".to_owned()),
        diff_hunk: Some("@@ -1,2 +1,2 @@".to_owned()),
        commit_sha: Some("abc123".to_owned()),
        in_reply_to_id: None,
        created_at: Some("2026-02-12T10:00:00Z".to_owned()),
    }
}

#[fixture]
fn rendered_jsonl(sample_comment: ExportedComment) -> Result<String, IntakeError> {
    let mut buffer = Vec::new();
    write_jsonl(&mut buffer, &[sample_comment])?;
    String::from_utf8(buffer).map_err(|error| IntakeError::Io {
        message: format!("failed to encode JSONL as UTF-8: {error}"),
    })
}

fn collect_until_finished(handle: &CodexExecutionHandle) -> Vec<CodexExecutionUpdate> {
    let mut updates = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline {
        match handle.try_recv() {
            Ok(update) => {
                let is_finished = matches!(update, CodexExecutionUpdate::Finished(_));
                updates.push(update);
                if is_finished {
                    break;
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(10)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }
    }

    updates
}

#[fixture]
fn temp_dir() -> TempDir {
    TempDir::new().expect("failed to create temporary directory")
}

fn utf8_path_from_temp(temp_dir: &TempDir) -> Result<Utf8PathBuf, IntakeError> {
    Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).map_err(|_| IntakeError::Io {
        message: "temporary directory path is not valid UTF-8".to_owned(),
    })
}

fn create_script(
    temp_dir: &TempDir,
    name: &str,
    contents: &str,
) -> Result<Utf8PathBuf, IntakeError> {
    let path =
        Utf8PathBuf::from_path_buf(temp_dir.path().join(name)).map_err(|_| IntakeError::Io {
            message: "script path is not valid UTF-8".to_owned(),
        })?;

    let temp_utf8 =
        Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).map_err(|_| IntakeError::Io {
            message: "temporary directory path is not valid UTF-8".to_owned(),
        })?;
    let dir = Dir::open_ambient_dir(&temp_utf8, ambient_authority()).map_err(|error| {
        IntakeError::Io {
            message: format!("failed to open temp directory: {error}"),
        }
    })?;
    dir.write(name, contents).map_err(|error| IntakeError::Io {
        message: format!("failed to write script: {error}"),
    })?;

    #[cfg(unix)]
    {
        use cap_std::fs::PermissionsExt;

        let metadata = dir.metadata(name).map_err(|error| IntakeError::Io {
            message: format!("failed to stat script: {error}"),
        })?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        dir.set_permissions(name, permissions)
            .map_err(|error| IntakeError::Io {
                message: format!("failed to chmod script: {error}"),
            })?;
    }

    Ok(path)
}

#[rstest]
fn command_spec_includes_app_server_subcommand() {
    let spec = build_command_spec("codex");
    assert_eq!(spec.program, "codex");
    assert_eq!(spec.args, vec!["app-server"]);
}

#[rstest]
fn parse_progress_event_handles_json_and_non_json_lines() {
    let json_event = parse_progress_event(r#"{"type":"turn.started"}"#);
    assert_eq!(
        json_event,
        CodexProgressEvent::Status {
            message: "event: turn.started".to_owned(),
        }
    );

    let warning = parse_progress_event("not-json");
    assert_eq!(
        warning,
        CodexProgressEvent::ParseWarning {
            raw_line: "not-json".to_owned(),
        }
    );
}

#[rstest]
fn start_rejects_empty_comment_export() {
    let context = CodexExecutionContext::new("owner", "repo", 42);
    let request = CodexExecutionRequest::new(context, String::new(), None);
    let service = SystemCodexExecutionService::new();

    let error = service
        .start(request)
        .expect_err("empty comments should fail");
    assert!(matches!(error, IntakeError::Configuration { .. }));
}

#[rstest]
fn successful_run_streams_events_and_writes_transcript(
    temp_dir: TempDir,
    rendered_jsonl: Result<String, IntakeError>,
) -> TestResult {
    let script = create_script(
        &temp_dir,
        "codex-success.sh",
        concat!(
            "#!/bin/sh\n",
            "printf '%s\\n' '{\"type\":\"turn.started\"}'\n",
            "printf '%s\\n' '{\"type\":\"item.completed\"}'\n",
            "exit 0\n"
        ),
    )?;

    let context = CodexExecutionContext::new("owner", "repo", 42)
        .with_transcript_dir(utf8_path_from_temp(&temp_dir)?);
    let request = CodexExecutionRequest::new(
        context,
        rendered_jsonl?,
        Some("https://github.com/owner/repo/pull/42".to_owned()),
    );
    let service = SystemCodexExecutionService::with_command_path(script);

    let handle = service.start(request)?;
    let updates = collect_until_finished(&handle);

    if !updates
        .iter()
        .any(|update| matches!(update, CodexExecutionUpdate::Progress(_)))
    {
        return Err("expected at least one progress update".into());
    }

    let outcome = updates
        .iter()
        .find_map(|update| match update {
            CodexExecutionUpdate::Finished(outcome) => Some(outcome.clone()),
            CodexExecutionUpdate::Progress(_) => None,
        })
        .ok_or("expected finished update")?;

    let transcript_path = match outcome {
        CodexExecutionOutcome::Succeeded { transcript_path } => transcript_path,
        CodexExecutionOutcome::Failed { message, .. } => {
            return Err(format!("expected success, got failure: {message}").into());
        }
    };

    let parent = transcript_path
        .parent()
        .ok_or("transcript path has no parent")?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())?;
    let file_name = transcript_path
        .file_name()
        .ok_or("transcript path has no file name")?;
    let transcript = dir.read_to_string(file_name)?;

    if !transcript.contains("turn.started") {
        return Err("transcript missing turn.started".into());
    }
    if !transcript.contains("item.completed") {
        return Err("transcript missing item.completed".into());
    }

    Ok(())
}

#[rstest]
fn non_zero_exit_is_reported_with_exit_code(
    temp_dir: TempDir,
    rendered_jsonl: Result<String, IntakeError>,
) -> TestResult {
    let script = create_script(
        &temp_dir,
        "codex-fail.sh",
        concat!(
            "#!/bin/sh\n",
            "printf '%s\\n' '{\"type\":\"turn.started\"}'\n",
            "exit 9\n"
        ),
    )?;

    let context = CodexExecutionContext::new("owner", "repo", 42)
        .with_transcript_dir(utf8_path_from_temp(&temp_dir)?);
    let request = CodexExecutionRequest::new(context, rendered_jsonl?, None);
    let service = SystemCodexExecutionService::with_command_path(script);

    let handle = service.start(request)?;
    let updates = collect_until_finished(&handle);

    let outcome = updates
        .iter()
        .find_map(|update| match update {
            CodexExecutionUpdate::Finished(outcome) => Some(outcome.clone()),
            CodexExecutionUpdate::Progress(_) => None,
        })
        .ok_or("expected finished update")?;

    match outcome {
        CodexExecutionOutcome::Failed {
            exit_code,
            transcript_path,
            ..
        } => {
            if exit_code != Some(9) {
                return Err(format!("expected exit code 9, got {exit_code:?}").into());
            }
            if transcript_path.is_none() {
                return Err("expected transcript path to be present".into());
            }
        }
        CodexExecutionOutcome::Succeeded { .. } => {
            return Err("expected non-zero exit failure".into());
        }
    }

    Ok(())
}
