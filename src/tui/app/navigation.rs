//! Navigation handlers and cursor management.
//!
//! Each navigation method updates the cursor position via the centralised
//! `set_cursor` helper, which adjusts the scroll offset and updates
//! `selected_comment_id` to keep selection tracking synchronised.

use bubbletea_rs::Cmd;

use super::ReviewApp;
use crate::tui::messages::AppMsg;

impl ReviewApp {
    /// Dispatches navigation messages to their handlers.
    pub(super) fn handle_navigation_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
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

    fn move_cursor_up(&mut self, step: usize) {
        let new_pos = self.filter_state.cursor_position.saturating_sub(step);
        self.set_cursor(new_pos);
    }

    fn move_cursor_down(&mut self, step: usize) {
        let max_index = self.filtered_count().saturating_sub(1);
        let new_pos = self
            .filter_state
            .cursor_position
            .saturating_add(step)
            .min(max_index);
        self.set_cursor(new_pos);
    }

    /// Handles cursor up navigation.
    pub(super) fn handle_cursor_up(&mut self) -> Option<Cmd> {
        self.move_cursor_up(1);
        None
    }

    /// Handles cursor down navigation.
    pub(super) fn handle_cursor_down(&mut self) -> Option<Cmd> {
        self.move_cursor_down(1);
        None
    }

    /// Handles page up navigation.
    pub(super) fn handle_page_up(&mut self) -> Option<Cmd> {
        let page_size = self.review_list.visible_height();
        self.move_cursor_up(page_size);
        None
    }

    /// Handles page down navigation.
    pub(super) fn handle_page_down(&mut self) -> Option<Cmd> {
        let page_size = self.review_list.visible_height();
        self.move_cursor_down(page_size);
        None
    }

    /// Handles Home key navigation.
    pub(super) fn handle_home(&mut self) -> Option<Cmd> {
        // Reset scroll offset to ensure the view starts at the top.
        self.filter_state.scroll_offset = 0;
        self.set_cursor(0);
        None
    }

    /// Handles End key navigation.
    pub(super) fn handle_end(&mut self) -> Option<Cmd> {
        let max_index = self.filtered_count().saturating_sub(1);
        // Clamp scroll offset to the last page start so the full final page is
        // visible, even when scroll_offset is stale after a list shrink.
        let visible_height = self.review_list.visible_height();
        self.filter_state.scroll_offset =
            max_index.saturating_sub(visible_height.saturating_sub(1));
        self.set_cursor(max_index);
        None
    }
}
