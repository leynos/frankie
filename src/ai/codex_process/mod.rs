//! Process execution helpers for Codex integration.

mod app_server;
mod finalise;
mod messaging;
mod resume;
mod session;
mod stderr;
mod stream;
mod termination;

use std::fmt::Write as _;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Sender};

use camino::Utf8PathBuf;

use crate::ai::codex_exec::{CodexExecutionHandle, CodexExecutionRequest, CodexExecutionUpdate};
use crate::ai::session::{SessionState, SessionStatus};

use super::transcript::TranscriptWriter;

use self::finalise::finalize;
use self::messaging::send_failure_with_details;
use self::session::{build_running_session_state, log_sidecar_write_error, update_session_status};
use self::stderr::StderrCapture;
use self::stream::{StreamCompletion, StreamProgressContext};
use self::termination::terminate_child;

pub(crate) use self::resume::{ResumeParams, run_codex_resume};

#[cfg(test)]
pub(crate) use self::stream::parse_progress_event;

/// Error returned by Codex process execution helpers.
///
/// Carries a human-readable `message` and a `kind` used to distinguish
/// interruption flows from hard failures.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(super) struct RunError {
    /// Human-readable failure detail surfaced to callers and UI updates.
    message: String,
    /// Classification used to drive interruption-aware handling paths.
    kind: RunErrorKind,
}

/// Classifies execution failures for control-flow decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunErrorKind {
    /// The run was intentionally interrupted and can be surfaced as such.
    Interruption,
    /// A non-interruption execution failure.
    HardFailure,
}

impl RunError {
    /// Creates a hard-failure [`RunError`] with the provided message.
    ///
    /// Parameters:
    /// - `message`: failure detail convertible to `String`.
    ///
    /// Returns:
    /// - A [`RunError`] with kind [`RunErrorKind::HardFailure`].
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: RunErrorKind::HardFailure,
        }
    }

    /// Creates an interruption [`RunError`] with the provided message.
    ///
    /// Parameters:
    /// - `message`: interruption detail convertible to `String`.
    ///
    /// Returns:
    /// - A [`RunError`] with kind [`RunErrorKind::Interruption`].
    pub(super) fn interruption(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: RunErrorKind::Interruption,
        }
    }

    /// Returns `true` when the error kind is [`RunErrorKind::Interruption`].
    pub(super) const fn is_interruption(&self) -> bool {
        matches!(self.kind, RunErrorKind::Interruption)
    }
}

/// Command-line specification for spawning Codex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexCommandSpec {
    /// Executable path used to spawn the Codex process.
    pub(crate) program: String,
    /// Default argument vector passed to the executable.
    pub(crate) args: Vec<String>,
}

/// Builds the command program and default arguments for a Codex run.
///
/// Parameters:
/// - `command_path`: filesystem path (or command name) used as the executable.
///
/// Returns:
/// - A [`CodexCommandSpec`] containing `program = command_path` and the default
///   `app-server` subcommand argument.
pub(crate) fn build_command_spec(command_path: &str) -> CodexCommandSpec {
    CodexCommandSpec {
        program: command_path.to_owned(),
        args: vec!["app-server".to_owned()],
    }
}

struct RunContext<'a> {
    session_state: &'a mut SessionState,
    sender: &'a Sender<CodexExecutionUpdate>,
    stderr_capture: StderrCapture,
}

/// Starts Codex execution on a background thread and returns a handle.
///
/// Parameters:
/// - `command_path`: executable path used to launch Codex.
/// - `request`: execution payload consumed by the spawned worker thread.
/// - `transcript_path`: output path for the run transcript.
///
/// Returns:
/// - A [`CodexExecutionHandle`] backed by a channel receiver for streaming
///   [`CodexExecutionUpdate`] values from the worker thread.
///
/// Behaviour:
/// - Creates an `mpsc` channel.
/// - Spawns a thread that runs `execute_codex` and publishes updates via the
///   channel sender.
/// - Interruption semantics are preserved through [`RunError::interruption`]
///   within the execution pipeline.
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

fn take_io(
    child: &mut Child,
) -> Result<(ChildStdout, Option<ChildStdin>, StderrCapture), RunError> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RunError::new("codex stdout stream was unavailable"))?;
    let stdin = child.stdin.take();
    let stderr_capture = StderrCapture::spawn(child.stderr.take());
    Ok((stdout, stdin, stderr_capture))
}

/// Creates a new transcript file, handling errors by sending
/// a failure update and returning None.
fn prepare_transcript_for_execution(
    transcript_path: &Utf8PathBuf,
    sender: &Sender<CodexExecutionUpdate>,
) -> Option<TranscriptWriter> {
    match TranscriptWriter::create(transcript_path) {
        Ok(writer) => Some(writer),
        Err(error) => {
            send_failure_with_details(
                sender,
                format!("failed to create transcript: {error}"),
                None,
                Some(transcript_path.clone()),
            );
            None
        }
    }
}

fn execute_codex(
    command_path: &str,
    request: &CodexExecutionRequest,
    transcript_path: Utf8PathBuf,
    sender: &Sender<CodexExecutionUpdate>,
) {
    let Some(mut transcript) = prepare_transcript_for_execution(&transcript_path, sender) else {
        return;
    };

    let mut session_state = build_running_session_state(request, &transcript_path);
    if let Err(error) = session_state.write_sidecar() {
        let detail = error.to_string();
        log_sidecar_write_error(session_state.sidecar_path().as_str(), detail.as_str());
    }

    let mut child = match spawn_codex(command_path) {
        Ok(child) => child,
        Err(error) => {
            update_session_status(&mut session_state, SessionStatus::Failed);
            send_failure_with_details(
                sender,
                error.to_string(),
                None,
                Some(transcript_path.clone()),
            );
            return;
        }
    };

    let (stdout, stdin, stderr_capture) = match take_io(&mut child) {
        Ok(io) => io,
        Err(error) => {
            terminate_child(&mut child);
            update_session_status(&mut session_state, SessionStatus::Failed);
            send_failure_with_details(
                sender,
                error.to_string(),
                None,
                Some(transcript_path.clone()),
            );
            return;
        }
    };

    let mut run_ctx = RunContext {
        session_state: &mut session_state,
        sender,
        stderr_capture,
    };

    let prompt = build_prompt(request);
    let completion = run_stream_with_context(
        StreamRunInput {
            stdout,
            stdin,
            prompt: &prompt,
        },
        &mut transcript,
        &mut run_ctx,
        stream::stream_progress,
    );

    if let Some(outcome) = completion {
        finalize(&mut child, outcome, transcript_path, &mut run_ctx);
    } else {
        terminate_child(&mut child);
    }
}

struct StreamRunInput<'a> {
    stdout: ChildStdout,
    stdin: Option<ChildStdin>,
    prompt: &'a str,
}

/// Captures the thread ID from the streaming context and persists it to the
/// session sidecar, logging any write errors.
fn capture_and_persist_thread_id(
    ctx: &mut StreamProgressContext<'_>,
    run_ctx: &mut RunContext<'_>,
) {
    let Some(tid) = ctx.thread_id.take() else {
        return;
    };

    run_ctx.session_state.thread_id = Some(tid);
    if let Err(error) = run_ctx.session_state.write_sidecar() {
        let detail = error.to_string();
        log_sidecar_write_error(
            run_ctx.session_state.sidecar_path().as_str(),
            detail.as_str(),
        );
    }
}

fn run_stream_with_context<F>(
    input: StreamRunInput<'_>,
    transcript: &mut TranscriptWriter,
    run_ctx: &mut RunContext<'_>,
    stream_fn: F,
) -> Option<StreamCompletion>
where
    F: FnOnce(
        ChildStdout,
        Option<ChildStdin>,
        &mut StreamProgressContext<'_>,
    ) -> Result<StreamCompletion, RunError>,
{
    let result = {
        let mut ctx = StreamProgressContext {
            prompt: input.prompt,
            transcript,
            sender: run_ctx.sender,
            thread_id: None,
        };
        let result = stream_fn(input.stdout, input.stdin, &mut ctx);
        capture_and_persist_thread_id(&mut ctx, run_ctx);
        result
    };

    let outcome = match result {
        Ok(completion) => completion,
        Err(error) => {
            let status = if error.is_interruption() {
                SessionStatus::Interrupted
            } else {
                SessionStatus::Failed
            };
            update_session_status(run_ctx.session_state, status);
            send_failure_with_details(
                run_ctx.sender,
                run_ctx.stderr_capture.append_to(error.to_string()),
                None,
                Some(run_ctx.session_state.transcript_path.clone()),
            );
            return None;
        }
    };

    if let Err(error) = transcript.flush() {
        update_session_status(run_ctx.session_state, SessionStatus::Failed);
        send_failure_with_details(
            run_ctx.sender,
            run_ctx
                .stderr_capture
                .append_to(format!("failed to flush transcript: {error}")),
            None,
            Some(run_ctx.session_state.transcript_path.clone()),
        );
        return None;
    }

    Some(outcome)
}

fn build_prompt(request: &CodexExecutionRequest) -> String {
    let mut prompt = format!(
        concat!(
            "Resolve review comments for pull request {}/{} #{}.",
            "\nSummarize key changes and apply fixes where safe.",
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
