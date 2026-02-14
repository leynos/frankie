//! Navigation handlers and cursor management.
//!
//! Each navigation method updates the cursor position and then calls
//! `update_selected_id()` via the centralised `set_cursor` helper to
//! ensure selection tracking stays synchronised with cursor position.
//! Scrolling is then adjusted so the cursor remains in the visible window.

use bubbletea_rs::Cmd;

use super::ReviewApp;

impl ReviewApp {
    /// Adjusts the scroll offset so the cursor remains within the viewport.
    pub(super) const fn ensure_cursor_visible(&mut self) {
        let cursor_position = self.filter_state.cursor_position;
        let scroll_offset = self.filter_state.scroll_offset;
        let visible_height = self.review_list.visible_height();

        if cursor_position < scroll_offset {
            self.filter_state.scroll_offset = cursor_position;
            return;
        }

        let viewport_end = scroll_offset.saturating_add(visible_height);
        if cursor_position >= viewport_end {
            self.filter_state.scroll_offset =
                cursor_position.saturating_sub(visible_height.saturating_sub(1));
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
        self.adjust_scroll_to_cursor();
        None
    }

    /// Handles cursor down navigation.
    pub(super) fn handle_cursor_down(&mut self) -> Option<Cmd> {
        self.move_cursor_down(1);
        self.adjust_scroll_to_cursor();
        None
    }

    /// Handles page up navigation.
    pub(super) fn handle_page_up(&mut self) -> Option<Cmd> {
        let page_size = self.review_list.visible_height();
        self.move_cursor_up(page_size);
        self.adjust_scroll_to_cursor();
        None
    }

    /// Handles page down navigation.
    pub(super) fn handle_page_down(&mut self) -> Option<Cmd> {
        let page_size = self.review_list.visible_height();
        self.move_cursor_down(page_size);
        self.adjust_scroll_to_cursor();
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
        self.set_cursor(max_index);
        None
    }
}
