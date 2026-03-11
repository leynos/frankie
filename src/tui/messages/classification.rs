//! Message classification helpers for TUI dispatch.

use crate::github::error::IntakeError;

use super::AppMsg;

/// Logical categories used for message dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageCategory {
    /// Cursor and paging navigation actions.
    Navigation,
    /// Filtering and filter-cycling actions.
    Filter,
    /// Full-screen diff context actions.
    DiffContext,
    /// Time-travel loading and navigation actions.
    TimeTravel,
    /// Codex execution and session-resume actions.
    Codex,
    /// Reply-drafting and AI rewrite actions.
    ReplyDraft,
    /// Automated resolution verification actions.
    Verification,
    /// PR discussion summary actions.
    PrDiscussionSummary,
    /// Data refresh and background sync actions.
    Data,
    /// Lifecycle and miscellaneous actions.
    Lifecycle,
}

impl AppMsg {
    /// Creates an error message from an `IntakeError`.
    #[must_use]
    #[doc(hidden)]
    pub fn from_error(error: &IntakeError) -> Self {
        Self::RefreshFailed(error.to_string())
    }

    /// Returns the dispatch category for this message.
    #[must_use]
    pub const fn category(&self) -> MessageCategory {
        match self {
            Self::CursorUp
            | Self::CursorDown
            | Self::PageUp
            | Self::PageDown
            | Self::Home
            | Self::End => MessageCategory::Navigation,
            Self::SetFilter(_) | Self::ClearFilter | Self::CycleFilter => MessageCategory::Filter,
            Self::ShowDiffContext | Self::HideDiffContext | Self::NextHunk | Self::PreviousHunk => {
                MessageCategory::DiffContext
            }
            Self::EnterTimeTravel
            | Self::ExitTimeTravel
            | Self::TimeTravelLoaded(_)
            | Self::TimeTravelFailed(_)
            | Self::NextCommit
            | Self::PreviousCommit
            | Self::CommitNavigated(_) => MessageCategory::TimeTravel,
            Self::StartCodexExecution
            | Self::CodexPollTick
            | Self::CodexProgress(_)
            | Self::CodexFinished(_)
            | Self::ResumePromptShown(_)
            | Self::ResumeAccepted
            | Self::ResumeDeclined => MessageCategory::Codex,
            Self::StartReplyDraft
            | Self::ReplyDraftInsertTemplate { .. }
            | Self::ReplyDraftInsertChar(_)
            | Self::ReplyDraftBackspace
            | Self::ReplyDraftRequestSend
            | Self::ReplyDraftCancel
            | Self::ReplyDraftRequestAiRewrite { .. }
            | Self::ReplyDraftAiRewriteReady { .. }
            | Self::ReplyDraftAiApply
            | Self::ReplyDraftAiDiscard => MessageCategory::ReplyDraft,
            Self::VerifySelectedComment
            | Self::VerifyFilteredComments
            | Self::VerificationReady { .. }
            | Self::VerificationFailed { .. } => MessageCategory::Verification,
            Self::GeneratePrDiscussionSummary
            | Self::PrDiscussionSummaryReady { .. }
            | Self::PrDiscussionSummaryFailed { .. }
            | Self::OpenSelectedPrDiscussionSummaryLink
            | Self::HidePrDiscussionSummary => MessageCategory::PrDiscussionSummary,
            Self::RefreshRequested
            | Self::RefreshComplete(_)
            | Self::RefreshFailed(_)
            | Self::SyncTick
            | Self::SyncComplete { .. } => MessageCategory::Data,
            Self::EscapePressed
            | Self::Initialized
            | Self::Quit
            | Self::ToggleHelp
            | Self::WindowResized { .. } => MessageCategory::Lifecycle,
        }
    }

    /// Returns `true` if this is a navigation message.
    #[must_use]
    pub const fn is_navigation(&self) -> bool {
        matches!(
            self,
            Self::CursorUp
                | Self::CursorDown
                | Self::PageUp
                | Self::PageDown
                | Self::Home
                | Self::End
        )
    }

    /// Returns `true` if this is a filter message.
    #[must_use]
    pub const fn is_filter(&self) -> bool {
        matches!(
            self,
            Self::SetFilter(_) | Self::ClearFilter | Self::CycleFilter
        )
    }

    /// Returns `true` if this is a data loading or sync message.
    #[must_use]
    pub const fn is_data(&self) -> bool {
        matches!(
            self,
            Self::RefreshRequested
                | Self::RefreshComplete(_)
                | Self::RefreshFailed(_)
                | Self::SyncTick
                | Self::SyncComplete { .. }
        )
    }

    /// Returns `true` if this is a resolution verification message.
    #[must_use]
    pub const fn is_verification(&self) -> bool {
        matches!(
            self,
            Self::VerifySelectedComment
                | Self::VerifyFilteredComments
                | Self::VerificationReady { .. }
                | Self::VerificationFailed { .. }
        )
    }

    /// Returns `true` if this is a PR discussion summary message.
    #[must_use]
    pub const fn is_pr_discussion_summary(&self) -> bool {
        matches!(
            self,
            Self::GeneratePrDiscussionSummary
                | Self::PrDiscussionSummaryReady { .. }
                | Self::PrDiscussionSummaryFailed { .. }
                | Self::OpenSelectedPrDiscussionSummaryLink
                | Self::HidePrDiscussionSummary
        )
    }

    /// Returns `true` if this is a Codex execution message.
    #[must_use]
    pub const fn is_codex(&self) -> bool {
        matches!(
            self,
            Self::StartCodexExecution
                | Self::CodexPollTick
                | Self::CodexProgress(_)
                | Self::CodexFinished(_)
                | Self::ResumePromptShown(_)
                | Self::ResumeAccepted
                | Self::ResumeDeclined
        )
    }

    /// Returns `true` if this is a reply-drafting message.
    #[must_use]
    pub const fn is_reply_draft(&self) -> bool {
        matches!(
            self,
            Self::StartReplyDraft
                | Self::ReplyDraftInsertTemplate { .. }
                | Self::ReplyDraftInsertChar(_)
                | Self::ReplyDraftBackspace
                | Self::ReplyDraftRequestSend
                | Self::ReplyDraftCancel
                | Self::ReplyDraftRequestAiRewrite { .. }
                | Self::ReplyDraftAiRewriteReady { .. }
                | Self::ReplyDraftAiApply
                | Self::ReplyDraftAiDiscard
        )
    }

    /// Returns `true` if this is a diff context navigation message.
    #[must_use]
    pub const fn is_diff_context(&self) -> bool {
        matches!(
            self,
            Self::ShowDiffContext | Self::HideDiffContext | Self::NextHunk | Self::PreviousHunk
        )
    }

    /// Returns `true` if this is a time-travel navigation message.
    #[must_use]
    pub const fn is_time_travel(&self) -> bool {
        matches!(
            self,
            Self::EnterTimeTravel
                | Self::ExitTimeTravel
                | Self::TimeTravelLoaded(_)
                | Self::TimeTravelFailed(_)
                | Self::NextCommit
                | Self::PreviousCommit
                | Self::CommitNavigated(_)
        )
    }
}
