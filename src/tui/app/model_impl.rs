//! `Model` trait implementation for the review TUI application.
//!
//! This module contains the `bubbletea_rs::Model` trait implementation for
//! `ReviewApp`, handling initialisation, update dispatch, and view rendering.

use std::any::Any;

use bubbletea_rs::{Cmd, Model};
use unicode_width::UnicodeWidthChar;

use super::ReviewApp;
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
            model = model.with_git_ops(git_ops, head_sha);
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
            return self.normalise_viewport(&self.render_help_overlay());
        }

        // Handle special view modes with early returns
        if self.view_mode == ViewMode::DiffContext {
            return self.normalise_viewport(&self.render_diff_context_view());
        }
        if self.view_mode == ViewMode::TimeTravel {
            return self.normalise_viewport(&self.render_time_travel_view());
        }

        // Render main ReviewList view
        let mut output = String::new();

        output.push_str(&self.render_header());
        output.push_str(&self.render_filter_bar());
        output.push('\n');

        let list_height = self.calculate_list_height();
        let terminal_width = (self.width as usize).max(1);

        let list_ctx = ReviewListViewContext {
            reviews: &self.reviews,
            filtered_indices: &self.filtered_indices,
            cursor_position: self.filter_state.cursor_position,
            scroll_offset: self.filter_state.scroll_offset,
            visible_height: list_height,
            max_width: terminal_width,
        };
        let list_view = self.review_list.view(&list_ctx);
        output.push_str(&list_view);

        // Render comment detail pane
        let detail_height = self.calculate_detail_height();
        let detail_ctx = CommentDetailViewContext {
            selected_comment: self.selected_comment(),
            max_width: terminal_width,
            max_height: detail_height,
        };
        output.push_str(&self.comment_detail.view(&detail_ctx));
        output.push_str(&self.render_status_bar());

        self.normalise_viewport(&output)
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

    /// Normalises the rendered frame to terminal dimensions.
    ///
    /// The output stream from components can leave stale trailing cells behind
    /// when rows are shorter than previous frames, especially after resize.
    /// We clamp rows to one column less than terminal width to avoid autowrap
    /// behaviour, while still padding with spaces to clear stale trailing
    /// cells after resize.
    fn normalise_viewport(&self, output: &str) -> String {
        let width = self.width.max(1) as usize;
        let safe_width = width.saturating_sub(1).max(1);
        let height = self.height.max(1) as usize;

        let lines: Vec<String> = output
            .lines()
            .map(|line| pad_or_truncate_line(line, safe_width))
            .collect();

        normalise_lines_to_height(lines, height, safe_width)
    }
}

fn normalise_lines_to_height(mut lines: Vec<String>, height: usize, width: usize) -> String {
    lines.truncate(height);

    let missing = height.saturating_sub(lines.len());
    let blank = " ".repeat(width);
    lines.extend(std::iter::repeat_with(|| blank.clone()).take(missing));

    let mut normalised = lines.join("\n");
    normalised.push('\n');
    normalised
}

fn pad_or_truncate_line(line: &str, width: usize) -> String {
    let mut output = String::new();
    let mut visible = 0usize;
    let mut in_escape = false;
    let mut had_ansi = false;

    for ch in line.chars() {
        if in_escape {
            output.push(ch);
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }

        if ch == '\x1b' {
            in_escape = true;
            had_ansi = true;
            output.push(ch);
            continue;
        }

        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if char_width == 0 {
            output.push(ch);
            continue;
        }

        if visible.saturating_add(char_width) > width {
            break;
        }

        output.push(ch);
        visible = visible.saturating_add(char_width);
    }

    if visible < width {
        output.push_str(&" ".repeat(width - visible));
    }

    if had_ansi {
        // Always reset when ANSI was seen to avoid style leakage between lines.
        output.push_str("\x1b[0m");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{normalise_lines_to_height, pad_or_truncate_line};

    #[test]
    fn pad_or_truncate_line_resets_ansi_without_explicit_reset() {
        let line = "\u{1b}[31mred";
        let result = pad_or_truncate_line(line, 3);

        assert_eq!(result, "\u{1b}[31mred\u{1b}[0m");
    }

    #[test]
    fn pad_or_truncate_line_handles_wide_characters() {
        let result = pad_or_truncate_line("你好世界", 5);
        assert_eq!(result, "你好 ");
    }

    #[test]
    fn normalise_lines_to_height_pads_missing_rows() {
        let result = normalise_lines_to_height(vec!["abcd".to_owned()], 3, 4);
        assert_eq!(result, "abcd\n    \n    \n");
    }

    #[test]
    fn normalise_lines_to_height_truncates_extra_rows() {
        let lines = vec!["1111".to_owned(), "2222".to_owned(), "3333".to_owned()];
        let result = normalise_lines_to_height(lines, 2, 4);
        assert_eq!(result, "1111\n2222\n");
    }
}
