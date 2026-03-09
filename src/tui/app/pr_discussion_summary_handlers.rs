//! PR-discussion summary handlers for the review TUI.

use std::any::Any;
use std::sync::Arc;

use bubbletea_rs::Cmd;

use crate::ai::{PrDiscussionSummaryRequest, PrDiscussionSummaryService};
use crate::tui::messages::AppMsg;

use super::{PrDiscussionSummaryViewState, ReviewApp, ViewMode};

#[derive(Debug)]
struct SummaryTaskParams {
    request_id: u64,
    service: Arc<dyn PrDiscussionSummaryService>,
    request: PrDiscussionSummaryRequest,
}

impl ReviewApp {
    pub(super) fn handle_pr_discussion_summary_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::GeneratePrDiscussionSummary => self.generate_pr_discussion_summary(),
            AppMsg::PrDiscussionSummaryReady {
                request_id,
                summary,
            } => {
                self.handle_pr_discussion_summary_ready(*request_id, summary);
                None
            }
            AppMsg::PrDiscussionSummaryFailed {
                request_id,
                message,
            } => {
                self.handle_pr_discussion_summary_failed(*request_id, message);
                None
            }
            AppMsg::OpenSelectedPrDiscussionSummaryLink => {
                self.open_selected_pr_discussion_summary_link();
                None
            }
            AppMsg::HidePrDiscussionSummary => {
                self.view_mode = ViewMode::ReviewList;
                self.error = None;
                None
            }
            _ => None,
        }
    }

    pub(super) fn handle_pr_discussion_summary_navigation(&mut self, msg: &AppMsg) -> Option<Cmd> {
        let visible_height = self.summary_view_height();
        let state = self.pr_discussion_summary.as_mut()?;

        match msg {
            AppMsg::CursorUp => state.cursor_up(visible_height),
            AppMsg::CursorDown => state.cursor_down(visible_height),
            AppMsg::PageUp => state.page_up(visible_height),
            AppMsg::PageDown => state.page_down(visible_height),
            AppMsg::Home => state.home(),
            AppMsg::End => state.end(visible_height),
            _ => return None,
        }

        None
    }

    fn generate_pr_discussion_summary(&mut self) -> Option<Cmd> {
        if self.reviews.is_empty() {
            self.error =
                Some("PR discussion summary requires at least one review comment".to_owned());
            return None;
        }

        let request_id = self.next_pr_discussion_summary_request_id;
        self.next_pr_discussion_summary_request_id =
            self.next_pr_discussion_summary_request_id.saturating_add(1);
        self.in_flight_pr_discussion_summary_request_id = Some(request_id);
        self.error = None;

        let pr_number =
            crate::tui::get_refresh_locator().map_or(0, |locator| locator.number().get());
        let request = PrDiscussionSummaryRequest::new(pr_number, None, self.reviews.clone())
            .with_verification_results(self.verification.results.clone());

        Some(spawn_pr_discussion_summary(SummaryTaskParams {
            request_id,
            service: Arc::clone(&self.pr_discussion_summary_service),
            request,
        }))
    }

    fn handle_pr_discussion_summary_ready(
        &mut self,
        request_id: u64,
        summary: &crate::ai::PrDiscussionSummary,
    ) {
        if self.in_flight_pr_discussion_summary_request_id != Some(request_id) {
            return;
        }

        self.in_flight_pr_discussion_summary_request_id = None;
        self.pr_discussion_summary = Some(PrDiscussionSummaryViewState::new(summary.clone()));
        self.view_mode = ViewMode::PrDiscussionSummary;
        self.error = None;
    }

    fn handle_pr_discussion_summary_failed(&mut self, request_id: u64, message: &str) {
        if self.in_flight_pr_discussion_summary_request_id != Some(request_id) {
            return;
        }

        self.in_flight_pr_discussion_summary_request_id = None;
        self.error = Some(message.to_owned());
    }

    fn open_selected_pr_discussion_summary_link(&mut self) {
        let Some(state) = self.pr_discussion_summary.as_ref() else {
            self.error = Some("No PR discussion summary is open.".to_owned());
            return;
        };
        let Some(link) = state.selected_link().cloned() else {
            self.error = Some("PR discussion summary has no selectable item.".to_owned());
            return;
        };

        self.handle_clear_filter();
        if !self.select_by_id(link.comment_id.as_u64()) {
            self.error = Some(format!(
                "Could not find review comment {} referenced by the summary link",
                link.comment_id.as_u64()
            ));
            return;
        }

        self.view_mode = ViewMode::ReviewList;
        self.error = None;
    }

    fn summary_view_height(&self) -> usize {
        (self.height as usize).saturating_sub(2).max(1)
    }
}

fn spawn_pr_discussion_summary(params: SummaryTaskParams) -> Cmd {
    Box::pin(async move {
        match tokio::task::spawn_blocking(move || params.service.summarize(&params.request)).await {
            Ok(Ok(summary)) => Some(Box::new(AppMsg::PrDiscussionSummaryReady {
                request_id: params.request_id,
                summary,
            }) as Box<dyn Any + Send>),
            Ok(Err(error)) => Some(Box::new(AppMsg::PrDiscussionSummaryFailed {
                request_id: params.request_id,
                message: error.to_string(),
            }) as Box<dyn Any + Send>),
            Err(error) => Some(Box::new(AppMsg::PrDiscussionSummaryFailed {
                request_id: params.request_id,
                message: format!("Summary task join error: {error}"),
            }) as Box<dyn Any + Send>),
        }
    })
}
