//! Finalisation helpers for Codex process execution.
//!
//! Keeps terminal outcome handling separate from orchestration flow in `mod.rs`.

use std::process::Child;

use camino::Utf8PathBuf;

use crate::ai::session::SessionStatus;

use super::RunContext;
use super::app_server::AppServerCompletion;
use super::messaging::{send_failure_with_details, send_success};
use super::session::{session_status_from_completion, update_session_status};
use super::stream::StreamCompletion;
use super::termination::terminate_child;

pub(super) fn finalize(
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
