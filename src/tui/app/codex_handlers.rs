//! Codex execution handlers for the review TUI.
//!
//! This module keeps Codex launch, polling, and result mapping logic separate
//! from the main app update routing.

use std::any::Any;
use std::time::Duration;

use bubbletea_rs::Cmd;

use crate::ai::{
    CodexExecutionContext, CodexExecutionOutcome, CodexExecutionRequest, CodexExecutionUpdate,
};
use crate::export::{ExportedComment, sort_comments, write_jsonl};
use crate::github::IntakeError;
use crate::tui::messages::AppMsg;

use super::ReviewApp;

/// Poll interval for draining Codex progress events.
const CODEX_POLL_INTERVAL: Duration = Duration::from_millis(150);

impl ReviewApp {
    /// Dispatches Codex lifecycle messages to their handlers.
    pub(super) fn handle_codex_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::StartCodexExecution => self.handle_start_codex_execution(),
            AppMsg::CodexPollTick => self.handle_codex_poll_tick(),
            AppMsg::CodexProgress(event) => {
                self.codex_status = Some(event.status_line());
                None
            }
            AppMsg::CodexFinished(outcome) => {
                self.codex_handle = None;
                self.apply_codex_outcome(outcome.clone());
                None
            }
            _ => None,
        }
    }

    fn handle_start_codex_execution(&mut self) -> Option<Cmd> {
        if self.is_codex_running() {
            self.codex_status = Some("Codex run already in progress".to_owned());
            return Some(Self::arm_codex_poll_timer());
        }

        let request = match self.build_codex_request() {
            Ok(request) => request,
            Err(error) => {
                self.error = Some(format!("Codex execution could not start: {error}"));
                return None;
            }
        };

        match self.codex_service.start(request) {
            Ok(handle) => {
                self.codex_handle = Some(handle);
                self.codex_status = Some("launching Codex execution".to_owned());
                self.error = None;
                Some(Self::arm_codex_poll_timer())
            }
            Err(error) => {
                self.error = Some(format!("Codex execution failed to start: {error}"));
                None
            }
        }
    }

    fn handle_codex_poll_tick(&mut self) -> Option<Cmd> {
        if !self.is_codex_running() {
            return None;
        }

        if let Some(outcome) = self.drain_codex_updates() {
            self.codex_handle = None;
            self.apply_codex_outcome(outcome);
            return None;
        }

        Some(Self::arm_codex_poll_timer())
    }

    fn drain_codex_updates(&mut self) -> Option<CodexExecutionOutcome> {
        let handle = self.codex_handle.as_ref()?;

        loop {
            match handle.try_recv() {
                Ok(CodexExecutionUpdate::Progress(event)) => {
                    self.codex_status = Some(event.status_line());
                }
                Ok(CodexExecutionUpdate::Finished(outcome)) => return Some(outcome),
                Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Some(CodexExecutionOutcome::Failed {
                        message: "Codex progress stream disconnected unexpectedly".to_owned(),
                        exit_code: None,
                        transcript_path: None,
                    });
                }
            }
        }
    }

    fn apply_codex_outcome(&mut self, outcome: CodexExecutionOutcome) {
        match outcome {
            CodexExecutionOutcome::Succeeded { transcript_path } => {
                self.error = None;
                self.codex_status = Some(format!(
                    "Codex execution completed; transcript: {transcript_path}"
                ));
            }
            CodexExecutionOutcome::Failed {
                message,
                exit_code,
                transcript_path,
            } => {
                let path_text = transcript_path
                    .as_ref()
                    .map_or_else(String::new, |path| format!(". Transcript: {path}"));
                let exit_text =
                    exit_code.map_or_else(String::new, |code| format!(" (exit code: {code})"));
                self.codex_status = Some("Codex execution failed".to_owned());
                self.error = Some(format!(
                    "Codex execution failed{exit_text}: {message}{path_text}"
                ));
            }
        }
    }

    fn build_codex_request(&self) -> Result<CodexExecutionRequest, IntakeError> {
        let comments_jsonl = self.build_filtered_comments_jsonl()?;
        let locator = crate::tui::get_refresh_locator().ok_or_else(|| IntakeError::Api {
            message: "Codex execution requires refresh context".to_owned(),
        })?;

        let context = CodexExecutionContext::new(
            locator.owner().as_str(),
            locator.repository().as_str(),
            locator.number().get(),
        );

        let pr_url = Some(format!(
            "https://github.com/{}/{}/pull/{}",
            locator.owner().as_str(),
            locator.repository().as_str(),
            locator.number().get()
        ));

        Ok(CodexExecutionRequest::new(context, comments_jsonl, pr_url))
    }

    fn build_filtered_comments_jsonl(&self) -> Result<String, IntakeError> {
        if self.filtered_indices.is_empty() {
            return Err(IntakeError::Configuration {
                message: "no filtered comments available for Codex execution".to_owned(),
            });
        }

        let mut comments: Vec<ExportedComment> = self
            .filtered_reviews()
            .into_iter()
            .map(ExportedComment::from)
            .collect();
        sort_comments(&mut comments);

        let mut buffer = Vec::new();
        write_jsonl(&mut buffer, &comments)?;

        String::from_utf8(buffer).map_err(|error| IntakeError::Io {
            message: format!("failed to encode comment export as UTF-8: {error}"),
        })
    }

    pub(super) fn arm_codex_poll_timer() -> Cmd {
        Box::pin(async {
            tokio::time::sleep(CODEX_POLL_INTERVAL).await;
            Some(Box::new(AppMsg::CodexPollTick) as Box<dyn Any + Send>)
        })
    }
}

#[cfg(test)]
#[path = "codex_handlers_tests.rs"]
mod tests;
