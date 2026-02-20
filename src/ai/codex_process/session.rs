//! Session-state helpers for Codex process execution.
//!
//! Keeps sidecar lifecycle and status transitions separate from process wiring.

use camino::Utf8PathBuf;
use chrono::Utc;

use crate::ai::codex_exec::CodexExecutionRequest;
use crate::ai::session::{SessionState, SessionStatus};

use super::app_server::AppServerCompletion;

pub(super) fn build_running_session_state(
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

pub(super) const fn session_status_from_completion(
    completion: &AppServerCompletion,
) -> SessionStatus {
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

pub(super) fn log_sidecar_write_error(sidecar_path: &str, error: &str) {
    tracing::warn!("failed to write session sidecar '{sidecar_path}': {error}");
}
