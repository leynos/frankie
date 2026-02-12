//! `Model` trait implementation for the review TUI application.
//!
//! This module contains the `bubbletea_rs::Model` trait implementation for
//! `ReviewApp`, handling initialisation, update dispatch, and view rendering.

use std::any::Any;

use bubbletea_rs::{Cmd, Model};

use super::{DETAIL_HEIGHT, ReviewApp};
use crate::tui::app::ViewMode;
use crate::tui::components::{CommentDetailViewContext, ReviewListViewContext};
use crate::tui::input::{InputContext, map_key_to_message_with_context};
use crate::tui::messages::AppMsg;

impl Model for ReviewApp {
    fn init() -> (Self, Option<Cmd>) {
        // Retrieve initial data from module-level storage
        let reviews = crate::tui::get_initial_reviews();
        let mut model = Self::new(reviews);

        // Wire up git operations for time-travel if available
        if let Some((git_ops, head_sha)) = crate::tui::get_git_ops_context() {
            model.git_ops = Some(git_ops);
            model.head_sha = Some(head_sha);
        }

        // Emit an immediate startup message to trigger the first render cycle.
        // The sync timer is armed when `AppMsg::Initialized` is handled.
        let cmd = Self::immediate_init_cmd();

        (model, Some(cmd))
    }

    fn update(&mut self, msg: Box<dyn Any + Send>) -> Option<Cmd> {
        // Try to downcast to our message type
        if let Some(app_msg) = msg.downcast_ref::<AppMsg>() {
            return self.handle_message(app_msg);
        }

        // Handle key events from bubbletea-rs with context-aware mapping
        if let Some(key_msg) = msg.downcast_ref::<bubbletea_rs::event::KeyMsg>() {
            if self.show_help {
                return self.handle_message(&AppMsg::ToggleHelp);
            }
            let context = self.input_context();
            let app_msg = map_key_to_message_with_context(key_msg, context);
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

        // Handle special view modes with early returns
        if self.view_mode == ViewMode::DiffContext {
            return self.render_diff_context_view();
        }
        if self.view_mode == ViewMode::TimeTravel {
            return self.render_time_travel_view();
        }

        // Render main ReviewList view
        let mut output = String::new();

        output.push_str(&self.render_header());
        output.push_str(&self.render_filter_bar());
        output.push('\n');

        let list_height = self.calculate_list_height();

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
            max_height: DETAIL_HEIGHT,
        };
        output.push_str(&self.comment_detail.view(&detail_ctx));

        output.push('\n');
        output.push_str(&self.render_status_bar());

        output
    }
}

impl ReviewApp {
    /// Returns the current input context for context-aware key mapping.
    pub(super) const fn input_context(&self) -> InputContext {
        match self.view_mode {
            ViewMode::ReviewList => InputContext::ReviewList,
            ViewMode::DiffContext => InputContext::DiffContext,
            ViewMode::TimeTravel => InputContext::TimeTravel,
        }
    }
}
