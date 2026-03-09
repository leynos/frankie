//! Full-screen PR-discussion summary view for the review TUI.

use crate::tui::app::{PrDiscussionSummaryRow, PrDiscussionSummaryViewState};
use crate::tui::components::text_truncate::truncate_to_display_width_with_ellipsis;

/// Context for rendering the PR-discussion summary view.
#[derive(Debug, Clone)]
pub(crate) struct PrDiscussionSummaryViewContext<'a> {
    /// Summary state to render, if available.
    pub state: Option<&'a PrDiscussionSummaryViewState>,
    /// Maximum visible width in display columns.
    pub max_width: usize,
    /// Maximum visible height in rows.
    pub max_height: usize,
}

/// Stateless component rendering the PR-discussion summary view.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct PrDiscussionSummaryComponent;

impl PrDiscussionSummaryComponent {
    /// Renders the PR-discussion summary view.
    #[must_use]
    pub fn view(ctx: &PrDiscussionSummaryViewContext<'_>) -> String {
        let Some(state) = ctx.state else {
            return "(No PR discussion summary available)\n".to_owned();
        };

        if state.rows().is_empty() {
            return "(PR discussion summary is empty)\n".to_owned();
        }

        let start = state.scroll_offset();
        let end = (start + ctx.max_height.max(1)).min(state.rows().len());
        let mut output = String::new();

        for row_index in start..end {
            let Some(line) = state
                .rows()
                .get(row_index)
                .and_then(|row| render_row(row, state))
            else {
                continue;
            };
            output.push_str(&truncate_to_display_width_with_ellipsis(
                line.as_str(),
                ctx.max_width,
            ));
            output.push('\n');
        }

        output
    }
}

fn render_row(
    row: &PrDiscussionSummaryRow,
    state: &PrDiscussionSummaryViewState,
) -> Option<String> {
    match row {
        PrDiscussionSummaryRow::FileHeading(file_path) => Some(format!("File: {file_path}")),
        PrDiscussionSummaryRow::SeverityHeading(severity) => {
            Some(format!("  Severity: {severity}"))
        }
        PrDiscussionSummaryRow::Item { item_index } => {
            state.summary().item_at(*item_index).map(|item| {
                format!(
                    "{} {} -- {} [{}]",
                    selected_prefix(state, *item_index),
                    item.headline,
                    item.rationale,
                    item.tui_link
                )
            })
        }
    }
}

const fn selected_prefix(state: &PrDiscussionSummaryViewState, item_index: usize) -> &'static str {
    if state.item_cursor() == item_index {
        ">"
    } else {
        " "
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{PrDiscussionSummaryComponent, PrDiscussionSummaryViewContext};
    use crate::ai::{
        DiscussionSeverity, DiscussionSummaryItem, FileDiscussionSummary, PrDiscussionSummary,
        SeverityBucket, TuiViewLink,
    };
    use crate::tui::app::PrDiscussionSummaryViewState;

    fn sample_state() -> PrDiscussionSummaryViewState {
        PrDiscussionSummaryViewState::new(PrDiscussionSummary {
            files: vec![FileDiscussionSummary {
                file_path: "src/main.rs".to_owned(),
                severities: vec![SeverityBucket {
                    severity: DiscussionSeverity::High,
                    items: vec![DiscussionSummaryItem {
                        root_comment_id: 1_u64.into(),
                        related_comment_ids: vec![1_u64.into()],
                        headline: "Handle panic path".to_owned(),
                        rationale: "Review thread flagged unwrap".to_owned(),
                        severity: DiscussionSeverity::High,
                        tui_link: TuiViewLink::comment_detail(1_u64.into()),
                    }],
                }],
            }],
        })
    }

    #[rstest]
    fn view_renders_grouped_summary_rows() {
        let state = sample_state();
        let output = PrDiscussionSummaryComponent::view(&PrDiscussionSummaryViewContext {
            state: Some(&state),
            max_width: 120,
            max_height: 10,
        });

        assert!(output.contains("File: src/main.rs"));
        assert!(output.contains("Severity: high"));
        assert!(output.contains("> Handle panic path"));
        assert!(output.contains("frankie://review-comment/1?view=detail"));
    }

    #[rstest]
    fn view_truncates_long_rows_to_available_width() {
        let state = sample_state();
        let output = PrDiscussionSummaryComponent::view(&PrDiscussionSummaryViewContext {
            state: Some(&state),
            max_width: 20,
            max_height: 10,
        });

        for line in output.lines() {
            assert!(line.chars().count() <= 20);
        }
    }
}
