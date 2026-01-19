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
        if let Some(error) = &self.error {
            return format!("Error: {error}\n");
        }

        let hints = match self.view_mode {
            super::ViewMode::ReviewList => {
                "j/k:navigate  f:filter  c:context  t:time-travel  r:refresh  ?:help  q:quit"
            }
            super::ViewMode::DiffContext => "[/]:hunks  Esc:back  ?:help  q:quit",
            super::ViewMode::TimeTravel => "h/l:commits  Esc:back  q:quit",
        };
        format!("{hints}\n")
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

Press any key to close this help.
";
        help_text.to_owned()
    }

    /// Renders the full-screen diff context view.
    pub(super) fn render_diff_context_view(&self) -> String {
        let mut output = String::new();

        output.push_str(&self.render_header());

        let chrome_height = 2_usize; // header + status bar
        let total_height = self.height as usize;
        let body_height = total_height.saturating_sub(chrome_height);

        let ctx = DiffContextViewContext {
            hunks: self.diff_context_state.hunks(),
            current_index: self.diff_context_state.current_index(),
            max_height: body_height,
        };

        output.push_str(&DiffContextComponent::view(&ctx));
        output.push_str(&self.render_status_bar());

        output
    }

    /// Renders the time-travel navigation view.
    pub(super) fn render_time_travel_view(&self) -> String {
        let mut output = String::new();

        output.push_str(&self.render_header());

        let chrome_height = 2_usize; // header + status bar
        let total_height = self.height as usize;
        let body_height = total_height.saturating_sub(chrome_height);

        let ctx = TimeTravelViewContext {
            state: self.time_travel_state.as_ref(),
            max_width: self.width as usize,
            max_height: body_height,
        };

        output.push_str(&TimeTravelViewComponent::view(&ctx));
        output.push_str(&self.render_status_bar());

        output
    }
}
