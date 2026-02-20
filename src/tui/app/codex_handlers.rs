//! Codex execution handlers for the review TUI.
//!
//! This module keeps Codex launch, polling, and result mapping logic separate
//! from the main app update routing.

use std::any::Any;

use bubbletea_rs::Cmd;

use crate::ai::transcript::default_transcript_base_dir;
use crate::ai::{
    CodexExecutionContext, CodexExecutionOutcome, CodexExecutionRequest, CodexExecutionUpdate,
    CodexResumeRequest, SessionState, find_interrupted_session,
};
use crate::export::{ExportedComment, sort_comments, write_jsonl};
use crate::github::IntakeError;
use crate::tui::messages::AppMsg;

use super::ReviewApp;

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
            AppMsg::ResumePromptShown(session) => {
                self.resume_prompt = Some((**session).clone());
                None
            }
            AppMsg::ResumeAccepted => self.handle_resume_accepted(),
            AppMsg::ResumeDeclined => self.handle_resume_declined(),
            _ => None,
        }
    }

    fn handle_start_codex_execution(&mut self) -> Option<Cmd> {
        if self.is_codex_running() {
            self.codex_status = Some("Codex run already in progress".to_owned());
            return Some(self.arm_codex_poll_timer());
        }

        if let Some(session) = Self::check_for_interrupted_session() {
            self.resume_prompt = Some(session);
            return None;
        }

        self.start_fresh_codex_execution()
    }

    /// Checks for an interrupted session matching the current PR context.
    fn check_for_interrupted_session() -> Option<SessionState> {
        let locator = crate::tui::get_refresh_locator()?;
        let base_dir = default_transcript_base_dir()
            .map_err(|error| {
                tracing::debug!("default_transcript_base_dir failed: {:?}", error);
            })
            .ok()?;
        find_interrupted_session(
            base_dir.as_path(),
            locator.owner().as_str(),
            locator.repository().as_str(),
            locator.number().get(),
        )
        .map_err(|error| {
            tracing::debug!("find_interrupted_session failed: {:?}", error);
        })
        .ok()
        .flatten()
    }

    /// Starts a fresh Codex execution (no resume prompt).
    fn start_fresh_codex_execution(&mut self) -> Option<Cmd> {
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
                Some(self.arm_codex_poll_timer())
            }
            Err(error) => {
                self.error = Some(format!("Codex execution failed to start: {error}"));
                None
            }
        }
    }

    /// Handles the user accepting the resume prompt.
    fn handle_resume_accepted(&mut self) -> Option<Cmd> {
        let session = self.resume_prompt.clone()?;

        let comments_jsonl = match self.build_filtered_comments_jsonl() {
            Ok(jsonl) => jsonl,
            Err(error) => {
                self.error = Some(format!("Resume failed: {error}"));
                return None;
            }
        };

        let pr_url = Self::build_pr_url();
        let request = CodexResumeRequest::new(session, comments_jsonl, pr_url);

        match self.codex_service.resume(request) {
            Ok(handle) => {
                self.resume_prompt = None;
                self.codex_handle = Some(handle);
                self.codex_status = Some("resuming interrupted Codex session".to_owned());
                self.error = None;
                Some(self.arm_codex_poll_timer())
            }
            Err(error) => {
                self.error = Some(format!("Codex resume failed: {error}"));
                None
            }
        }
    }

    /// Handles the user declining the resume prompt; starts a fresh run.
    fn handle_resume_declined(&mut self) -> Option<Cmd> {
        self.resume_prompt = None;
        self.start_fresh_codex_execution()
    }

    /// Builds the PR URL from the current refresh context.
    fn build_pr_url() -> Option<String> {
        // This assumes `get_refresh_locator()` points to a GitHub-hosted
        // repository; update this owner/repository/number URL builder if
        // provider-agnostic links (e.g. GitLab or Bitbucket) are needed.
        let locator = crate::tui::get_refresh_locator()?;
        Some(format!(
            "https://github.com/{}/{}/pull/{}",
            locator.owner().as_str(),
            locator.repository().as_str(),
            locator.number().get()
        ))
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

        Some(self.arm_codex_poll_timer())
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

        Ok(CodexExecutionRequest::new(
            context,
            comments_jsonl,
            Self::build_pr_url(),
        ))
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

    pub(super) fn arm_codex_poll_timer(&self) -> Cmd {
        let interval = self.codex_poll_interval;
        Box::pin(async move {
            tokio::time::sleep(interval).await;
            Some(Box::new(AppMsg::CodexPollTick) as Box<dyn Any + Send>)
        })
    }
}

#[cfg(test)]
#[path = "codex_handlers_tests.rs"]
mod tests;
