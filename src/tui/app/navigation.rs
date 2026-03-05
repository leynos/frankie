//! Navigation handlers and cursor management.
//!
//! Each navigation method updates the cursor position via the centralised
//! `set_cursor` helper, which adjusts the scroll offset and updates
//! `selected_comment_id` to keep selection tracking synchronised.

use bubbletea_rs::Cmd;

use super::ReviewApp;
use crate::tui::messages::AppMsg;

impl ReviewApp {
    /// Returns the ID of the currently selected comment, if any.
    #[must_use]
    pub fn current_selected_id(&self) -> Option<u64> {
        self.selected_comment().map(|review| review.id)
    }

    /// Returns a reference to the currently selected comment, if any.
    #[must_use]
    pub fn selected_comment(&self) -> Option<&crate::github::models::ReviewComment> {
        self.filtered_indices
            .get(self.filter_state.cursor_position)
            .and_then(|&idx| self.reviews.get(idx))
    }

    /// Selects the comment with the given ID by moving the cursor to it.
    ///
    /// Returns `true` if the comment was found and selected, or `false`
    /// if no comment with the given ID exists in the current filtered view.
    pub fn select_by_id(&mut self, id: u64) -> bool {
        self.find_filtered_index_by_id(id)
            .map(|index| self.set_cursor(index))
            .is_some()
    }

    /// Finds the position within the filtered list for a comment by its ID.
    ///
    /// Returns `Some(index)` if a comment with the given `id` exists in the
    /// current filtered view, or `None` if not found or filtered out.
    /// Used to restore cursor position after sync operations.
    pub(crate) fn find_filtered_index_by_id(&self, id: u64) -> Option<usize> {
        self.filtered_indices
            .iter()
            .position(|&idx| self.reviews.get(idx).is_some_and(|review| review.id == id))
    }

    /// Updates the tracked `selected_comment_id` from the current cursor position.
    ///
    /// Synchronises `selected_comment_id` with whatever comment is currently
    /// under the cursor. Call this after any cursor movement to maintain
    /// selection tracking for sync operations.
    pub(crate) fn update_selected_id(&mut self) {
        self.selected_comment_id = self.current_selected_id();
    }

    /// Clamps the cursor to valid bounds and updates the selected comment ID.
    ///
    /// This helper centralises the common pattern of clamping the cursor after
    /// filter changes and then updating the tracked selection.
    pub(crate) fn clamp_cursor_and_update_selection(&mut self) {
        self.filter_state.clamp_cursor(self.filtered_count());
        self.adjust_scroll_to_cursor();
        self.update_selected_id();
    }

    /// Sets the cursor position and updates the selected comment ID.
    ///
    /// This helper centralises the common pattern of moving the cursor and
    /// updating viewport/selection state in navigation handlers.
    pub(crate) fn set_cursor(&mut self, position: usize) {
        self.filter_state.cursor_position = position;
        self.adjust_scroll_to_cursor();
        self.update_selected_id();
    }

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
