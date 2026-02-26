//! Rendering logic for the review TUI application.
//!
//! This module contains the view rendering methods that produce string output
//! for display in the terminal. These are pure query methods that read state
//! without modification.

use super::ReviewApp;
use crate::tui::components::{
    DiffContextComponent, DiffContextViewContext, TimeTravelViewComponent, TimeTravelViewContext,
};

impl ReviewApp {
    fn render_chrome_with_body<F>(&self, render_body: F) -> String
    where
        F: FnOnce(usize) -> String,
    {
        let mut output = String::new();

        output.push_str(&self.render_header());

        let chrome_height = 2_usize; // header + status bar
        let total_height = self.height as usize;
        let body_height = total_height.saturating_sub(chrome_height);

        output.push_str(&render_body(body_height));
        output.push_str(&self.render_status_bar());

        output
    }

    /// Renders the header bar.
    pub(super) fn render_header(&self) -> String {
        let title = "Frankie - Review Comments";
        let loading_indicator = if self.loading { " [Loading...]" } else { "" };
        format!("{title}{loading_indicator}\n")
    }

    /// Renders the filter bar showing active filter.
    pub(super) fn render_filter_bar(&self) -> String {
        let label = self.filter_state.active_filter.label();
        let count = self.filtered_count();
        let total = self.reviews.len();
        format!("Filter: {label} ({count}/{total})\n")
    }

    /// Renders the status bar with help hints.
    pub(super) fn render_status_bar(&self) -> String {
        if let Some(session) = &self.resume_prompt {
            let timestamp = session.started_at.format("%Y-%m-%d %H:%M UTC");
            return format!("Interrupted session from {timestamp}. Resume? [y/n]\n");
        }

        if let Some(error) = &self.error {
            return format!("Error: {error}\n");
        }

        if let Some(codex_status) = &self.codex_status {
            return self.render_codex_status(codex_status);
        }

        if self.has_reply_draft() {
            return self.render_reply_draft_status();
        }

        let hints = match self.view_mode {
            super::ViewMode::ReviewList => self.review_list_status_hints(),
            super::ViewMode::DiffContext => "[/]:hunks  Esc:back  ?:help  q:quit",
            super::ViewMode::TimeTravel => "h/l:commits  Esc:back  ?:help  q:quit",
        };
        format!("{hints}\n")
    }

    fn render_codex_status(&self, status: &str) -> String {
        let running_suffix = if self.is_codex_running() {
            " (running)"
        } else {
            ""
        };
        format!("Codex: {status}{running_suffix}\n")
    }

    fn render_reply_draft_status(&self) -> String {
        if self.has_reply_draft_ai_preview() {
            "Reply draft: Y:apply  N:discard  text:edit  Enter:ready  Esc:cancel\n".to_owned()
        } else {
            "Reply draft: 1-9:template  E:expand  W:reword  text:edit  Backspace:delete  Enter:ready  Esc:cancel\n"
                .to_owned()
        }
    }

    /// Renders the help overlay if visible.
    pub(super) fn render_help_overlay(&self) -> String {
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
  c          View full-screen context
  t          Time-travel to comment's commit
  a          Start inline reply draft
  x          Run Codex using filtered comments
  ?          Toggle this help
  q          Quit

Diff context:
  [          Previous hunk
  ]          Next hunk
  Esc        Return to review list

Time-travel:
  h          Previous (older) commit
  l          Next (more recent) commit
  Esc        Return to review list

Reply draft:
  1-9        Insert template
  E          AI expand draft
  W          AI reword draft
  Y          Apply AI preview
  N          Discard AI preview
  text keys  Edit draft text
  Backspace  Delete one character
  Enter      Mark draft ready to send
  Esc        Discard draft and return

Press any key to close this help.
";
        help_text.to_owned()
    }

    /// Renders the full-screen diff context view.
    pub(super) fn render_diff_context_view(&self) -> String {
        self.render_chrome_with_body(|body_height| {
            let ctx = DiffContextViewContext {
                hunks: self.diff_context_state.hunks(),
                current_index: self.diff_context_state.current_index(),
                max_height: body_height,
            };

            DiffContextComponent::view(&ctx)
        })
    }

    /// Renders the time-travel navigation view.
    pub(super) fn render_time_travel_view(&self) -> String {
        self.render_chrome_with_body(|body_height| {
            let ctx = TimeTravelViewContext {
                state: self.time_travel_state.as_ref(),
                max_width: self.width as usize,
                max_height: body_height,
            };

            TimeTravelViewComponent::view(&ctx)
        })
    }

    const fn review_list_status_hints(&self) -> &'static str {
        if self.width <= 80 {
            "q:quit  ?:help  j/k:move  f:filter  a:reply  x:codex"
        } else {
            "j/k:move  f:filter  c:context  t:travel  a:reply  x:codex  r:refresh  ?:help  q:quit"
        }
    }
}
