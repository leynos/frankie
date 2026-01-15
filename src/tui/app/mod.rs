//! Main TUI application model implementing the MVU pattern.
//!
//! This module provides the core application state and update logic for the
//! review listing TUI. It coordinates between components, manages filter state,
//! and handles async data loading.
//!
//! # Module Structure
//!
//! - `rendering`: View rendering methods for terminal output
//! - `sync_handlers`: Background sync and refresh handling

use std::any::Any;

use bubbletea_rs::{Cmd, Model};

use crate::github::models::ReviewComment;

use super::components::{
    CommentDetailComponent, CommentDetailViewContext, ReviewListComponent, ReviewListViewContext,
};
use super::input::map_key_to_message;
use super::messages::AppMsg;
use super::state::{FilterState, ReviewFilter};

mod navigation;
mod rendering;
mod sync_handlers;

/// Main application model for the review listing TUI.
#[derive(Debug)]
pub struct ReviewApp {
    /// All review comments (unfiltered).
    pub(crate) reviews: Vec<ReviewComment>,
    /// Cached indices of reviews matching the current filter.
    /// Invalidated when reviews or filter change.
    filtered_indices: Vec<usize>,
    /// Filter and cursor state.
    pub(crate) filter_state: FilterState,
    /// Whether data is currently loading.
    pub(crate) loading: bool,
    /// Current error message, if any.
    pub(crate) error: Option<String>,
    /// Terminal dimensions.
    width: u16,
    height: u16,
    /// Whether help overlay is visible.
    pub(crate) show_help: bool,
    /// Review list component.
    review_list: ReviewListComponent,
    /// Comment detail component.
    comment_detail: CommentDetailComponent,
    /// ID of the currently selected comment, used to restore cursor after sync.
    pub(crate) selected_comment_id: Option<u64>,
}

impl ReviewApp {
    /// Creates a new application with the given review comments.
    #[must_use]
    pub fn new(reviews: Vec<ReviewComment>) -> Self {
        // Build initial cache with all indices (default filter is All)
        let filtered_indices: Vec<_> = (0..reviews.len()).collect();
        // Track ID of first comment for selection preservation
        let selected_comment_id = filtered_indices
            .first()
            .and_then(|&i| reviews.get(i))
            .map(|r| r.id);
        Self {
            reviews,
            filtered_indices,
            filter_state: FilterState::new(),
            loading: false,
            error: None,
            width: 80,
            height: 24,
            show_help: false,
            review_list: ReviewListComponent::new(),
            comment_detail: CommentDetailComponent::new(),
            selected_comment_id,
        }
    }

    /// Creates an empty application (for initial loading state).
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Returns the currently filtered reviews.
    #[must_use]
    pub fn filtered_reviews(&self) -> Vec<&ReviewComment> {
        self.filtered_indices
            .iter()
            .filter_map(|&i| self.reviews.get(i))
            .collect()
    }

    /// Returns the count of filtered reviews.
    #[must_use]
    pub const fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Rebuilds the filtered indices cache based on the current filter.
    ///
    /// This method iterates through all reviews and updates `filtered_indices`
    /// to contain only the indices of reviews matching the active filter.
    /// Call this after modifying `reviews` or changing the active filter.
    pub(crate) fn rebuild_filter_cache(&mut self) {
        self.filtered_indices = self
            .reviews
            .iter()
            .enumerate()
            .filter(|(_, review)| {
                self.filter_state
                    .active_filter
                    .matches(review, &self.reviews)
            })
            .map(|(i, _)| i)
            .collect();
    }

    /// Returns the current cursor position.
    #[must_use]
    pub const fn cursor_position(&self) -> usize {
        self.filter_state.cursor_position
    }

    /// Returns the active filter.
    #[must_use]
    pub const fn active_filter(&self) -> &ReviewFilter {
        &self.filter_state.active_filter
    }

    /// Returns the ID of the currently selected comment, if any.
    #[must_use]
    pub fn current_selected_id(&self) -> Option<u64> {
        self.filtered_indices
            .get(self.filter_state.cursor_position)
            .and_then(|&idx| self.reviews.get(idx))
            .map(|r| r.id)
    }

    /// Returns a reference to the currently selected comment, if any.
    #[must_use]
    pub fn selected_comment(&self) -> Option<&ReviewComment> {
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
            .position(|&idx| self.reviews.get(idx).is_some_and(|r| r.id == id))
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
    fn clamp_cursor_and_update_selection(&mut self) {
        self.filter_state.clamp_cursor(self.filtered_count());
        self.update_selected_id();
    }

    /// Sets the cursor position and updates the selected comment ID.
    ///
    /// This helper centralises the common pattern of moving the cursor and
    /// updating the tracked selection in navigation handlers.
    fn set_cursor(&mut self, position: usize) {
        self.filter_state.cursor_position = position;
        self.update_selected_id();
    }

    /// Handles a message and updates state accordingly.
    ///
    /// This method is the core update function that processes all application
    /// messages and returns any resulting commands. It delegates to specialised
    /// handlers for each message category to keep cyclomatic complexity low.
    pub fn handle_message(&mut self, msg: &AppMsg) -> Option<Cmd> {
        if msg.is_navigation() {
            return self.handle_navigation_msg(msg);
        }
        if msg.is_filter() {
            return self.handle_filter_msg(msg);
        }
        if msg.is_data() {
            return self.handle_data_msg(msg);
        }
        self.handle_lifecycle_msg(msg)
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
                debug_assert!(
                    false,
                    "non-navigation message routed to handle_navigation_msg"
                );
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
                debug_assert!(false, "non-filter message routed to handle_filter_msg");
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
                debug_assert!(false, "non-data message routed to handle_data_msg");
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
                debug_assert!(
                    false,
                    "non-lifecycle message routed to handle_lifecycle_msg"
                );
                None
            }
        }
    }

    // Filter handlers

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

    // Window event handlers

    fn handle_resize(&mut self, width: u16, height: u16) -> Option<Cmd> {
        self.width = width;
        self.height = height;
        let list_height = height.saturating_sub(4) as usize;
        self.review_list.set_visible_height(list_height);
        None
    }
}

impl Model for ReviewApp {
    fn init() -> (Self, Option<Cmd>) {
        // Retrieve initial data from module-level storage
        let reviews = super::get_initial_reviews();
        let model = Self::new(reviews);

        // Start the background sync timer
        let cmd = Self::arm_sync_timer();

        (model, Some(cmd))
    }

    fn update(&mut self, msg: Box<dyn Any + Send>) -> Option<Cmd> {
        // Try to downcast to our message type
        if let Some(app_msg) = msg.downcast_ref::<AppMsg>() {
            return self.handle_message(app_msg);
        }

        // Handle key events from bubbletea-rs
        if let Some(key_msg) = msg.downcast_ref::<bubbletea_rs::event::KeyMsg>() {
            let app_msg = map_key_to_message(key_msg);
            if let Some(mapped) = app_msg {
                return self.handle_message(&mapped);
            }
        }

        // Handle window size messages
        if let Some(size_msg) = msg.downcast_ref::<bubbletea_rs::event::WindowSizeMsg>() {
            let resize_msg = AppMsg::WindowResized {
                width: size_msg.width,
                height: size_msg.height,
            };
            return self.handle_message(&resize_msg);
        }

        None
    }

    fn view(&self) -> String {
        // If help is shown, render overlay instead
        if self.show_help {
            return self.render_help_overlay();
        }

        let mut output = String::new();

        output.push_str(&self.render_header());
        output.push_str(&self.render_filter_bar());
        output.push('\n');

        // Calculate layout heights
        // Layout: header (1) + filter bar (1) + newline (1) + list + detail + status bar (1)
        // Reserve space for detail pane (minimum 8 lines) and chrome (4 lines)
        let chrome_height = 4_usize; // header + filter bar + newline + status bar
        let detail_height = 8_usize; // minimum height for detail pane
        let total_height = self.height as usize;
        let list_height = total_height
            .saturating_sub(chrome_height)
            .saturating_sub(detail_height);

        let list_ctx = ReviewListViewContext {
            reviews: &self.reviews,
            filtered_indices: &self.filtered_indices,
            cursor_position: self.filter_state.cursor_position,
            scroll_offset: self.filter_state.scroll_offset,
            visible_height: list_height,
        };
        let list_view = self.review_list.view(&list_ctx);
        output.push_str(&list_view);

        // Render comment detail pane
        let detail_ctx = CommentDetailViewContext {
            selected_comment: self.selected_comment(),
            max_width: 80.min(self.width as usize),
            max_height: detail_height,
        };
        output.push_str(&self.comment_detail.view(&detail_ctx));

        output.push('\n');
        output.push_str(&self.render_status_bar());

        output
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
