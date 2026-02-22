//! Layout helpers for the review TUI model.
//!
//! This module encapsulates list/detail height calculations and scroll
//! adjustments based on terminal size and cursor movement.

use super::{CHROME_HEIGHT, MIN_DETAIL_HEIGHT, MIN_LIST_HEIGHT, ReviewApp};

impl ReviewApp {
    /// Returns the number of rows available for the UI chrome and comments.
    ///
    /// Body rows available to the list and detail sections.
    const fn visible_body_height(&self) -> usize {
        (self.height as usize).saturating_sub(CHROME_HEIGHT)
    }

    /// Updates the visible row count for the review list and stores it in the
    /// component.
    pub(super) fn set_visible_list_height(&mut self) {
        let list_height = self.calculate_list_height();
        self.review_list.set_visible_height(list_height);
    }

    /// Calculates the number of rows available for the review list.
    ///
    /// The detail pane uses the remaining body rows once the list is bounded.
    /// This ensures both list and detail grow with the terminal and avoids a
    /// fixed list/detail ratio.
    pub(super) fn calculate_list_height(&self) -> usize {
        let body_height = self.visible_body_height();

        let list_max = if body_height > MIN_DETAIL_HEIGHT {
            body_height.saturating_sub(MIN_DETAIL_HEIGHT)
        } else {
            0
        };

        let natural_list_height = self.filtered_count().max(MIN_LIST_HEIGHT);
        natural_list_height.min(list_max).max(MIN_LIST_HEIGHT)
    }

    /// Calculates the number of rows available for the detail pane.
    pub(super) const fn calculate_detail_height(&self) -> usize {
        let body_height = self.visible_body_height();
        body_height.saturating_sub(self.review_list.visible_height())
    }

    /// Adjusts scroll offset so the selected cursor remains visible.
    pub(super) const fn adjust_scroll_to_cursor(&mut self) {
        let cursor = self.filter_state.cursor_position;
        let visible_height = self.review_list.visible_height();

        // If nothing is visible, keep the scroll offset unchanged.
        if visible_height == 0 {
            return;
        }

        if cursor < self.filter_state.scroll_offset {
            self.filter_state.scroll_offset = cursor;
            return;
        }

        let viewport_end = self
            .filter_state
            .scroll_offset
            .saturating_add(visible_height);
        if cursor >= viewport_end {
            self.filter_state.scroll_offset =
                cursor.saturating_sub(visible_height.saturating_sub(1));
        }
    }
}
