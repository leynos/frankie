//! Process execution helpers for Codex integration.
//!
//! Manages the lifecycle of a `codex app-server` child process:
//! spawning, stdin/stdout plumbing, stream polling, and graceful
//! termination.

mod app_server;
mod resume;
mod stderr;
mod stream;

use std::fmt::Write as _;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Sender};

use camino::Utf8PathBuf;
use chrono::Utc;

use crate::ai::codex_exec::{
    CodexExecutionHandle, CodexExecutionOutcome, CodexExecutionRequest, CodexExecutionUpdate,
};
use crate::ai::session::{SessionState, SessionStatus};

use super::transcript::TranscriptWriter;

use self::app_server::AppServerCompletion;
use self::stderr::StderrCapture;
use self::stream::{StreamCompletion, StreamProgressContext};

pub(crate) use self::resume::{ResumeParams, run_codex_resume};

#[cfg(test)]
pub(crate) use self::stream::parse_progress_event;

/// Error arising during a Codex child-process run.
///
/// Wraps a human-readable description of the failure so callers can
/// forward it through the TUI channel without matching on variants.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct RunError(String);

impl RunError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Command name and arguments used to launch the Codex process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexCommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

/// Builds the command specification for `codex app-server`.
pub(crate) fn build_command_spec(command_path: &str) -> CodexCommandSpec {
    CodexCommandSpec {
        program: command_path.to_owned(),
        args: vec!["app-server".to_owned()],
    }
}

/// Tracks the channel sender and session state for an execution run.
struct RunContext<'a> {
    session_state: &'a mut SessionState,
    sender: &'a Sender<CodexExecutionUpdate>,
    stderr_capture: StderrCapture,
}

/// Bundles I/O streams taken from a spawned child process.
struct ProcessStreams {
    stdout: ChildStdout,
    stdin: Option<ChildStdin>,
}

/// Spawns a background thread that executes Codex and streams progress
/// updates through the returned handle.
pub(crate) fn run_codex(
    command_path: String,
    request: CodexExecutionRequest,
    transcript_path: Utf8PathBuf,
) -> CodexExecutionHandle {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        execute_codex(command_path.as_str(), &request, transcript_path, &sender);
    });

    CodexExecutionHandle::new(receiver)
}

fn spawn_codex(command_path: &str) -> Result<Child, RunError> {
    let command_spec = build_command_spec(command_path);
    let mut command = Command::new(&command_spec.program);
    command
        .args(&command_spec.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    command
        .spawn()
        .map_err(|error| RunError::new(format!("failed to launch Codex: {error}")))
}

/// Takes I/O handles from a spawned child process.
fn take_io(child: &mut Child) -> Result<(ProcessStreams, StderrCapture), RunError> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RunError::new("codex stdout stream was unavailable"))?;
    let stdin = child.stdin.take();
    let stderr_capture = StderrCapture::spawn(child.stderr.take());
    Ok((ProcessStreams { stdout, stdin }, stderr_capture))
}

fn execute_codex(
    command_path: &str,
    request: &CodexExecutionRequest,
    transcript_path: Utf8PathBuf,
    sender: &Sender<CodexExecutionUpdate>,
) {
    let mut transcript = match TranscriptWriter::create(&transcript_path) {
        Ok(writer) => writer,
        Err(error) => {
            send_failure(sender, format!("failed to create transcript: {error}"));
            return;
        }
    };

    let mut session_state = SessionState {
        status: SessionStatus::Running,
        transcript_path: transcript_path.clone(),
        thread_id: None,
        owner: request.context().owner().to_owned(),
        repository: request.context().repository().to_owned(),
        pr_number: request.context().pr_number(),
        started_at: Utc::now(),
        finished_at: None,
    };
    let _sidecar_result = session_state.write_sidecar();

    let mut child = match spawn_codex(command_path) {
        Ok(child) => child,
        Err(error) => {
            update_session_status(&mut session_state, SessionStatus::Failed);
            send_failure(sender, error.to_string());
            return;
        }
    };

    let (streams, stderr_capture) = match take_io(&mut child) {
        Ok(io) => io,
        Err(error) => {
            update_session_status(&mut session_state, SessionStatus::Failed);
            send_failure(sender, error.to_string());
            return;
        }
    };

    let mut run_ctx = RunContext {
        session_state: &mut session_state,
        sender,
        stderr_capture,
    };

    let prompt = build_prompt(request);
    let completion = run_stream(streams, &prompt, &mut transcript, &mut run_ctx);

    if let Some(outcome) = completion {
        finalize(&mut child, outcome, transcript_path, &mut run_ctx);
    }
}

/// Runs the streaming loop, returning the completion when successful.
fn run_stream(
    streams: ProcessStreams,
    prompt: &str,
    transcript: &mut TranscriptWriter,
    run_ctx: &mut RunContext<'_>,
) -> Option<StreamCompletion> {
    let result = {
        let mut ctx = StreamProgressContext {
            prompt,
            transcript,
            sender: run_ctx.sender,
            thread_id: None,
        };
        let result = stream::stream_progress(streams.stdout, streams.stdin, &mut ctx);
        if let Some(tid) = ctx.thread_id.take() {
            run_ctx.session_state.thread_id = Some(tid);
        }
        result
    };

    let outcome = match result {
        Ok(completion) => completion,
        Err(error) => {
            update_session_status(run_ctx.session_state, SessionStatus::Interrupted);
            send_failure(
                run_ctx.sender,
                run_ctx.stderr_capture.append_to(error.to_string()),
            );
            return None;
        }
    };

    if let Err(error) = transcript.flush() {
        update_session_status(run_ctx.session_state, SessionStatus::Failed);
        send_failure(
            run_ctx.sender,
            run_ctx
                .stderr_capture
                .append_to(format!("failed to flush transcript: {error}")),
        );
        return None;
    }

    Some(outcome)
}

/// Resolves the terminal session status and sends the final outcome.
fn finalize(
    child: &mut Child,
    completion: StreamCompletion,
    transcript_path: Utf8PathBuf,
    run_ctx: &mut RunContext<'_>,
) {
    match completion {
        StreamCompletion::AppServer(ref outcome) => {
            let terminal_status = session_status_from_completion(outcome);
            update_session_status(run_ctx.session_state, terminal_status);
            terminate_child(child);
            send_app_server_outcome(completion, transcript_path, run_ctx);
        }
        StreamCompletion::ProcessExit => {
            update_session_status(run_ctx.session_state, SessionStatus::Interrupted);
            complete_with_exit(child, transcript_path, run_ctx);
        }
    }
}

fn send_app_server_outcome(
    outcome: StreamCompletion,
    transcript_path: Utf8PathBuf,
    run_ctx: &mut RunContext<'_>,
) {
    let StreamCompletion::AppServer(app_outcome) = outcome else {
        return;
    };
    match app_outcome {
        AppServerCompletion::Succeeded => {
            send_success(run_ctx.sender, transcript_path);
        }
        AppServerCompletion::Failed { message, .. } => {
            send_failure(run_ctx.sender, run_ctx.stderr_capture.append_to(message));
        }
    }
}

fn complete_with_exit(
    child: &mut Child,
    transcript_path: Utf8PathBuf,
    run_ctx: &mut RunContext<'_>,
) {
    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => {
            send_failure(
                run_ctx.sender,
                run_ctx
                    .stderr_capture
                    .append_to(format!("failed waiting for Codex exit: {error}")),
            );
            return;
        }
    };

    if status.success() {
        send_success(run_ctx.sender, transcript_path);
        return;
    }

    let message = run_ctx
        .stderr_capture
        .append_to("codex exited with a non-zero status".to_owned());
    drop(run_ctx.sender.send(CodexExecutionUpdate::Finished(
        CodexExecutionOutcome::Failed {
            message,
            exit_code: status.code(),
            transcript_path: Some(transcript_path),
        },
    )));
}

fn log_termination_error(action: &str, error: &std::io::Error) {
    tracing::trace!("failed to {action} Codex child process: {error}");
}

fn terminate_child(child: &mut Child) {
    if let Ok(None) = child.try_wait() {
        if let Err(error) = child.kill() {
            log_termination_error("kill", &error);
        }
        if let Err(error) = child.wait() {
            log_termination_error("wait for", &error);
        }
    }
}

fn send_success(sender: &Sender<CodexExecutionUpdate>, transcript_path: Utf8PathBuf) {
    drop(sender.send(CodexExecutionUpdate::Finished(
        CodexExecutionOutcome::Succeeded { transcript_path },
    )));
}

fn send_failure(sender: &Sender<CodexExecutionUpdate>, message: String) {
    drop(sender.send(CodexExecutionUpdate::Finished(
        CodexExecutionOutcome::Failed {
            message,
            exit_code: None,
            transcript_path: None,
        },
    )));
}

/// Maps an app-server completion to a session status.
const fn session_status_from_completion(completion: &AppServerCompletion) -> SessionStatus {
    match completion {
        AppServerCompletion::Succeeded => SessionStatus::Completed,
        AppServerCompletion::Failed {
            interrupted: true, ..
        } => SessionStatus::Interrupted,
        AppServerCompletion::Failed {
            interrupted: false, ..
        } => SessionStatus::Failed,
    }
}

/// Updates session state to a terminal status and persists the sidecar.
fn update_session_status(state: &mut SessionState, status: SessionStatus) {
    state.status = status;
    state.finished_at = Some(Utc::now());
    let _sidecar_result = state.write_sidecar();
}

fn build_prompt(request: &CodexExecutionRequest) -> String {
    let mut prompt = format!(
        concat!(
            "Resolve review comments for pull request {}/{} #{}.",
            "\nSummarise key changes and apply fixes where safe.",
            "\nReview comments (JSONL):\n"
        ),
        request.context().owner(),
        request.context().repository(),
        request.context().pr_number(),
    );

    if let Some(pr_url) = request.pr_url() {
        let _infallible = writeln!(prompt, "Pull request URL: {pr_url}");
    }

    prompt.push_str(request.comments_jsonl());
    prompt
}
