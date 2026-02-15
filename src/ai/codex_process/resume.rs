//! Resume execution path for interrupted Codex sessions.
//!
//! Manages the lifecycle of a resumed `codex app-server` session,
//! using the `thread/resume` JSON-RPC method to reconnect to a prior
//! server-side thread.

use std::sync::mpsc::{self, Sender};

use camino::Utf8PathBuf;
use chrono::Utc;

use crate::ai::codex_exec::{CodexExecutionHandle, CodexExecutionUpdate};
use crate::ai::session::{SessionState, SessionStatus};
use crate::ai::transcript::TranscriptWriter;

use super::stream::{self, StreamCompletion, StreamProgressContext};
use super::{
    ProcessStreams, RunContext, finalize, send_failure, spawn_codex, take_io, update_session_status,
};

/// Context for a resumed Codex execution.
pub(crate) struct ResumeParams {
    /// Path to the `codex` executable.
    pub(crate) command_path: String,
    /// Prompt text including updated review comments.
    pub(crate) prompt: String,
    /// Server-side thread ID from the prior session.
    pub(crate) thread_id: String,
    /// Path to the existing transcript file.
    pub(crate) transcript_path: Utf8PathBuf,
    /// Session state from the interrupted run.
    pub(crate) session_state: SessionState,
}

/// Spawns a background thread that resumes a Codex session via
/// `thread/resume` and streams progress through the returned handle.
pub(crate) fn run_codex_resume(params: ResumeParams) -> CodexExecutionHandle {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        execute_resume(params, &sender);
    });

    CodexExecutionHandle::new(receiver)
}

fn execute_resume(mut params: ResumeParams, sender: &Sender<CodexExecutionUpdate>) {
    let mut transcript = match TranscriptWriter::open_append(&params.transcript_path) {
        Ok(writer) => writer,
        Err(error) => {
            send_failure(
                sender,
                format!("failed to open transcript for append: {error}"),
            );
            return;
        }
    };

    params.session_state.status = SessionStatus::Running;
    params.session_state.started_at = Utc::now();
    params.session_state.finished_at = None;
    let _sidecar_result = params.session_state.write_sidecar();

    let mut child = match spawn_codex(params.command_path.as_str()) {
        Ok(child) => child,
        Err(error) => {
            update_session_status(&mut params.session_state, SessionStatus::Failed);
            send_failure(sender, error.to_string());
            return;
        }
    };

    let (streams, stderr_capture) = match take_io(&mut child) {
        Ok(io) => io,
        Err(error) => {
            update_session_status(&mut params.session_state, SessionStatus::Failed);
            send_failure(sender, error.to_string());
            return;
        }
    };

    let mut run_ctx = RunContext {
        session_state: &mut params.session_state,
        sender,
        stderr_capture,
    };

    let input = ResumeStreamInput {
        thread_id: params.thread_id.as_str(),
        prompt: params.prompt.as_str(),
    };
    let completion = run_resume_stream(streams, &input, &mut transcript, &mut run_ctx);

    if let Some(outcome) = completion {
        finalize(&mut child, outcome, params.transcript_path, &mut run_ctx);
    }
}

/// Prompt and thread identity for a resume stream.
struct ResumeStreamInput<'a> {
    thread_id: &'a str,
    prompt: &'a str,
}

/// Runs the resume streaming loop, returning the completion on success.
fn run_resume_stream(
    streams: ProcessStreams,
    input: &ResumeStreamInput<'_>,
    transcript: &mut TranscriptWriter,
    run_ctx: &mut RunContext<'_>,
) -> Option<StreamCompletion> {
    let result = {
        let mut ctx = StreamProgressContext {
            prompt: input.prompt,
            transcript,
            sender: run_ctx.sender,
            thread_id: None,
        };
        let result = stream::stream_resume_progress(
            streams.stdout,
            streams.stdin,
            &mut ctx,
            input.thread_id,
        );
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
