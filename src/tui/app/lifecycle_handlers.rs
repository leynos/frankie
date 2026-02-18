//! Lifecycle and window handlers for the review TUI.
//!
//! This module handles startup initialisation, terminal resize events, and
//! high-level lifecycle messages such as quit and help toggling.

use bubbletea_rs::Cmd;

use super::{ReviewApp, ViewMode};
use crate::tui::messages::AppMsg;

impl ReviewApp {
    /// Dispatches lifecycle and window messages to their handlers.
    pub(super) fn handle_lifecycle_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::Initialized => self.handle_initialized(),
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

    /// Handles the synthetic startup message.
    ///
    /// `Initialized` is intended as a one-shot event emitted during startup.
    /// Subsequent `Initialized` messages are ignored to avoid re-arming the
    /// sync timer unintentionally.
    fn handle_initialized(&mut self) -> Option<Cmd> {
        if self.has_initialized {
            return None;
        }

        self.has_initialized = true;
        Some(Self::arm_sync_timer())
    }

    fn handle_resize(&mut self, width: u16, height: u16) -> Option<Cmd> {
        self.width = width;
        self.height = height;
        let list_height = self.calculate_list_height();
        self.review_list.set_visible_height(list_height);
        self.adjust_scroll_to_cursor();
        if self.view_mode == ViewMode::DiffContext
            && self.diff_context_state.cached_width() != width as usize
        {
            self.rebuild_diff_context_state();
        }
        None
    }
}
