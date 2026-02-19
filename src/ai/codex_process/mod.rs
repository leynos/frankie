//! Process execution helpers for Codex integration.

mod app_server;
mod resume;
mod stderr;
mod stream;
mod termination;

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
use self::termination::terminate_child;

pub(crate) use self::resume::{ResumeParams, run_codex_resume};

#[cfg(test)]
pub(crate) use self::stream::parse_progress_event;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct RunError(String);

impl RunError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexCommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

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

fn execute_codex(
    command_path: &str,
    request: &CodexExecutionRequest,
    transcript_path: Utf8PathBuf,
    sender: &Sender<CodexExecutionUpdate>,
) {
    let mut transcript = match TranscriptWriter::create(&transcript_path) {
        Ok(writer) => writer,
        Err(error) => {
            send_failure_with_details(
                sender,
                format!("failed to create transcript: {error}"),
                None,
                Some(transcript_path.clone()),
            );
            return;
        }
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
    }
}

struct StreamRunInput<'a> {
    stdout: ChildStdout,
    stdin: Option<ChildStdin>,
    prompt: &'a str,
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
        if let Some(tid) = ctx.thread_id.take() {
            run_ctx.session_state.thread_id = Some(tid);
        }
        result
    };

    let outcome = match result {
        Ok(completion) => completion,
        Err(error) => {
            update_session_status(run_ctx.session_state, SessionStatus::Interrupted);
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

fn finalize(
    child: &mut Child,
    completion: StreamCompletion,
    transcript_path: Utf8PathBuf,
    run_ctx: &mut RunContext<'_>,
) {
    match completion {
        StreamCompletion::AppServer(outcome) => {
            let terminal_status = session_status_from_completion(&outcome);
            update_session_status(run_ctx.session_state, terminal_status);
            terminate_child(child);
            send_app_server_outcome(outcome, transcript_path, run_ctx);
        }
        StreamCompletion::ProcessExit => {
            complete_with_exit(child, transcript_path, run_ctx);
        }
    }
}

fn send_app_server_outcome(
    outcome: AppServerCompletion,
    transcript_path: Utf8PathBuf,
    run_ctx: &mut RunContext<'_>,
) {
    match outcome {
        AppServerCompletion::Succeeded => {
            send_success(run_ctx.sender, transcript_path);
        }
        AppServerCompletion::Failed { message, .. } => {
            send_failure_with_details(
                run_ctx.sender,
                run_ctx.stderr_capture.append_to(message),
                None,
                Some(run_ctx.session_state.transcript_path.clone()),
            );
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
            update_session_status(run_ctx.session_state, SessionStatus::Failed);
            send_failure_with_details(
                run_ctx.sender,
                run_ctx
                    .stderr_capture
                    .append_to(format!("failed waiting for Codex exit: {error}")),
                None,
                Some(transcript_path),
            );
            return;
        }
    };

    if status.success() {
        update_session_status(run_ctx.session_state, SessionStatus::Completed);
        send_success(run_ctx.sender, transcript_path);
        return;
    }

    update_session_status(run_ctx.session_state, SessionStatus::Failed);
    let message = run_ctx
        .stderr_capture
        .append_to("codex exited with a non-zero status".to_owned());
    send_failure_with_details(
        run_ctx.sender,
        message,
        status.code(),
        Some(transcript_path),
    );
}

fn build_running_session_state(
    request: &CodexExecutionRequest,
    transcript_path: &Utf8PathBuf,
) -> SessionState {
    SessionState {
        status: SessionStatus::Running,
        transcript_path: transcript_path.clone(),
        thread_id: None,
        owner: request.context().owner().to_owned(),
        repository: request.context().repository().to_owned(),
        pr_number: request.context().pr_number(),
        started_at: Utc::now(),
        finished_at: None,
    }
}

fn send_success(sender: &Sender<CodexExecutionUpdate>, transcript_path: Utf8PathBuf) {
    drop(sender.send(CodexExecutionUpdate::Finished(
        CodexExecutionOutcome::Succeeded { transcript_path },
    )));
}

pub(super) fn send_failure_with_details(
    sender: &Sender<CodexExecutionUpdate>,
    message: String,
    exit_code: Option<i32>,
    transcript_path: Option<Utf8PathBuf>,
) {
    drop(sender.send(CodexExecutionUpdate::Finished(
        CodexExecutionOutcome::Failed {
            message,
            exit_code,
            transcript_path,
        },
    )));
}

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

pub(super) fn update_session_status(state: &mut SessionState, status: SessionStatus) {
    state.status = status;
    state.finished_at = Some(Utc::now());
    if let Err(error) = state.write_sidecar() {
        let detail = error.to_string();
        log_sidecar_write_error(state.sidecar_path().as_str(), detail.as_str());
    }
}

fn log_sidecar_write_error(sidecar_path: &str, error: &str) {
    tracing::warn!("failed to write session sidecar '{sidecar_path}': {error}");
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
