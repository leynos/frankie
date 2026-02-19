//! Child-process termination helpers for Codex runs.
//!
//! Encapsulates best-effort kill/wait logic and trace-level diagnostics.

use std::process::{Child, ExitStatus};

pub(super) fn terminate_child(child: &mut Child) {
    if !child_is_running(child) {
        return;
    }

    log_if_kill_failed(child.kill());
    log_if_wait_failed(child.wait());
}

fn child_is_running(child: &mut Child) -> bool {
    child.try_wait().ok().is_some_and(|status| status.is_none())
}

fn log_if_kill_failed(result: std::io::Result<()>) {
    if let Err(error) = result {
        tracing::trace!("failed to kill Codex child process: {error}");
    }
}

fn log_if_wait_failed(result: std::io::Result<ExitStatus>) {
    if let Err(error) = result {
        tracing::trace!("failed to wait for Codex child process: {error}");
    }
}
