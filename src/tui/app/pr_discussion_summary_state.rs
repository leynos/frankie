//! TUI-local state for the PR-discussion summary view.

use crate::ai::{FileDiscussionSummary, PrDiscussionSummary, SeverityBucket, TuiViewLink};

/// Summary view state used only by the TUI adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrDiscussionSummaryViewState {
    summary: PrDiscussionSummary,
    item_cursor: usize,
    scroll_offset: usize,
}

impl PrDiscussionSummaryViewState {
    /// Builds render/navigation state from a shared summary DTO.
    #[must_use]
    pub const fn new(summary: PrDiscussionSummary) -> Self {
        Self {
            summary,
            item_cursor: 0,
            scroll_offset: 0,
        }
    }

    /// Returns the backing shared summary.
    #[must_use]
    pub const fn summary(&self) -> &PrDiscussionSummary {
        &self.summary
    }

    /// Returns the selected item cursor.
    #[must_use]
    pub const fn item_cursor(&self) -> usize {
        self.item_cursor
    }

    /// Returns the current scroll offset measured in rendered rows.
    #[must_use]
    pub const fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Returns the currently selected link, if any.
    #[must_use]
    pub fn selected_link(&self) -> Option<&TuiViewLink> {
        self.summary
            .item_at(self.item_cursor)
            .map(|item| &item.tui_link)
    }

    /// Moves the cursor up by one item.
    pub fn cursor_up(&mut self, visible_height: usize) {
        self.item_cursor = self.item_cursor.saturating_sub(1);
        self.adjust_scroll(visible_height);
    }

    /// Moves the cursor down by one item.
    pub fn cursor_down(&mut self, visible_height: usize) {
        let max_index = self.summary.item_count().saturating_sub(1);
        self.item_cursor = self.item_cursor.saturating_add(1).min(max_index);
        self.adjust_scroll(visible_height);
    }

    /// Moves the cursor up by one page.
    pub fn page_up(&mut self, visible_height: usize) {
        self.item_cursor = self.item_cursor.saturating_sub(visible_height.max(1));
        self.adjust_scroll(visible_height);
    }

    /// Moves the cursor down by one page.
    pub fn page_down(&mut self, visible_height: usize) {
        let max_index = self.summary.item_count().saturating_sub(1);
        self.item_cursor = self
            .item_cursor
            .saturating_add(visible_height.max(1))
            .min(max_index);
        self.adjust_scroll(visible_height);
    }

    /// Moves the cursor to the first item.
    pub const fn home(&mut self) {
        self.item_cursor = 0;
        self.scroll_offset = 0;
    }

    /// Moves the cursor to the final item.
    pub fn end(&mut self, visible_height: usize) {
        self.item_cursor = self.summary.item_count().saturating_sub(1);
        self.adjust_scroll(visible_height);
    }

    fn adjust_scroll(&mut self, visible_height: usize) {
        let effective_height = visible_height.max(1);
        let Some(selected_row) = self.selected_row_index() else {
            self.scroll_offset = 0;
            return;
        };

        if selected_row < self.scroll_offset {
            self.scroll_offset = selected_row;
            return;
        }

        let visible_end = self
            .scroll_offset
            .saturating_add(effective_height)
            .saturating_sub(1);
        if selected_row > visible_end {
            self.scroll_offset = selected_row.saturating_sub(effective_height.saturating_sub(1));
        }
    }

    fn selected_row_index(&self) -> Option<usize> {
        row_index_for_item(&self.summary.files, self.item_cursor)
    }
}

fn row_index_for_item(files: &[FileDiscussionSummary], target_item_index: usize) -> Option<usize> {
    let mut row_index = 0_usize;
    let mut item_index = 0_usize;

    for file in files {
        row_index = row_index.saturating_add(1);
        for bucket in &file.severities {
            let Some(found_index) = row_index_for_item_in_bucket(
                bucket,
                target_item_index,
                &mut row_index,
                &mut item_index,
            ) else {
                continue;
            };
            return Some(found_index);
        }
    }

    None
}

fn row_index_for_item_in_bucket(
    bucket: &SeverityBucket,
    target_item_index: usize,
    row_index: &mut usize,
    item_index: &mut usize,
) -> Option<usize> {
    *row_index = row_index.saturating_add(1);

    for _ in &bucket.items {
        if *item_index == target_item_index {
            return Some(*row_index);
        }
        *row_index = row_index.saturating_add(1);
        *item_index = item_index.saturating_add(1);
    }

    None
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::PrDiscussionSummaryViewState;
    use crate::ai::{
        DiscussionSeverity, DiscussionSummaryItem, FileDiscussionSummary, PrDiscussionSummary,
        SeverityBucket, TuiViewLink,
    };

    fn sample_summary() -> PrDiscussionSummary {
        PrDiscussionSummary {
            files: vec![FileDiscussionSummary {
                file_path: "src/main.rs".to_owned(),
                severities: vec![SeverityBucket {
                    severity: DiscussionSeverity::High,
                    items: vec![
                        DiscussionSummaryItem {
                            root_comment_id: 1_u64.into(),
                            related_comment_ids: vec![1_u64.into()],
                            headline: "First".to_owned(),
                            rationale: "One".to_owned(),
                            severity: DiscussionSeverity::High,
                            tui_link: TuiViewLink::comment_detail(1_u64.into()),
                        },
                        DiscussionSummaryItem {
                            root_comment_id: 2_u64.into(),
                            related_comment_ids: vec![2_u64.into()],
                            headline: "Second".to_owned(),
                            rationale: "Two".to_owned(),
                            severity: DiscussionSeverity::High,
                            tui_link: TuiViewLink::comment_detail(2_u64.into()),
                        },
                    ],
                }],
            }],
        }
    }

    #[rstest]
    fn state_builds_headings_and_item_rows() {
        let state = PrDiscussionSummaryViewState::new(sample_summary());

        assert_eq!(state.item_cursor(), 0);
        assert_eq!(
            state.selected_link().map(ToString::to_string),
            Some("frankie://review-comment/1?view=detail".to_owned())
        );
    }

    #[rstest]
    fn state_navigation_keeps_selected_row_visible() {
        let mut state = PrDiscussionSummaryViewState::new(sample_summary());

        state.cursor_down(1);

        assert_eq!(state.item_cursor(), 1);
        assert_eq!(state.scroll_offset(), 3);
    }
}
