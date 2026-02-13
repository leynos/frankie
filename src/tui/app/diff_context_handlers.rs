//! Diff context view handlers for the review TUI.
//!
//! This module manages the full-screen diff context view, including entering
//! and exiting the view and handling hunk navigation messages.

use bubbletea_rs::Cmd;

use super::{ReviewApp, ViewMode};
use crate::tui::messages::AppMsg;
use crate::tui::state::{collect_diff_hunks, find_hunk_index};

impl ReviewApp {
    /// Rebuilds the diff context state from the current filtered reviews.
    pub(super) fn rebuild_diff_context_state(&mut self) {
        let max_width = self.width as usize;
        let hunks = collect_diff_hunks(&self.reviews, &self.filtered_indices);
        let preferred_index = find_hunk_index(&hunks, self.selected_comment());
        let rendered = self.diff_context_component.render_hunks(&hunks, max_width);

        self.diff_context_state
            .rebuild(rendered, max_width, preferred_index);
    }

    /// Enters the full-screen diff context view.
    pub(super) fn enter_diff_context(&mut self) {
        self.rebuild_diff_context_state();
        self.view_mode = ViewMode::DiffContext;
    }

    /// Exits the full-screen diff context view.
    pub(super) const fn exit_diff_context(&mut self) {
        self.view_mode = ViewMode::ReviewList;
    }

    /// Handles diff context navigation messages.
    pub(super) fn handle_diff_context_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::ShowDiffContext => {
                self.enter_diff_context();
                None
            }
            AppMsg::HideDiffContext | AppMsg::EscapePressed => {
                self.exit_diff_context();
                None
            }
            AppMsg::NextHunk => {
                self.diff_context_state.move_next();
                None
            }
            AppMsg::PreviousHunk => {
                self.diff_context_state.move_previous();
                None
            }
            _ => {
                // Unreachable: caller filters to diff-context messages.
                None
            }
        }
    }
}
