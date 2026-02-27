//! AI integration services for Codex-assisted workflows.
//!
//! This module contains process execution and transcript persistence utilities
//! used by the review TUI when launching `codex app-server`.

pub mod codex_exec;
mod codex_process;
pub mod comment_rewrite;
pub mod session;
pub mod transcript;

pub use codex_exec::{
    CodexExecutionContext, CodexExecutionHandle, CodexExecutionOutcome, CodexExecutionRequest,
    CodexExecutionService, CodexExecutionUpdate, CodexProgressEvent, CodexResumeRequest,
    SystemCodexExecutionService,
};
pub use comment_rewrite::{
    CommentRewriteContext, CommentRewriteFallback, CommentRewriteGenerated, CommentRewriteMode,
    CommentRewriteModeParseError, CommentRewriteOutcome, CommentRewriteRequest,
    CommentRewriteService, OpenAiCommentRewriteConfig, OpenAiCommentRewriteService,
    SideBySideDiffPreview, SideBySideLine, build_side_by_side_diff_preview, rewrite_with_fallback,
};
pub use session::{SessionState, SessionStatus, find_interrupted_session};
