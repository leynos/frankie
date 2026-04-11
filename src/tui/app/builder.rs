//! Application builder for constructing `ReviewApp` with configuration.
//!
//! This module provides builder methods for creating `ReviewApp` instances
//! with custom services and configuration options.

use std::sync::Arc;

use crate::ai::{CodexExecutionService, CommentRewriteService, PrDiscussionSummaryService};
use crate::config::DEFAULT_COMMIT_HISTORY_LIMIT;
use crate::local::GitOperations;
use crate::persistence::ReviewCommentVerificationCache;
use crate::tui::ReplyDraftConfig;
use crate::verification::ResolutionVerificationService;

use super::ReviewApp;

impl ReviewApp {
    /// Sets the git operations for time-travel navigation.
    ///
    /// Call this method after creating the app if a local Git repository is
    /// available to enable time-travel functionality.
    #[must_use]
    pub fn with_git_ops(mut self, git_ops: Arc<dyn GitOperations>, head_sha: String) -> Self {
        self.git_ops = Some(git_ops);
        self.head_sha = Some(head_sha);
        self
    }

    /// Sets the maximum number of commits to load in time-travel history.
    #[must_use]
    pub const fn with_commit_history_limit(mut self, limit: usize) -> Self {
        self.commit_history_limit = limit;
        self
    }

    /// Sets the Codex execution service used by this app instance.
    #[must_use]
    pub fn with_codex_service(mut self, codex_service: Arc<dyn CodexExecutionService>) -> Self {
        self.codex_service = codex_service;
        self
    }

    /// Sets the poll interval used when draining Codex progress events.
    #[must_use]
    pub const fn with_codex_poll_interval(mut self, interval: std::time::Duration) -> Self {
        self.codex_poll_interval = interval;
        self
    }

    /// Sets reply-drafting configuration for this app instance.
    #[must_use]
    pub fn with_reply_draft_config(mut self, reply_draft_config: ReplyDraftConfig) -> Self {
        self.reply_draft_config = reply_draft_config;
        self
    }

    /// Sets the rewrite service used by AI draft helpers.
    #[must_use]
    pub fn with_comment_rewrite_service(
        mut self,
        comment_rewrite_service: Arc<dyn CommentRewriteService>,
    ) -> Self {
        self.comment_rewrite_service = comment_rewrite_service;
        self
    }

    /// Sets the resolution verification service for this app instance.
    #[must_use]
    pub fn with_resolution_verification_service(
        mut self,
        service: Arc<dyn ResolutionVerificationService>,
    ) -> Self {
        self.verification.service = Some(service);
        self
    }

    /// Sets the verification cache used to load and persist verification results.
    #[must_use]
    pub fn with_review_comment_verification_cache(
        mut self,
        cache: Arc<ReviewCommentVerificationCache>,
    ) -> Self {
        self.verification.cache = Some(cache);
        self
    }

    /// Sets the PR-discussion summary service for this app instance.
    #[must_use]
    pub fn with_pr_discussion_summary_service(
        mut self,
        pr_discussion_summary_service: Arc<dyn PrDiscussionSummaryService>,
    ) -> Self {
        self.pr_discussion_summary_service = pr_discussion_summary_service;
        self
    }
}

/// Returns the default commit history limit used when constructing `ReviewApp`.
#[must_use]
pub(super) const fn default_commit_history_limit() -> usize {
    DEFAULT_COMMIT_HISTORY_LIMIT
}
