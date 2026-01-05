//! Rendering logic for the review TUI application.
//!
//! This module contains the view rendering methods that produce string output
//! for display in the terminal. These are pure query methods that read state
//! without modification.

use super::ReviewApp;

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

        let hints = "j/k:navigate  f:filter  r:refresh  ?:help  q:quit";
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
  ?          Toggle this help
  q          Quit

Press any key to close this help.
";
        help_text.to_owned()
    }
}
