//! Filter message handlers for the review TUI.
//!
//! This module contains the handlers for filter-related messages, managing
//! the active review filter and keeping the cursor position synchronised.

use bubbletea_rs::Cmd;

use super::ReviewApp;
use crate::tui::state::ReviewFilter;

impl ReviewApp {
    /// Handles a `SetFilter` message by applying the given filter.
    pub(super) fn handle_set_filter(&mut self, filter: &ReviewFilter) -> Option<Cmd> {
        self.filter_state.active_filter = filter.clone();
        self.rebuild_filter_cache();
        self.clamp_cursor_and_update_selection();
        None
    }

    /// Handles a `ClearFilter` message by resetting the filter to `All`.
    pub(super) fn handle_clear_filter(&mut self) -> Option<Cmd> {
        self.filter_state.active_filter = ReviewFilter::All;
        self.rebuild_filter_cache();
        self.clamp_cursor_and_update_selection();
        None
    }

    /// Cycles the active filter between `All` and `Unresolved`.
    ///
    /// This method only toggles between the two primary filter modes:
    /// - From `All` -> switches to `Unresolved`
    /// - From any other filter (including `ByFile`, `ByReviewer`,
    ///   `ByCommitRange`) -> resets to `All`
    ///
    /// This simplified cycling is intentional: other filter variants require
    /// parameters (file path, reviewer name, commit range) that cannot be
    /// cycled through without additional user input.
    pub(super) fn handle_cycle_filter(&mut self) -> Option<Cmd> {
        let next_filter = match &self.filter_state.active_filter {
            ReviewFilter::All => ReviewFilter::Unresolved,
            _ => ReviewFilter::All,
        };
        self.filter_state.active_filter = next_filter;
        self.rebuild_filter_cache();
        self.clamp_cursor_and_update_selection();
        None
    }
}
