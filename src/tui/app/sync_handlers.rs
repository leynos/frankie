//! Background sync and refresh handlers for the review TUI.
//!
//! This module contains the message handlers related to incremental sync
//! and data refresh functionality. These handlers manage the sync timer,
//! process incoming review data, and preserve selection across updates.

use std::any::Any;
use std::time::Duration;

use bubbletea_rs::Cmd;

use super::ReviewApp;
use crate::github::models::ReviewComment;
use crate::tui::app::ViewMode;
use crate::tui::messages::AppMsg;

/// Default interval between background syncs.
pub(super) const SYNC_INTERVAL: Duration = Duration::from_secs(30);

impl ReviewApp {
    /// Dispatches data loading and sync messages to their handlers.
    pub(super) fn handle_data_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
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

    /// Handles a manual refresh request by delegating to sync logic.
    ///
    /// This ensures consistent behaviour between manual refresh and
    /// background sync, including selection preservation.
    pub(super) fn handle_refresh_requested(&mut self) -> Option<Cmd> {
        // Delegate to sync tick for consistent behaviour
        self.handle_sync_tick()
    }

    /// Applies new reviews with incremental merge and selection preservation.
    ///
    /// This is the shared logic for both manual refresh and background sync:
    /// 1. Captures current selection by ID
    /// 2. Merges reviews using ID-based tracking
    /// 3. Rebuilds filter cache
    /// 4. Restores selection by ID, or clamps if deleted
    /// 5. Clears loading state and error
    pub(super) fn apply_new_reviews(&mut self, new_reviews: &[ReviewComment]) {
        // Capture current selection
        let selected_id = self.selected_comment_id;

        // Merge reviews using incremental sync
        let merge_result = crate::tui::sync::merge_reviews(&self.reviews, new_reviews.to_vec());
        self.reviews = merge_result.reviews;

        // Rebuild filter cache
        self.rebuild_filter_cache();

        // Restore selection by ID, or clamp if deleted
        if let Some(id) = selected_id {
            if let Some(new_index) = self.find_filtered_index_by_id(id) {
                self.filter_state.cursor_position = new_index;
            } else {
                self.filter_state.clamp_cursor(self.filtered_count());
            }
        }

        self.adjust_scroll_to_cursor();
        // Update selected_comment_id to match new cursor position
        self.update_selected_id();

        if self.view_mode == ViewMode::DiffContext {
            self.rebuild_diff_context_state();
        }

        self.loading = false;
        self.error = None;
    }

    /// Handles legacy refresh complete (for backward compatibility).
    pub(super) fn handle_refresh_complete(&mut self, new_reviews: &[ReviewComment]) -> Option<Cmd> {
        self.apply_new_reviews(new_reviews);
        None
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns Option<Cmd> for consistency with other message handlers"
    )]
    pub(super) fn handle_refresh_failed(&mut self, error_msg: &str) -> Option<Cmd> {
        self.loading = false;
        self.error = Some(error_msg.to_owned());
        // Re-arm the sync timer so that transient failures don't stop periodic sync
        Some(Self::arm_sync_timer())
    }

    /// Handles a background sync timer tick.
    ///
    /// Skips the sync if already loading to prevent duplicate requests.
    /// Returns a command that fetches reviews and records timing.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns Option<Cmd> for consistency with other message handlers"
    )]
    pub(super) fn handle_sync_tick(&mut self) -> Option<Cmd> {
        // Don't start new sync if already loading
        if self.loading {
            return Some(Self::arm_sync_timer());
        }

        self.loading = true;
        self.error = None;

        Some(Box::pin(async {
            let start = std::time::Instant::now();
            match crate::tui::fetch_reviews().await {
                Ok(reviews) => {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "Latency over u64::MAX milliseconds is unrealistic"
                    )]
                    let latency_ms = start.elapsed().as_millis() as u64;
                    Some(Box::new(AppMsg::SyncComplete {
                        reviews,
                        latency_ms,
                    }) as Box<dyn Any + Send>)
                }
                Err(error) => {
                    Some(Box::new(AppMsg::RefreshFailed(error.to_string())) as Box<dyn Any + Send>)
                }
            }
        }))
    }

    /// Handles successful sync completion with incremental merge.
    ///
    /// Delegates to `apply_new_reviews` for the merge/selection logic,
    /// then records telemetry and re-arms the sync timer.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns Option<Cmd> for consistency with other message handlers"
    )]
    pub(super) fn handle_sync_complete(
        &mut self,
        new_reviews: &[ReviewComment],
        latency_ms: u64,
    ) -> Option<Cmd> {
        self.apply_new_reviews(new_reviews);

        // Log telemetry
        crate::tui::record_sync_telemetry(latency_ms, self.reviews.len(), true);

        // Re-arm sync timer
        Some(Self::arm_sync_timer())
    }

    /// Creates a command that triggers a sync tick after the sync interval.
    pub(super) fn arm_sync_timer() -> Cmd {
        Box::pin(async {
            tokio::time::sleep(SYNC_INTERVAL).await;
            Some(Box::new(AppMsg::SyncTick) as Box<dyn Any + Send>)
        })
    }

    /// Creates a command that emits `Initialized` immediately.
    ///
    /// This synthetic startup event triggers the first render cycle without
    /// waiting for user input.
    pub(super) fn immediate_init_cmd() -> Cmd {
        Box::pin(async { Some(Box::new(AppMsg::Initialized) as Box<dyn Any + Send>) })
    }
}
