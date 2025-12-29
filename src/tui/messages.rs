//! Message types for the TUI update loop.
//!
//! This module defines all message types that can be sent to the application's
//! update function. Messages represent user actions, async command results,
//! and system events.

use crate::github::error::IntakeError;
use crate::github::models::ReviewComment;

use super::state::ReviewFilter;

/// Messages for the review listing TUI application.
#[derive(Debug, Clone)]
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

    // Data loading
    /// Request a refresh of review data from the API.
    RefreshRequested,
    /// Refresh completed successfully with new data.
    RefreshComplete(Vec<ReviewComment>),
    /// Refresh failed with an error.
    RefreshFailed(String),

    // Application lifecycle
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

impl AppMsg {
    /// Creates an error message from an `IntakeError`.
    #[must_use]
    pub fn from_error(error: &IntakeError) -> Self {
        Self::RefreshFailed(error.to_string())
    }
}
