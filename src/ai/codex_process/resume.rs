//! Resume execution path for interrupted Codex sessions.
//!
//! Manages the lifecycle of a resumed `codex app-server` session,
//! using the `thread/resume` JSON-RPC method to reconnect to a prior
//! server-side thread.

use std::sync::mpsc::{self, Sender};

use camino::Utf8PathBuf;

use crate::ai::codex_exec::{CodexExecutionHandle, CodexExecutionUpdate};
use crate::ai::session::{SessionState, SessionStatus};
use crate::ai::transcript::TranscriptWriter;

use super::stream;
use super::{
    RunContext, StreamRunInput, finalize, run_stream_with_context, send_failure_with_details,
    spawn_codex, take_io, update_session_status,
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

/// Opens the transcript file in append mode, handling errors by sending
/// a failure update and returning None.
fn prepare_transcript_for_resume(
    transcript_path: &Utf8PathBuf,
    sender: &Sender<CodexExecutionUpdate>,
) -> Option<TranscriptWriter> {
    match TranscriptWriter::open_append(transcript_path) {
        Ok(writer) => Some(writer),
        Err(error) => {
            send_failure_with_details(
                sender,
                format!("failed to open transcript for append: {error}"),
                None,
                Some(transcript_path.clone()),
            );
            None
        }
    }
}

/// Updates the session state to Running and persists the sidecar,
/// handling errors by sending a failure update and returning false.
fn prepare_session_for_resume(
    session_state: &mut SessionState,
    transcript_path: &Utf8PathBuf,
    sender: &Sender<CodexExecutionUpdate>,
) -> bool {
    session_state.status = SessionStatus::Running;
    session_state.finished_at = None;
    if let Err(error) = session_state.write_sidecar() {
        send_failure_with_details(
            sender,
            format!("failed to persist resumed session state: {error}"),
            None,
            Some(transcript_path.clone()),
        );
        return false;
    }
    true
}

fn execute_resume(mut params: ResumeParams, sender: &Sender<CodexExecutionUpdate>) {
    let Some(mut transcript) = prepare_transcript_for_resume(&params.transcript_path, sender)
    else {
        return;
    };

    if !prepare_session_for_resume(&mut params.session_state, &params.transcript_path, sender) {
        return;
    }

    let mut child = match spawn_codex(params.command_path.as_str()) {
        Ok(child) => child,
        Err(error) => {
            update_session_status(&mut params.session_state, SessionStatus::Failed);
            send_failure_with_details(
                sender,
                error.to_string(),
                None,
                Some(params.transcript_path.clone()),
            );
            return;
        }
    };

    let (stdout, stdin, stderr_capture) = match take_io(&mut child) {
        Ok(io) => io,
        Err(error) => {
            update_session_status(&mut params.session_state, SessionStatus::Failed);
            send_failure_with_details(
                sender,
                error.to_string(),
                None,
                Some(params.transcript_path.clone()),
            );
            return;
        }
    };

    let mut run_ctx = RunContext {
        session_state: &mut params.session_state,
        sender,
        stderr_capture,
    };

    let thread_id = params.thread_id.as_str();
    let completion = run_stream_with_context(
        StreamRunInput {
            stdout,
            stdin,
            prompt: params.prompt.as_str(),
        },
        &mut transcript,
        &mut run_ctx,
        |stream_stdout, stream_stdin, context| {
            stream::stream_resume_progress(stream_stdout, stream_stdin, context, thread_id)
        },
    );

    if let Some(outcome) = completion {
        finalize(&mut child, outcome, params.transcript_path, &mut run_ctx);
    }
}
