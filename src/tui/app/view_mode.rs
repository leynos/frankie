//! Shared view-mode and layout constants for the review TUI.

/// Layout rows reserved for header, filter bar, separator newline, and status bar.
pub(crate) const CHROME_HEIGHT: usize = 4;
/// Minimum rows reserved for the comment detail pane to keep detail area visible.
pub(crate) const MIN_DETAIL_HEIGHT: usize = 2;
/// Minimum rows for the review list, ensuring at least one row is visible
/// even when the terminal height is very small.
pub(crate) const MIN_LIST_HEIGHT: usize = 1;

/// Tracks which view is currently active in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    ReviewList,
    DiffContext,
    TimeTravel,
}
