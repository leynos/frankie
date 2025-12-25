//! Main TUI application model implementing the MVU pattern.
//!
//! This module provides the core application state and update logic for the
//! review listing TUI. It coordinates between components, manages filter state,
//! and handles async data loading.

use std::any::Any;

use bubbletea_rs::{Cmd, Model};

use crate::github::models::ReviewComment;

use super::components::ReviewListComponent;
use super::messages::AppMsg;
use super::state::{FilterState, ReviewFilter};

/// Main application model for the review listing TUI.
#[derive(Debug, Clone)]
pub struct ReviewApp {
    /// All review comments (unfiltered).
    reviews: Vec<ReviewComment>,
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
        Self {
            reviews,
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
        self.filter_state.apply_filter(&self.reviews)
    }

    /// Returns the count of filtered reviews.
    #[must_use]
    pub fn filtered_count(&self) -> usize {
        self.filtered_reviews().len()
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
        let new_count = self.reviews.iter().filter(|r| filter.matches(r)).count();
        self.filter_state.set_filter(filter.clone(), new_count);
        None
    }

    fn handle_clear_filter(&mut self) -> Option<Cmd> {
        let count = self.reviews.len();
        self.filter_state.set_filter(ReviewFilter::All, count);
        None
    }

    fn handle_cycle_filter(&mut self) -> Option<Cmd> {
        let next_filter = match &self.filter_state.active_filter {
            ReviewFilter::All => ReviewFilter::Unresolved,
            _ => ReviewFilter::All,
        };
        let new_count = self
            .reviews
            .iter()
            .filter(|r| next_filter.matches(r))
            .count();
        self.filter_state.set_filter(next_filter, new_count);
        None
    }

    // Data loading handlers

    fn handle_refresh_requested(&mut self) -> Option<Cmd> {
        self.loading = true;
        self.error = None;
        None
    }

    fn handle_refresh_complete(&mut self, new_reviews: &[ReviewComment]) -> Option<Cmd> {
        self.reviews = new_reviews.to_vec();
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
        let reviews = super::take_initial_reviews();
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

        let filtered = self.filtered_reviews();
        let list_view = self.review_list.view(
            &filtered,
            self.filter_state.cursor_position,
            self.filter_state.scroll_offset,
        );
        output.push_str(&list_view);

        output.push('\n');
        output.push_str(&self.render_status_bar());

        output
    }
}

/// Maps a key event to an application message.
#[expect(
    clippy::missing_const_for_fn,
    reason = "KeyCode match patterns prevent const evaluation"
)]
fn map_key_to_message(key: &bubbletea_rs::event::KeyMsg) -> Option<AppMsg> {
    use crossterm::event::KeyCode;

    match key.key {
        KeyCode::Char('q') => Some(AppMsg::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(AppMsg::CursorDown),
        KeyCode::Char('k') | KeyCode::Up => Some(AppMsg::CursorUp),
        KeyCode::PageDown => Some(AppMsg::PageDown),
        KeyCode::PageUp => Some(AppMsg::PageUp),
        KeyCode::Home | KeyCode::Char('g') => Some(AppMsg::Home),
        KeyCode::End | KeyCode::Char('G') => Some(AppMsg::End),
        KeyCode::Char('f') => Some(AppMsg::CycleFilter),
        KeyCode::Esc => Some(AppMsg::ClearFilter),
        KeyCode::Char('r') => Some(AppMsg::RefreshRequested),
        KeyCode::Char('?') => Some(AppMsg::ToggleHelp),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_reviews() -> Vec<ReviewComment> {
        vec![
            ReviewComment {
                id: 1,
                body: Some("First comment".to_owned()),
                author: Some("alice".to_owned()),
                file_path: Some("src/main.rs".to_owned()),
                line_number: Some(10),
                original_line_number: None,
                diff_hunk: None,
                commit_sha: None,
                in_reply_to_id: None,
                created_at: None,
                updated_at: None,
            },
            ReviewComment {
                id: 2,
                body: Some("Second comment".to_owned()),
                author: Some("bob".to_owned()),
                file_path: Some("src/lib.rs".to_owned()),
                line_number: Some(20),
                original_line_number: None,
                diff_hunk: None,
                commit_sha: None,
                in_reply_to_id: Some(1), // This is a reply
                created_at: None,
                updated_at: None,
            },
        ]
    }

    #[test]
    fn new_app_has_all_reviews() {
        let reviews = make_reviews();
        let app = ReviewApp::new(reviews.clone());
        assert_eq!(app.filtered_count(), 2);
    }

    #[test]
    fn cursor_navigation_works() {
        let reviews = make_reviews();
        let mut app = ReviewApp::new(reviews);

        assert_eq!(app.cursor_position(), 0);

        app.handle_message(&AppMsg::CursorDown);
        assert_eq!(app.cursor_position(), 1);

        app.handle_message(&AppMsg::CursorDown);
        assert_eq!(app.cursor_position(), 1); // Cannot go past end

        app.handle_message(&AppMsg::CursorUp);
        assert_eq!(app.cursor_position(), 0);

        app.handle_message(&AppMsg::CursorUp);
        assert_eq!(app.cursor_position(), 0); // Cannot go below 0
    }

    #[test]
    fn filter_changes_preserve_valid_cursor() {
        let reviews = make_reviews();
        let mut app = ReviewApp::new(reviews);

        app.handle_message(&AppMsg::CursorDown);
        assert_eq!(app.cursor_position(), 1);

        // Switch to unresolved filter - only 1 item matches
        app.handle_message(&AppMsg::SetFilter(ReviewFilter::Unresolved));
        assert_eq!(app.filtered_count(), 1);
        assert_eq!(app.cursor_position(), 0); // Clamped to valid range
    }

    #[test]
    fn view_renders_without_panic() {
        let reviews = make_reviews();
        let app = ReviewApp::new(reviews);
        let output = app.view();

        assert!(output.contains("Frankie"));
        assert!(output.contains("Filter:"));
        assert!(output.contains("alice"));
    }

    #[test]
    fn quit_message_returns_quit_command() {
        let mut app = ReviewApp::empty();
        let cmd = app.handle_message(&AppMsg::Quit);
        assert!(cmd.is_some());
    }

    #[test]
    fn refresh_complete_updates_data() {
        let mut app = ReviewApp::empty();
        assert_eq!(app.filtered_count(), 0);

        let new_reviews = make_reviews();
        app.handle_message(&AppMsg::RefreshComplete(new_reviews));

        assert_eq!(app.filtered_count(), 2);
        assert!(!app.loading);
    }

    #[test]
    fn toggle_help_shows_and_hides_overlay() {
        let mut app = ReviewApp::empty();
        assert!(!app.show_help);

        app.handle_message(&AppMsg::ToggleHelp);
        assert!(app.show_help);

        let view = app.view();
        assert!(view.contains("Keyboard Shortcuts"));

        app.handle_message(&AppMsg::ToggleHelp);
        assert!(!app.show_help);
    }
}
