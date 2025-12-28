//! Main TUI application model implementing the MVU pattern.
//!
//! This module provides the core application state and update logic for the
//! review listing TUI. It coordinates between components, manages filter state,
//! and handles async data loading.

use std::any::Any;

use bubbletea_rs::{Cmd, Model};

use crate::github::models::ReviewComment;

use super::components::{ReviewListComponent, ReviewListViewContext};
use super::input::map_key_to_message;
use super::messages::AppMsg;
use super::state::{FilterState, ReviewFilter};

/// Main application model for the review listing TUI.
#[derive(Debug, Clone)]
pub struct ReviewApp {
    /// All review comments (unfiltered).
    reviews: Vec<ReviewComment>,
    /// Cached indices of reviews matching the current filter.
    /// Invalidated when reviews or filter change.
    filtered_indices: Vec<usize>,
    /// Filter and cursor state.
    filter_state: FilterState,
    /// Whether data is currently loading.
    loading: bool,
    /// Current error message, if any.
    error: Option<String>,
    /// Terminal dimensions.
    width: u16,
    height: u16,
    /// Whether help overlay is visible.
    show_help: bool,
    /// Review list component.
    review_list: ReviewListComponent,
}

impl ReviewApp {
    /// Creates a new application with the given review comments.
    #[must_use]
    pub fn new(reviews: Vec<ReviewComment>) -> Self {
        // Build initial cache with all indices (default filter is All)
        let filtered_indices = (0..reviews.len()).collect();
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
    fn rebuild_filter_cache(&mut self) {
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

    /// Handles a message and updates state accordingly.
    fn handle_message(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            // Navigation
            AppMsg::CursorUp => self.handle_cursor_up(),
            AppMsg::CursorDown => self.handle_cursor_down(),
            AppMsg::PageUp => self.handle_page_up(),
            AppMsg::PageDown => self.handle_page_down(),
            AppMsg::Home => self.handle_home(),
            AppMsg::End => self.handle_end(),

            // Filter changes
            AppMsg::SetFilter(filter) => self.handle_set_filter(filter),
            AppMsg::ClearFilter => self.handle_clear_filter(),
            AppMsg::CycleFilter => self.handle_cycle_filter(),

            // Data loading
            AppMsg::RefreshRequested => self.handle_refresh_requested(),
            AppMsg::RefreshComplete(new_reviews) => self.handle_refresh_complete(new_reviews),
            AppMsg::RefreshFailed(error_msg) => self.handle_refresh_failed(error_msg),

            // Application lifecycle
            AppMsg::Quit => Some(bubbletea_rs::quit()),
            AppMsg::ToggleHelp => {
                self.show_help = !self.show_help;
                None
            }

            // Window events
            AppMsg::WindowResized { width, height } => self.handle_resize(*width, *height),
        }
    }

    // Navigation handlers

    fn handle_cursor_up(&mut self) -> Option<Cmd> {
        self.filter_state.cursor_up();
        None
    }

    fn handle_cursor_down(&mut self) -> Option<Cmd> {
        let max_index = self.filtered_count().saturating_sub(1);
        self.filter_state.cursor_down(max_index);
        None
    }

    fn handle_page_up(&mut self) -> Option<Cmd> {
        let page_size = self.review_list.visible_height();
        self.filter_state.page_up(page_size);
        None
    }

    fn handle_page_down(&mut self) -> Option<Cmd> {
        let page_size = self.review_list.visible_height();
        let max_index = self.filtered_count().saturating_sub(1);
        self.filter_state.page_down(page_size, max_index);
        None
    }

    fn handle_home(&mut self) -> Option<Cmd> {
        self.filter_state.home();
        None
    }

    fn handle_end(&mut self) -> Option<Cmd> {
        let max_index = self.filtered_count().saturating_sub(1);
        self.filter_state.end(max_index);
        None
    }

    // Filter handlers

    fn handle_set_filter(&mut self, filter: &ReviewFilter) -> Option<Cmd> {
        self.filter_state.active_filter = filter.clone();
        self.rebuild_filter_cache();
        self.filter_state.clamp_cursor(self.filtered_count());
        None
    }

    fn handle_clear_filter(&mut self) -> Option<Cmd> {
        self.filter_state.active_filter = ReviewFilter::All;
        self.rebuild_filter_cache();
        self.filter_state.clamp_cursor(self.filtered_count());
        None
    }

    /// Cycles the active filter between `All` and `Unresolved`.
    ///
    /// This method only toggles between the two primary filter modes:
    /// - From `All` → switches to `Unresolved`
    /// - From any other filter (including `ByFile`, `ByReviewer`, `ByCommitRange`)
    ///   → resets to `All`
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
        self.filter_state.clamp_cursor(self.filtered_count());
        None
    }

    // Data loading handlers

    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns Option<Cmd> for consistency with other message handlers"
    )]
    fn handle_refresh_requested(&mut self) -> Option<Cmd> {
        self.loading = true;
        self.error = None;

        // Return a command that fetches reviews asynchronously
        Some(Box::pin(async {
            match super::fetch_reviews().await {
                Ok(reviews) => {
                    Some(Box::new(AppMsg::RefreshComplete(reviews)) as Box<dyn Any + Send>)
                }
                Err(error) => {
                    Some(Box::new(AppMsg::RefreshFailed(error.to_string())) as Box<dyn Any + Send>)
                }
            }
        }))
    }

    fn handle_refresh_complete(&mut self, new_reviews: &[ReviewComment]) -> Option<Cmd> {
        self.reviews = new_reviews.to_vec();
        self.rebuild_filter_cache();
        self.loading = false;
        self.error = None;
        self.filter_state.clamp_cursor(self.filtered_count());
        None
    }

    fn handle_refresh_failed(&mut self, error_msg: &str) -> Option<Cmd> {
        self.loading = false;
        self.error = Some(error_msg.to_owned());
        None
    }

    fn handle_resize(&mut self, width: u16, height: u16) -> Option<Cmd> {
        self.width = width;
        self.height = height;
        let list_height = height.saturating_sub(4) as usize;
        self.review_list.set_visible_height(list_height);
        None
    }

    /// Renders the header bar.
    fn render_header(&self) -> String {
        let title = "Frankie - Review Comments";
        let loading_indicator = if self.loading { " [Loading...]" } else { "" };
        format!("{title}{loading_indicator}\n")
    }

    /// Renders the filter bar showing active filter.
    fn render_filter_bar(&self) -> String {
        let label = self.filter_state.active_filter.label();
        let count = self.filtered_count();
        let total = self.reviews.len();
        format!("Filter: {label} ({count}/{total})\n")
    }

    /// Renders the status bar with help hints.
    fn render_status_bar(&self) -> String {
        if let Some(error) = &self.error {
            return format!("Error: {error}\n");
        }

        let hints = "j/k:navigate  f:filter  r:refresh  ?:help  q:quit";
        format!("{hints}\n")
    }

    /// Renders the help overlay if visible.
    fn render_help_overlay(&self) -> String {
        if !self.show_help {
            return String::new();
        }

        let help_text = r"
=== Keyboard Shortcuts ===

Navigation:
  j, Down    Move cursor down
  k, Up      Move cursor up
  PgDn       Page down
  PgUp       Page up
  Home, g    Go to first item
  End, G     Go to last item

Filtering:
  f          Cycle filter (All/Unresolved)
  Esc        Clear filter

Other:
  r          Refresh from GitHub
  ?          Toggle this help
  q          Quit

Press any key to close this help.
";
        help_text.to_owned()
    }
}

impl Model for ReviewApp {
    fn init() -> (Self, Option<Cmd>) {
        // Retrieve initial data from module-level storage
        let reviews = super::get_initial_reviews();
        (Self::new(reviews), None)
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

        let ctx = ReviewListViewContext {
            reviews: &self.reviews,
            filtered_indices: &self.filtered_indices,
            cursor_position: self.filter_state.cursor_position,
            scroll_offset: self.filter_state.scroll_offset,
        };
        let list_view = self.review_list.view(&ctx);
        output.push_str(&list_view);

        output.push('\n');
        output.push_str(&self.render_status_bar());

        output
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
