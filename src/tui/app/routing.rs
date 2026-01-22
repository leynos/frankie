//! Message routing and dispatch logic.
//!
//! This module handles routing messages based on the current view mode and
//! dispatching them to appropriate category handlers. It implements the
//! mode-based routing pattern where different view modes may handle or block
//! messages differently.

use bubbletea_rs::Cmd;

use super::ReviewApp;
use crate::tui::app::ViewMode;
use crate::tui::messages::AppMsg;
use crate::tui::state::{ReviewFilter, collect_diff_hunks, find_hunk_index};

/// Result of routing in `DiffContext` mode.
enum DiffContextRouting {
    Handled(Option<Cmd>),
    Fallthrough,
}

/// Result of routing in `TimeTravel` mode.
enum TimeTravelRouting {
    Handled(Option<Cmd>),
    Fallthrough,
}

/// Result of view mode-based routing.
enum ViewModeRouting {
    Handled(Option<Cmd>),
    Fallthrough,
}

impl ReviewApp {
    /// Enters the full-screen diff context view.
    pub(super) fn enter_diff_context(&mut self) {
        self.rebuild_diff_context_state();
        self.view_mode = ViewMode::DiffContext;
    }

    /// Exits the full-screen diff context view.
    const fn exit_diff_context(&mut self) {
        self.view_mode = ViewMode::ReviewList;
    }

    /// Rebuilds the diff context state from the current filtered reviews.
    pub(super) fn rebuild_diff_context_state(&mut self) {
        let max_width = self.width as usize;
        let hunks = collect_diff_hunks(&self.reviews, &self.filtered_indices);
        let preferred_index = find_hunk_index(&hunks, self.selected_comment());
        let rendered = self.diff_context_component.render_hunks(&hunks, max_width);

        self.diff_context_state
            .rebuild(rendered, max_width, preferred_index);
    }

    /// Handles diff context navigation messages.
    fn handle_diff_context_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::ShowDiffContext => {
                self.enter_diff_context();
                None
            }
            AppMsg::HideDiffContext | AppMsg::EscapePressed => {
                self.exit_diff_context();
                None
            }
            AppMsg::NextHunk => {
                self.diff_context_state.move_next();
                None
            }
            AppMsg::PreviousHunk => {
                self.diff_context_state.move_previous();
                None
            }
            _ => {
                // Unreachable: caller filters to diff-context messages.
                None
            }
        }
    }

    /// Routes messages when in `DiffContext` mode.
    ///
    /// Returns `DiffContextRouting::Handled` if the message was handled in
    /// `DiffContext` mode, or `DiffContextRouting::Fallthrough` if the message
    /// should fall through to regular routing.
    fn try_handle_in_diff_context_mode(&mut self, msg: &AppMsg) -> DiffContextRouting {
        if self.view_mode != ViewMode::DiffContext {
            return DiffContextRouting::Fallthrough;
        }

        // EscapePressed in DiffContext mode
        if matches!(msg, AppMsg::EscapePressed) {
            return DiffContextRouting::Handled(self.handle_diff_context_msg(msg));
        }

        // DiffContext-specific messages
        if msg.is_diff_context() {
            return DiffContextRouting::Handled(self.handle_diff_context_msg(msg));
        }

        // Block navigation and filter messages in DiffContext mode
        if msg.is_navigation() || msg.is_filter() {
            return DiffContextRouting::Handled(None);
        }

        // Allow other messages to fall through
        DiffContextRouting::Fallthrough
    }

    /// Checks if a message should be blocked when in time-travel mode.
    ///
    /// Time-travel mode blocks navigation, filter, and diff context messages
    /// to prevent interference with the time-travel view state.
    const fn is_blocked_in_time_travel(msg: &AppMsg) -> bool {
        msg.is_navigation() || msg.is_filter() || msg.is_diff_context()
    }

    /// Routes messages when in `TimeTravel` mode.
    ///
    /// Returns `TimeTravelRouting::Handled` if the message was handled in
    /// `TimeTravel` mode, or `TimeTravelRouting::Fallthrough` if the message
    /// should fall through to regular routing.
    fn try_handle_in_time_travel_mode(&mut self, msg: &AppMsg) -> TimeTravelRouting {
        if self.view_mode != ViewMode::TimeTravel {
            return TimeTravelRouting::Fallthrough;
        }

        // Time-travel-specific messages
        if msg.is_time_travel() {
            return TimeTravelRouting::Handled(self.handle_time_travel_msg(msg));
        }

        // Block navigation, filter, and diff context messages in TimeTravel mode
        if Self::is_blocked_in_time_travel(msg) {
            return TimeTravelRouting::Handled(None);
        }

        // Allow other messages to fall through (e.g., Quit, WindowResized)
        TimeTravelRouting::Fallthrough
    }

    /// Dispatches time-travel messages to their handlers.
    fn handle_time_travel_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::EnterTimeTravel => self.handle_enter_time_travel(),
            AppMsg::ExitTimeTravel => self.handle_exit_time_travel(),
            AppMsg::TimeTravelLoaded(state) => self.handle_time_travel_loaded(state.clone()),
            AppMsg::TimeTravelFailed(error) => self.handle_time_travel_failed(error),
            AppMsg::NextCommit => self.handle_next_commit(),
            AppMsg::PreviousCommit => self.handle_previous_commit(),
            AppMsg::CommitNavigated(state) => self.handle_commit_navigated(state.clone()),
            _ => None,
        }
    }

    /// Routes messages based on the current view mode.
    ///
    /// Returns `ViewModeRouting::Handled(cmd)` if the message was handled by
    /// mode-specific routing, or `ViewModeRouting::Fallthrough` if the message
    /// should proceed to category-based dispatch.
    fn route_by_view_mode(&mut self, msg: &AppMsg) -> ViewModeRouting {
        // Route TimeTravel mode messages first (takes priority)
        if let TimeTravelRouting::Handled(result) = self.try_handle_in_time_travel_mode(msg) {
            return ViewModeRouting::Handled(result);
        }

        // Route DiffContext mode messages
        if let DiffContextRouting::Handled(result) = self.try_handle_in_diff_context_mode(msg) {
            return ViewModeRouting::Handled(result);
        }

        // EscapePressed in ReviewList mode
        if matches!(msg, AppMsg::EscapePressed) {
            return ViewModeRouting::Handled(self.handle_clear_filter());
        }

        ViewModeRouting::Fallthrough
    }

    /// Dispatches messages based on their category.
    ///
    /// This method handles messages that were not intercepted by mode-specific
    /// routing, dispatching them to the appropriate category handler.
    fn dispatch_by_message_category(&mut self, msg: &AppMsg) -> Option<Cmd> {
        if msg.is_navigation() {
            return self.handle_navigation_msg(msg);
        }
        if msg.is_filter() {
            return self.handle_filter_msg(msg);
        }
        if msg.is_diff_context() {
            return self.handle_diff_context_msg(msg);
        }
        if msg.is_time_travel() {
            return self.handle_time_travel_msg(msg);
        }
        if msg.is_data() {
            return self.handle_data_msg(msg);
        }
        self.handle_lifecycle_msg(msg)
    }

    /// Handles a message and updates state accordingly.
    ///
    /// This method is the core update function that processes all application
    /// messages and returns any resulting commands. It first attempts mode-based
    /// routing, then falls back to category-based dispatch.
    #[doc(hidden)]
    pub fn handle_message(&mut self, msg: &AppMsg) -> Option<Cmd> {
        if let ViewModeRouting::Handled(result) = self.route_by_view_mode(msg) {
            return result;
        }
        self.dispatch_by_message_category(msg)
    }

    /// Dispatches navigation messages to their handlers.
    fn handle_navigation_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::CursorUp => self.handle_cursor_up(),
            AppMsg::CursorDown => self.handle_cursor_down(),
            AppMsg::PageUp => self.handle_page_up(),
            AppMsg::PageDown => self.handle_page_down(),
            AppMsg::Home => self.handle_home(),
            AppMsg::End => self.handle_end(),
            _ => {
                // Unreachable: caller filters to navigation messages.
                None
            }
        }
    }

    /// Dispatches filter messages to their handlers.
    fn handle_filter_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::SetFilter(filter) => self.handle_set_filter(filter),
            AppMsg::ClearFilter => self.handle_clear_filter(),
            AppMsg::CycleFilter => self.handle_cycle_filter(),
            _ => {
                // Unreachable: caller filters to filter messages.
                None
            }
        }
    }

    /// Dispatches data loading and sync messages to their handlers.
    fn handle_data_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::RefreshRequested => self.handle_refresh_requested(),
            AppMsg::RefreshComplete(new_reviews) => self.handle_refresh_complete(new_reviews),
            AppMsg::RefreshFailed(error_msg) => self.handle_refresh_failed(error_msg),
            AppMsg::SyncTick => self.handle_sync_tick(),
            AppMsg::SyncComplete {
                reviews,
                latency_ms,
            } => self.handle_sync_complete(reviews, *latency_ms),
            _ => {
                // Unreachable: caller filters to data messages.
                None
            }
        }
    }

    /// Dispatches lifecycle and window messages to their handlers.
    fn handle_lifecycle_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::Quit => Some(bubbletea_rs::quit()),
            AppMsg::ToggleHelp => {
                self.show_help = !self.show_help;
                None
            }
            AppMsg::WindowResized { width, height } => self.handle_resize(*width, *height),
            _ => {
                // Unreachable: caller filters to lifecycle messages.
                None
            }
        }
    }

    fn handle_set_filter(&mut self, filter: &ReviewFilter) -> Option<Cmd> {
        self.filter_state.active_filter = filter.clone();
        self.rebuild_filter_cache();
        self.clamp_cursor_and_update_selection();
        None
    }

    fn handle_clear_filter(&mut self) -> Option<Cmd> {
        self.filter_state.active_filter = ReviewFilter::All;
        self.rebuild_filter_cache();
        self.clamp_cursor_and_update_selection();
        None
    }

    /// Cycles the active filter between `All` and `Unresolved`.
    ///
    /// This method only toggles between the two primary filter modes:
    /// - From `All` -> switches to `Unresolved`
    /// - From any other filter (including `ByFile`, `ByReviewer`, `ByCommitRange`)
    ///   -> resets to `All`
    ///
    /// This simplified cycling is intentional: other filter variants require
    /// parameters (file path, reviewer name, commit range) that cannot be
    /// cycled through without additional user input.
    fn handle_cycle_filter(&mut self) -> Option<Cmd> {
        let next_filter = match &self.filter_state.active_filter {
            ReviewFilter::All => ReviewFilter::Unresolved,
            _ => ReviewFilter::All,
        };
        self.filter_state.active_filter = next_filter;
        self.rebuild_filter_cache();
        self.clamp_cursor_and_update_selection();
        None
    }

    fn handle_resize(&mut self, width: u16, height: u16) -> Option<Cmd> {
        self.width = width;
        self.height = height;
        let list_height = height.saturating_sub(4) as usize;
        self.review_list.set_visible_height(list_height);
        if self.view_mode == ViewMode::DiffContext
            && self.diff_context_state.cached_width() != width as usize
        {
            self.rebuild_diff_context_state();
        }
        None
    }
}
