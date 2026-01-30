//! Message routing for the TUI application.
//!
//! This module handles routing messages to the appropriate handlers based on
//! the current view mode. It provides a clean separation between mode-specific
//! message handling and the main application logic.

use bubbletea_rs::Cmd;

use crate::tui::messages::AppMsg;

use super::ReviewApp;
use super::ViewMode;

/// Result of message routing through view mode handlers.
///
/// Used to indicate whether a message was handled by a mode-specific handler
/// or should fall through to the default handling logic.
pub(super) enum MessageRouting {
    Handled(Option<Cmd>),
    Fallthrough,
}

impl ReviewApp {
    /// Routes messages when in `DiffContext` mode.
    ///
    /// Returns `MessageRouting::Handled` if the message was handled in
    /// `DiffContext` mode, or `MessageRouting::Fallthrough` if the message
    /// should fall through to regular routing.
    pub(super) fn try_handle_in_diff_context_mode(&mut self, msg: &AppMsg) -> MessageRouting {
        if self.view_mode != ViewMode::DiffContext {
            return MessageRouting::Fallthrough;
        }

        // EscapePressed in DiffContext mode
        if matches!(msg, AppMsg::EscapePressed) {
            return MessageRouting::Handled(self.handle_diff_context_msg(msg));
        }

        // DiffContext-specific messages
        if msg.is_diff_context() {
            return MessageRouting::Handled(self.handle_diff_context_msg(msg));
        }

        // Block navigation and filter messages in DiffContext mode
        if msg.is_navigation() || msg.is_filter() {
            return MessageRouting::Handled(None);
        }

        // Allow other messages to fall through
        MessageRouting::Fallthrough
    }

    /// Checks if a message should be blocked when in time-travel mode.
    ///
    /// Time-travel mode blocks navigation, filter, and diff context messages
    /// to prevent interference with the time-travel view state.
    pub(super) const fn is_blocked_in_time_travel(msg: &AppMsg) -> bool {
        msg.is_navigation() || msg.is_filter() || msg.is_diff_context()
    }

    /// Routes messages when in `TimeTravel` mode.
    ///
    /// Returns `MessageRouting::Handled` if the message was handled in
    /// `TimeTravel` mode, or `MessageRouting::Fallthrough` if the message
    /// should fall through to regular routing.
    pub(super) fn try_handle_in_time_travel_mode(&mut self, msg: &AppMsg) -> MessageRouting {
        if self.view_mode != ViewMode::TimeTravel {
            return MessageRouting::Fallthrough;
        }

        // Time-travel-specific messages
        if msg.is_time_travel() {
            return MessageRouting::Handled(self.handle_time_travel_msg(msg));
        }

        // Block navigation, filter, and diff context messages in TimeTravel mode
        if Self::is_blocked_in_time_travel(msg) {
            return MessageRouting::Handled(None);
        }

        // Allow other messages to fall through (e.g., Quit, WindowResized)
        MessageRouting::Fallthrough
    }

    /// Routes messages based on the current view mode.
    ///
    /// Returns `MessageRouting::Handled(cmd)` if the message was handled by
    /// mode-specific routing, or `MessageRouting::Fallthrough` if the message
    /// should proceed to category-based dispatch.
    pub(super) fn route_by_view_mode(&mut self, msg: &AppMsg) -> MessageRouting {
        // Route TimeTravel mode messages first (takes priority)
        if let MessageRouting::Handled(result) = self.try_handle_in_time_travel_mode(msg) {
            return MessageRouting::Handled(result);
        }

        // Route DiffContext mode messages
        if let MessageRouting::Handled(result) = self.try_handle_in_diff_context_mode(msg) {
            return MessageRouting::Handled(result);
        }

        // EscapePressed in ReviewList mode only
        if self.view_mode == ViewMode::ReviewList && matches!(msg, AppMsg::EscapePressed) {
            return MessageRouting::Handled(self.handle_clear_filter());
        }

        MessageRouting::Fallthrough
    }

    /// Dispatches messages based on their category.
    ///
    /// This method handles messages that were not intercepted by mode-specific
    /// routing, dispatching them to the appropriate category handler.
    pub(super) fn dispatch_by_message_category(&mut self, msg: &AppMsg) -> Option<Cmd> {
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
}
