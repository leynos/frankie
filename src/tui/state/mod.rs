//! State management for the review listing TUI.
//!
//! This module provides the core state types for managing filter criteria
//! and cursor position in the review list.

mod diff_context;
mod filter_state;

pub(crate) use diff_context::{
    DiffContextState, DiffHunk, RenderedDiffHunk, collect_diff_hunks, find_hunk_index,
};
pub use filter_state::{FilterState, ReviewFilter};
