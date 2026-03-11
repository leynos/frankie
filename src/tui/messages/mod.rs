//! Message types for the TUI update loop.
//!
//! This module defines all message types that can be sent to the application's
//! update function. Messages represent user actions, async command results,
//! and system events.

mod classification;

use crate::ai::{
    CodexExecutionOutcome, CodexProgressEvent, CommentRewriteMode, CommentRewriteOutcome,
    PrDiscussionSummary, SessionState,
};
use crate::github::models::ReviewComment;
use crate::verification::CommentVerificationResult;

use super::state::{ReviewFilter, TimeTravelState};

pub use self::classification::MessageCategory;

/// Messages for the review listing TUI application.
#[derive(Debug, Clone)]
#[doc(hidden)]
pub enum AppMsg {
    // Navigation
    /// Move cursor up one item.
    CursorUp,
    /// Move cursor down one item.
    CursorDown,
    /// Move cursor up one page.
    PageUp,
    /// Move cursor down one page.
    PageDown,
    /// Move cursor to first item.
    Home,
    /// Move cursor to last item.
    End,

    // Filter changes
    /// Apply a new filter.
    SetFilter(ReviewFilter),
    /// Clear all filters (show all reviews).
    ClearFilter,
    /// Cycle through available filters.
    CycleFilter,

    // Diff context navigation
    /// Enter the full-screen diff context view.
    ShowDiffContext,
    /// Exit the full-screen diff context view.
    HideDiffContext,
    /// Move to the next diff hunk.
    NextHunk,
    /// Move to the previous diff hunk.
    PreviousHunk,
    /// Escape key pressed (context-aware handling).
    EscapePressed,

    // Time-travel navigation
    /// Enter time-travel mode for the selected comment.
    EnterTimeTravel,
    /// Exit time-travel mode.
    ExitTimeTravel,
    /// Time-travel data loaded successfully.
    TimeTravelLoaded(Box<TimeTravelState>),
    /// Time-travel loading failed.
    TimeTravelFailed(String),
    /// Navigate to the next (more recent) commit in time-travel.
    NextCommit,
    /// Navigate to the previous (older) commit in time-travel.
    PreviousCommit,
    /// Commit navigation completed with updated state.
    CommitNavigated(Box<TimeTravelState>),

    // Codex execution
    /// Start Codex execution using currently filtered comments.
    StartCodexExecution,
    /// Poll for Codex execution updates.
    CodexPollTick,
    /// Codex execution reported a progress event.
    CodexProgress(CodexProgressEvent),
    /// Codex execution finished with an outcome.
    CodexFinished(CodexExecutionOutcome),

    // Session resumption
    /// An interrupted session was detected; prompt the user.
    ResumePromptShown(Box<SessionState>),
    /// User accepted the resume prompt.
    ResumeAccepted,
    /// User declined the resume prompt.
    ResumeDeclined,

    // Reply drafting
    /// Start a reply draft for the currently selected comment.
    StartReplyDraft,
    /// Insert a configured template into the active reply draft.
    ReplyDraftInsertTemplate {
        /// Zero-based template index.
        template_index: usize,
    },
    /// Insert one typed character into the active reply draft.
    ReplyDraftInsertChar(char),
    /// Remove the final character from the active reply draft.
    ReplyDraftBackspace,
    /// Validate the active reply draft and mark it ready to send.
    ReplyDraftRequestSend,
    /// Cancel and discard the active reply draft.
    ReplyDraftCancel,
    /// Request an AI rewrite for the active reply draft.
    ReplyDraftRequestAiRewrite {
        /// Rewrite mode to execute.
        mode: CommentRewriteMode,
    },
    /// AI rewrite request completed with generated or fallback outcome.
    ReplyDraftAiRewriteReady {
        /// Request identifier used to ignore stale async completions.
        request_id: u64,
        /// Rewrite mode that was requested.
        mode: CommentRewriteMode,
        /// Provider outcome used to update preview or errors.
        outcome: CommentRewriteOutcome,
    },
    /// Apply the currently previewed AI rewrite candidate.
    ReplyDraftAiApply,
    /// Discard the currently previewed AI rewrite candidate.
    ReplyDraftAiDiscard,

    // Resolution verification
    /// Verify the currently selected comment against local git state.
    VerifySelectedComment,
    /// Verify all comments in the current filtered set against local git state.
    VerifyFilteredComments,
    /// Verification completed with results.
    VerificationReady {
        /// Request identifier used to ignore stale async completions.
        request_id: u64,
        /// Verification results (one per verified comment).
        results: Vec<CommentVerificationResult>,
        /// Persistence failure detail, if storing results failed.
        persistence_error: Option<String>,
    },
    /// Verification failed unexpectedly.
    VerificationFailed {
        /// Request identifier used to ignore stale async completions.
        request_id: u64,
        /// User-readable failure message.
        message: String,
    },

    // PR discussion summary
    /// Request generation of a PR-level discussion summary.
    GeneratePrDiscussionSummary,
    /// PR discussion summary generation completed successfully.
    PrDiscussionSummaryReady {
        /// Request identifier used to ignore stale async completions.
        request_id: u64,
        /// Generated structured summary.
        summary: PrDiscussionSummary,
    },
    /// PR discussion summary generation failed.
    PrDiscussionSummaryFailed {
        /// Request identifier used to ignore stale async completions.
        request_id: u64,
        /// User-readable failure message.
        message: String,
    },
    /// Open the currently selected PR discussion summary link.
    OpenSelectedPrDiscussionSummaryLink,
    /// Close the PR discussion summary view.
    HidePrDiscussionSummary,

    // Data loading
    /// Request a refresh of review data from the API.
    RefreshRequested,
    /// Refresh completed successfully with new data.
    RefreshComplete(Vec<ReviewComment>),
    /// Refresh failed with an error.
    RefreshFailed(String),

    // Background sync
    /// Timer tick for background sync.
    SyncTick,
    /// Incremental sync completed successfully with new data and timing.
    SyncComplete {
        /// Fresh reviews from the API.
        reviews: Vec<ReviewComment>,
        /// Duration of the sync operation in milliseconds.
        latency_ms: u64,
    },

    // Application lifecycle
    /// Synthetic startup event emitted immediately after launch.
    Initialized,
    /// Quit the application.
    Quit,
    /// Toggle help overlay.
    ToggleHelp,

    // Window events
    /// Terminal window was resized.
    WindowResized {
        /// New width in columns.
        width: u16,
        /// New height in rows.
        height: u16,
    },
}
