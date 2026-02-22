//! `Model` trait implementation for the review TUI application.
//!
//! This module contains the `bubbletea_rs::Model` trait implementation for
//! `ReviewApp`, handling initialisation, update dispatch, and view rendering.

use std::any::Any;

use bubbletea_rs::{Cmd, Model};
use unicode_width::UnicodeWidthChar;

use super::ReviewApp;
use crate::tui::app::ViewMode;
use crate::tui::components::{
    CommentDetailViewContext, ReplyDraftRenderContext, ReviewListViewContext,
};
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
        let safe_terminal_width = (self.width as usize).saturating_sub(1).max(1);

        let list_ctx = ReviewListViewContext {
            reviews: &self.reviews,
            filtered_indices: &self.filtered_indices,
            cursor_position: self.filter_state.cursor_position,
            scroll_offset: self.filter_state.scroll_offset,
            visible_height: list_height,
            max_width: safe_terminal_width,
        };
        let list_view = self.review_list.view(&list_ctx);
        output.push_str(&list_view);

        // Render comment detail pane
        let detail_height = self.calculate_detail_height();
        if detail_height > 0 {
            let selected_comment = self.selected_comment();
            let reply_draft =
                selected_comment.and_then(|comment| self.reply_draft_for_comment(comment.id));
            let detail_ctx = CommentDetailViewContext {
                selected_comment,
                max_width: safe_terminal_width,
                max_height: detail_height,
                reply_draft,
            };
            output.push_str(&self.comment_detail.view(&detail_ctx));
        }
        output.push_str(&self.render_status_bar());

        self.normalise_viewport(&output)
    }
}

impl ReviewApp {
    /// Returns the current input context for context-aware key mapping.
    pub(super) fn input_context(&self) -> InputContext {
        if self.resume_prompt.is_some() {
            return InputContext::ResumePrompt;
        }
        if self.has_reply_draft_for_current_selection() {
            return InputContext::ReplyDraft;
        }
        match self.view_mode {
            ViewMode::ReviewList => InputContext::ReviewList,
            ViewMode::DiffContext => InputContext::DiffContext,
            ViewMode::TimeTravel => InputContext::TimeTravel,
        }
    }

    fn has_reply_draft_for_current_selection(&self) -> bool {
        let Some(draft) = self.reply_draft.as_ref() else {
            return false;
        };
        let Some(selected_comment) = self.selected_comment() else {
            return false;
        };

        draft.comment_id() == selected_comment.id
    }

    fn reply_draft_for_comment(&self, comment_id: u64) -> Option<ReplyDraftRenderContext<'_>> {
        let draft = self.reply_draft.as_ref()?;
        if draft.comment_id() != comment_id {
            return None;
        }

        Some(ReplyDraftRenderContext {
            text: draft.text(),
            char_count: draft.char_count(),
            max_length: draft.max_length(),
            ready_to_send: draft.is_ready_to_send(),
        })
    }

    /// Normalizes the rendered frame to terminal dimensions.
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
    truncate_ansi_line(line, width)
}

fn truncate_ansi_line(line: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let mut output = String::new();
    let mut visible_chars = 0usize;
    let mut in_escape = false;
    let mut had_ansi = false;
    let mut ended_with_reset = false;
    let mut escape_buffer = String::new();

    for ch in line.chars() {
        if in_escape {
            output.push(ch);
            escape_buffer.push(ch);
            if ch.is_ascii_alphabetic() {
                in_escape = false;
                update_reset_tracking(&escape_buffer, &mut ended_with_reset);
            }
            continue;
        }

        if ch == '\x1b' {
            in_escape = true;
            had_ansi = true;
            output.push(ch);
            escape_buffer.clear();
            escape_buffer.push(ch);
            continue;
        }

        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if char_width == 0 {
            output.push(ch);
            continue;
        }

        if visible_chars.saturating_add(char_width) > width {
            break;
        }

        output.push(ch);
        visible_chars = visible_chars.saturating_add(char_width);
    }
    add_padding(&mut output, visible_chars, width);
    add_ansi_reset_if_needed(&mut output, had_ansi, ended_with_reset);

    output
}

fn add_padding(output: &mut String, visible_chars: usize, width: usize) {
    if visible_chars < width {
        output.push_str(&" ".repeat(width - visible_chars));
    }
}

fn add_ansi_reset_if_needed(output: &mut String, had_ansi: bool, ended_with_reset: bool) {
    if had_ansi && !ended_with_reset {
        // Add a defensive reset when styles may still be active.
        output.push_str("\x1b[0m");
    }
}

fn update_reset_tracking(escape_sequence: &str, ended_with_reset: &mut bool) {
    if let Some(is_reset_sequence) = sgr_sequence_resets_styles(escape_sequence) {
        *ended_with_reset = is_reset_sequence;
    }
}

fn sgr_sequence_resets_styles(escape_sequence: &str) -> Option<bool> {
    let params = escape_sequence.strip_prefix("\x1b[")?.strip_suffix('m')?;
    if params.is_empty() {
        return Some(true);
    }

    Some(
        params
            .split(';')
            .all(|param| param.is_empty() || param == "0"),
    )
}

#[cfg(test)]
mod tests {
    use super::{normalise_lines_to_height, pad_or_truncate_line, sgr_sequence_resets_styles};

    #[test]
    fn pad_or_truncate_line_resets_ansi_without_explicit_reset() {
        let line = "\u{1b}[31mred";
        let result = pad_or_truncate_line(line, 3);

        assert_eq!(result, "\u{1b}[31mred\u{1b}[0m");
    }

    #[test]
    fn pad_or_truncate_line_avoids_duplicate_reset_when_line_is_already_reset() {
        let line = "\u{1b}[31mred\u{1b}[0m";
        let result = pad_or_truncate_line(line, 3);

        assert_eq!(result, line);
    }

    #[test]
    fn pad_or_truncate_line_adds_reset_after_non_reset_sgr_with_zero_prefix() {
        let line = "\u{1b}[0;31mred";
        let result = pad_or_truncate_line(line, 3);

        assert_eq!(result, "\u{1b}[0;31mred\u{1b}[0m");
    }

    #[test]
    fn pad_or_truncate_line_handles_wide_characters() {
        let result = pad_or_truncate_line("你好世界", 5);
        assert_eq!(result, "你好 ");
    }

    #[test]
    fn sgr_sequence_resets_styles_only_for_true_reset_sequences() {
        assert_eq!(sgr_sequence_resets_styles("\u{1b}[m"), Some(true));
        assert_eq!(sgr_sequence_resets_styles("\u{1b}[0m"), Some(true));
        assert_eq!(sgr_sequence_resets_styles("\u{1b}[0;0m"), Some(true));
        assert_eq!(sgr_sequence_resets_styles("\u{1b}[31m"), Some(false));
        assert_eq!(sgr_sequence_resets_styles("\u{1b}[0;31m"), Some(false));
        assert_eq!(sgr_sequence_resets_styles("\u{1b}[K"), None);
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
