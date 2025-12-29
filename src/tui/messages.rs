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
}
