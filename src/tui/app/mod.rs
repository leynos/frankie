//! Main TUI application model implementing the MVU pattern.
//!
//! This module provides the core application state and update logic for the
//! review listing TUI. It coordinates between components, manages filter state,
//! and handles async data loading.
//!
//! # Module Structure
//!
//! - `codex_handlers`: Codex execution trigger and stream polling
//! - `diff_context_handlers`: Full-screen diff context view management
//! - `filter_handlers`: Review filter application and cycling
//! - `model_impl`: `bubbletea_rs::Model` trait implementation
//! - `navigation`: Cursor and page navigation handlers
//! - `rendering`: View rendering methods for terminal output
//! - `routing`: Mode-aware message routing and category dispatch
//! - `sync_handlers`: Background sync and refresh handling
//! - `time_travel_handlers`: Time-travel navigation handlers

use std::sync::Arc;

use bubbletea_rs::Cmd;

use crate::ai::{CodexExecutionHandle, CodexExecutionService, SystemCodexExecutionService};
use crate::github::models::ReviewComment;
use crate::local::GitOperations;

use super::components::{CommentDetailComponent, DiffContextComponent, ReviewListComponent};
use super::messages::AppMsg;
use super::state::{DiffContextState, FilterState, ReviewFilter, TimeTravelState};

mod codex_handlers;
mod diff_context_handlers;
mod filter_handlers;
mod model_impl;
mod navigation;
mod rendering;
mod routing;
mod sync_handlers;
mod time_travel_handlers;

use routing::MessageRouting;

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
    /// Diff context component.
    diff_context_component: DiffContextComponent,
    /// Full-screen diff context state.
    diff_context_state: DiffContextState,
    /// Active view mode.
    view_mode: ViewMode,
    /// ID of the currently selected comment, used to restore cursor after sync.
    pub(crate) selected_comment_id: Option<u64>,
    /// Time-travel navigation state.
    time_travel_state: Option<TimeTravelState>,
    /// Git operations for time-travel (optional, requires local repo).
    git_ops: Option<Arc<dyn GitOperations>>,
    /// HEAD commit SHA for line mapping verification.
    head_sha: Option<String>,
    /// Service used to execute Codex runs.
    codex_service: Arc<dyn CodexExecutionService>,
    /// Active Codex execution handle while a run is in progress.
    codex_handle: Option<CodexExecutionHandle>,
    /// Latest Codex status line shown in the status bar.
    codex_status: Option<String>,
    /// Poll interval for draining Codex progress events.
    codex_poll_interval: std::time::Duration,
}

/// Tracks which view is currently active in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    ReviewList,
    DiffContext,
    TimeTravel,
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
            diff_context_component: DiffContextComponent::new(),
            diff_context_state: DiffContextState::default(),
            view_mode: ViewMode::ReviewList,
            selected_comment_id,
            time_travel_state: None,
            git_ops: None,
            head_sha: None,
            codex_service: Arc::new(SystemCodexExecutionService::new()),
            codex_handle: None,
            codex_status: None,
            codex_poll_interval: std::time::Duration::from_millis(150),
        }
    }

    /// Creates an empty application (for initial loading state).
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Sets the git operations for time-travel navigation.
    ///
    /// Call this method after creating the app if a local Git repository is
    /// available to enable time-travel functionality.
    #[must_use]
    pub fn with_git_ops(mut self, git_ops: Arc<dyn GitOperations>, head_sha: String) -> Self {
        self.git_ops = Some(git_ops);
        self.head_sha = Some(head_sha);
        self
    }

    /// Sets the Codex execution service used by this app instance.
    #[must_use]
    pub fn with_codex_service(mut self, codex_service: Arc<dyn CodexExecutionService>) -> Self {
        self.codex_service = codex_service;
        self
    }

    /// Sets the poll interval used when draining Codex progress events.
    #[must_use]
    pub const fn with_codex_poll_interval(mut self, interval: std::time::Duration) -> Self {
        self.codex_poll_interval = interval;
        self
    }

    /// Returns whether a Codex execution run is currently active.
    #[must_use]
    pub(super) const fn is_codex_running(&self) -> bool {
        self.codex_handle.is_some()
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

    /// Returns the current TUI error message, if any.
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Returns the latest Codex status line, if any.
    #[must_use]
    pub fn codex_status_message(&self) -> Option<&str> {
        self.codex_status.as_deref()
    }

    /// Returns the ID of the currently selected comment, if any.
    #[must_use]
    pub fn current_selected_id(&self) -> Option<u64> {
        self.selected_comment().map(|r| r.id)
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

    /// Dispatches time-travel messages to their handlers.
    fn handle_time_travel_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::EnterTimeTravel => self.handle_enter_time_travel(),
            AppMsg::ExitTimeTravel => self.handle_exit_time_travel(),
            AppMsg::TimeTravelLoaded(state) => self.handle_time_travel_loaded(state.clone()),
            AppMsg::TimeTravelFailed(error) => self.handle_time_travel_failed(error),
            AppMsg::NextCommit => self.handle_next_commit(),
            AppMsg::PreviousCommit => self.handle_previous_commit(),
            AppMsg::CommitNavigated(state) => self.handle_commit_navigated(state.clone()),
            _ => None,
        }
    }

    /// Handles a message and updates state accordingly.
    ///
    /// This method is the core update function that processes all application
    /// messages and returns any resulting commands. It first attempts mode-based
    /// routing, then falls back to category-based dispatch.
    #[doc(hidden)]
    pub fn handle_message(&mut self, msg: &AppMsg) -> Option<Cmd> {
        if let MessageRouting::Handled(result) = self.route_by_view_mode(msg) {
            return result;
        }
        self.dispatch_by_message_category(msg)
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
                // Unreachable: caller filters to navigation messages.
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
                // Unreachable: caller filters to filter messages.
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
                // Unreachable: caller filters to data messages.
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
                // Unreachable: caller filters to lifecycle messages.
                None
            }
        }
    }

    // Window event handlers

    fn handle_resize(&mut self, width: u16, height: u16) -> Option<Cmd> {
        self.width = width;
        self.height = height;
        let list_height = height.saturating_sub(4) as usize;
        self.review_list.set_visible_height(list_height);
        if self.view_mode == ViewMode::DiffContext
            && self.diff_context_state.cached_width() != width as usize
        {
            self.rebuild_diff_context_state();
        }
        None
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "help_overlay_input_tests.rs"]
mod help_overlay_input_tests;
