//! Messaging helpers for Codex execution updates.
//!
//! Centralizes success and failure event construction for the Codex runner.

use std::sync::mpsc::Sender;

use camino::Utf8PathBuf;

use crate::ai::codex_exec::{CodexExecutionOutcome, CodexExecutionUpdate};

pub(super) fn send_success(sender: &Sender<CodexExecutionUpdate>, transcript_path: Utf8PathBuf) {
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
