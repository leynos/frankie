//! Navigation handlers and cursor management.
//!
//! Each navigation method updates the cursor position and then calls
//! `update_selected_id()` via the centralised `set_cursor` helper to
//! ensure selection tracking stays synchronised with cursor position.

use bubbletea_rs::Cmd;

use super::ReviewApp;

impl ReviewApp {
    /// Handles cursor up navigation.
    pub(super) fn handle_cursor_up(&mut self) -> Option<Cmd> {
        let new_pos = self.filter_state.cursor_position.saturating_sub(1);
        self.set_cursor(new_pos);
        None
    }

    /// Handles cursor down navigation.
    pub(super) fn handle_cursor_down(&mut self) -> Option<Cmd> {
        let max_index = self.filtered_count().saturating_sub(1);
        let new_pos = self
            .filter_state
            .cursor_position
            .saturating_add(1)
            .min(max_index);
        self.set_cursor(new_pos);
        None
    }

    /// Handles page up navigation.
    pub(super) fn handle_page_up(&mut self) -> Option<Cmd> {
        let page_size = self.review_list.visible_height();
        let new_pos = self.filter_state.cursor_position.saturating_sub(page_size);
        self.set_cursor(new_pos);
        None
    }

    /// Handles page down navigation.
    pub(super) fn handle_page_down(&mut self) -> Option<Cmd> {
        let page_size = self.review_list.visible_height();
        let max_index = self.filtered_count().saturating_sub(1);
        let new_pos = self
            .filter_state
            .cursor_position
            .saturating_add(page_size)
            .min(max_index);
        self.set_cursor(new_pos);
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
        // Adjust scroll offset so the last item is visible at the bottom.
        // This mirrors handle_home which resets scroll_offset to 0.
        let visible_height = self.review_list.visible_height();
        self.filter_state.scroll_offset =
            max_index.saturating_sub(visible_height.saturating_sub(1));
        self.set_cursor(max_index);
        None
    }
}
