//! State management for the review listing TUI.
//!
//! This module provides the core state types for managing filter criteria,
//! cursor position in the review list, and time-travel navigation state.

mod diff_context;
mod filter_state;
mod reply_draft;

pub use crate::time_travel::{TimeTravelInitParams, TimeTravelState};
pub(crate) use diff_context::{
    DiffContextState, DiffHunk, RenderedDiffHunk, clamp_hunk_index, collect_diff_hunks,
    find_hunk_index,
};
pub use filter_state::{FilterState, ReviewFilter};
pub use reply_draft::{ReplyDraftError, ReplyDraftState};
