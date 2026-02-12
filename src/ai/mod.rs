//! AI integration services for Codex-assisted workflows.
//!
//! This module contains process execution and transcript persistence utilities
//! used by the review TUI when launching `codex exec`.

pub mod codex_exec;
mod codex_process;
pub mod transcript;

pub use codex_exec::{
    CodexExecutionContext, CodexExecutionHandle, CodexExecutionOutcome, CodexExecutionRequest,
    CodexExecutionService, CodexExecutionUpdate, CodexProgressEvent, SystemCodexExecutionService,
};
