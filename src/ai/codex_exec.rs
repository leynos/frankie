//! Codex execution service interfaces.
//!
//! This module defines request/response types and provides the default
//! `codex exec --json` service implementation used by the TUI.

use std::sync::mpsc::{Receiver, TryRecvError};

use camino::{Utf8Path, Utf8PathBuf};
use chrono::Utc;

use crate::github::IntakeError;

use super::codex_process::run_codex;
use super::transcript::{TranscriptMetadata, default_transcript_base_dir, transcript_path};

/// Context used to build Codex execution requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexExecutionContext {
    owner: String,
    repository: String,
    pr_number: u64,
    transcript_dir: Option<Utf8PathBuf>,
}

impl CodexExecutionContext {
    /// Creates a new Codex execution context.
    #[must_use]
    pub fn new(owner: impl Into<String>, repository: impl Into<String>, pr_number: u64) -> Self {
        Self {
            owner: owner.into(),
            repository: repository.into(),
            pr_number,
            transcript_dir: None,
        }
    }

    /// Overrides the transcript directory used for this context.
    #[must_use]
    pub fn with_transcript_dir(mut self, transcript_dir: Utf8PathBuf) -> Self {
        self.transcript_dir = Some(transcript_dir);
        self
    }

    pub(crate) const fn owner(&self) -> &str {
        self.owner.as_str()
    }

    pub(crate) const fn repository(&self) -> &str {
        self.repository.as_str()
    }

    pub(crate) const fn pr_number(&self) -> u64 {
        self.pr_number
    }

    pub(crate) fn transcript_dir(&self) -> Option<&Utf8Path> {
        self.transcript_dir.as_deref()
    }
}

/// Request payload for a Codex run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexExecutionRequest {
    context: CodexExecutionContext,
    pr_url: Option<String>,
    comments_jsonl: String,
}

impl CodexExecutionRequest {
    /// Creates a request using context and rendered comment export.
    #[must_use]
    pub const fn new(
        context: CodexExecutionContext,
        comments_jsonl: String,
        pr_url: Option<String>,
    ) -> Self {
        Self {
            context,
            pr_url,
            comments_jsonl,
        }
    }

    pub(crate) const fn context(&self) -> &CodexExecutionContext {
        &self.context
    }

    pub(crate) fn pr_url(&self) -> Option<&str> {
        self.pr_url.as_deref()
    }

    pub(crate) const fn comments_jsonl(&self) -> &str {
        self.comments_jsonl.as_str()
    }
}

/// Progress events surfaced to the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexProgressEvent {
    /// A parsed status message from a JSONL event.
    Status {
        /// Human-readable status text extracted from the event payload.
        message: String,
    },
    /// A non-JSON line encountered on stdout.
    ParseWarning {
        /// Raw line content that failed JSON parsing.
        raw_line: String,
    },
}

impl CodexProgressEvent {
    /// Formats a user-facing status line.
    #[must_use]
    pub fn status_line(&self) -> String {
        match self {
            Self::Status { message } => format!("progress: {message}"),
            Self::ParseWarning { raw_line } => {
                format!("received non-JSON event: {raw_line}")
            }
        }
    }
}

/// Final outcome for a Codex run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexExecutionOutcome {
    /// Codex exited with success.
    Succeeded {
        /// Transcript saved for this run.
        transcript_path: Utf8PathBuf,
    },
    /// Codex failed to run or exited non-zero.
    Failed {
        /// User-readable failure reason.
        message: String,
        /// Exit code, if available.
        exit_code: Option<i32>,
        /// Transcript path when available.
        transcript_path: Option<Utf8PathBuf>,
    },
}

/// Stream updates produced during execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexExecutionUpdate {
    /// Incremental progress update.
    Progress(CodexProgressEvent),
    /// Terminal outcome update.
    Finished(CodexExecutionOutcome),
}

/// Handle used by the TUI to poll execution updates.
pub struct CodexExecutionHandle {
    receiver: Receiver<CodexExecutionUpdate>,
}

impl std::fmt::Debug for CodexExecutionHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CodexExecutionHandle(..)")
    }
}

impl CodexExecutionHandle {
    /// Creates a handle from a channel receiver.
    #[must_use]
    pub const fn new(receiver: Receiver<CodexExecutionUpdate>) -> Self {
        Self { receiver }
    }

    /// Attempts to receive the next update.
    ///
    /// # Errors
    ///
    /// Returns [`TryRecvError::Empty`] when no update is available yet and
    /// [`TryRecvError::Disconnected`] when the execution stream has closed.
    pub fn try_recv(&self) -> Result<CodexExecutionUpdate, TryRecvError> {
        self.receiver.try_recv()
    }
}

/// Service abstraction for launching Codex runs.
pub trait CodexExecutionService: Send + Sync + std::fmt::Debug {
    /// Starts a Codex run and returns a polling handle.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError`] when the request is invalid or cannot be
    /// prepared for execution.
    fn start(&self, request: CodexExecutionRequest) -> Result<CodexExecutionHandle, IntakeError>;
}

/// Real `codex exec` implementation backed by local process execution.
#[derive(Debug, Clone)]
pub struct SystemCodexExecutionService {
    command_path: String,
}

impl Default for SystemCodexExecutionService {
    fn default() -> Self {
        Self {
            command_path: "codex".to_owned(),
        }
    }
}

impl SystemCodexExecutionService {
    /// Creates a service using the default `codex` executable.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a service using a custom command path.
    #[must_use]
    pub fn with_command_path(command_path: impl Into<String>) -> Self {
        Self {
            command_path: command_path.into(),
        }
    }
}

impl CodexExecutionService for SystemCodexExecutionService {
    fn start(&self, request: CodexExecutionRequest) -> Result<CodexExecutionHandle, IntakeError> {
        if request.comments_jsonl.trim().is_empty() {
            return Err(IntakeError::Configuration {
                message: "cannot run Codex without exported comments".to_owned(),
            });
        }

        let base_dir = match request.context().transcript_dir() {
            Some(path) => path.to_path_buf(),
            None => default_transcript_base_dir()?,
        };

        let metadata = TranscriptMetadata::new(
            request.context().owner(),
            request.context().repository(),
            request.context().pr_number(),
        );
        let transcript_path = transcript_path(base_dir.as_path(), &metadata, Utc::now());

        Ok(run_codex(
            self.command_path.clone(),
            request,
            transcript_path,
        ))
    }
}

#[cfg(test)]
#[path = "codex_exec_tests.rs"]
mod tests;
