//! Behavioural tests for the TUI PR-discussion summary flow.

use std::sync::Arc;

use bubbletea_rs::Cmd;
use bubbletea_rs::Model;
use frankie::ai::pr_discussion_summary::test_support::StubPrDiscussionSummaryService;
use frankie::ai::{
    DiscussionSeverity, DiscussionSummaryItem, FileDiscussionSummary, PrDiscussionSummary,
    PrDiscussionSummaryService, SeverityBucket, TuiViewLink,
};
use frankie::github::IntakeError;
use frankie::github::models::ReviewComment;
use frankie::github::models::test_support::minimal_review;
use frankie::tui::app::ReviewApp;
use frankie::tui::messages::AppMsg;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

type StepResult = Result<(), Box<dyn std::error::Error>>;

#[derive(ScenarioState, Default)]
struct TuiPrDiscussionSummaryState {
    app: Slot<ReviewApp>,
    pending_cmd: Slot<Option<Cmd>>,
    rendered_view: Slot<String>,
}

#[fixture]
fn tui_pr_discussion_summary_state() -> TuiPrDiscussionSummaryState {
    TuiPrDiscussionSummaryState::default()
}

fn sample_comments() -> Vec<ReviewComment> {
    vec![
        ReviewComment {
            file_path: Some("src/main.rs".to_owned()),
            line_number: Some(12),
            diff_hunk: Some("@@ -1 +1 @@\n+fn first() {}".to_owned()),
            ..minimal_review(1, "Handle the panic path", "alice")
        },
        ReviewComment {
            file_path: Some("src/lib.rs".to_owned()),
            line_number: Some(27),
            diff_hunk: Some("@@ -2 +2 @@\n+fn second() {}".to_owned()),
            ..minimal_review(2, "Consider extracting this helper", "bob")
        },
    ]
}

fn sample_summary() -> PrDiscussionSummary {
    PrDiscussionSummary {
        files: vec![FileDiscussionSummary {
            file_path: "src/main.rs".to_owned(),
            severities: vec![SeverityBucket {
                severity: DiscussionSeverity::High,
                items: vec![DiscussionSummaryItem {
                    root_comment_id: 1_u64.into(),
                    related_comment_ids: vec![1_u64.into()],
                    headline: "Handle panic path".to_owned(),
                    rationale: "Review thread flagged an unchecked failure".to_owned(),
                    severity: DiscussionSeverity::High,
                    tui_link: TuiViewLink::comment_detail(1_u64.into()),
                }],
            }],
        }],
    }
}

fn build_app(service: Arc<dyn PrDiscussionSummaryService>) -> ReviewApp {
    ReviewApp::new(sample_comments()).with_pr_discussion_summary_service(service)
}

#[given("a review TUI with PR discussion summary succeeding")]
fn given_tui_with_summary_success(tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState) {
    let app = build_app(Arc::new(StubPrDiscussionSummaryService::success(
        sample_summary(),
    )));
    tui_pr_discussion_summary_state.app.set(app);
    tui_pr_discussion_summary_state.pending_cmd.set(None);
}

#[given("a review TUI with PR discussion summary failing with {text}")]
fn given_tui_with_summary_failure(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
    text: String,
) {
    let app = build_app(Arc::new(StubPrDiscussionSummaryService::failure(
        IntakeError::Network {
            message: text.trim_matches('"').to_owned(),
        },
    )));
    tui_pr_discussion_summary_state.app.set(app);
    tui_pr_discussion_summary_state.pending_cmd.set(None);
}

#[when("the user requests a PR discussion summary")]
fn when_user_requests_summary(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
) -> StepResult {
    let maybe_cmd = tui_pr_discussion_summary_state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::GeneratePrDiscussionSummary))
        .ok_or("app should be initialised before requesting summary")?;
    tui_pr_discussion_summary_state.pending_cmd.set(maybe_cmd);
    Ok(())
}

#[when("the PR discussion summary command is executed")]
fn when_summary_command_executes(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
) -> StepResult {
    let maybe_cmd = tui_pr_discussion_summary_state
        .pending_cmd
        .with_mut(Option::take)
        .ok_or("pending command slot should be initialised")?;
    let cmd = maybe_cmd.ok_or("expected pending summary command")?;
    let runtime = tokio::runtime::Runtime::new()?;
    let maybe_msg = runtime.block_on(cmd);
    let Some(message) = maybe_msg else {
        return Err("summary command should return a message".into());
    };
    let app_msg = message
        .downcast::<AppMsg>()
        .map_err(|_| "summary command returned a non-AppMsg value")?;

    tui_pr_discussion_summary_state
        .app
        .with_mut(|app| {
            app.handle_message(&app_msg);
        })
        .ok_or("app should be initialised before applying summary result")?;

    Ok(())
}

#[when("the summary view is rendered")]
fn when_summary_view_is_rendered(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
) -> StepResult {
    let view = tui_pr_discussion_summary_state
        .app
        .with_ref(ReviewApp::view)
        .ok_or("app should be initialised before rendering view")?;
    tui_pr_discussion_summary_state.rendered_view.set(view);
    Ok(())
}

#[when("the user opens the selected summary link")]
fn when_user_opens_selected_summary_link(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
) -> StepResult {
    let view = tui_pr_discussion_summary_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::OpenSelectedPrDiscussionSummaryLink);
            app.view()
        })
        .ok_or("app should be initialised before opening summary link")?;
    if !view.contains("[alice] src/main.rs:12") {
        return Err(format!(
            "expected opening the summary link to show the linked comment detail, got:\n{view}"
        )
        .into());
    }
    tui_pr_discussion_summary_state.rendered_view.set(view);
    Ok(())
}

fn assert_view_contains(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
    text: String,
    should_contain: bool,
) -> StepResult {
    let owned_text = text;
    let expected = owned_text.trim_matches('"');
    let view = tui_pr_discussion_summary_state
        .rendered_view
        .with_ref(Clone::clone)
        .ok_or("rendered view should be captured before assertions")?;

    if view.contains(expected) != should_contain {
        let verb = if should_contain {
            "contain"
        } else {
            "not contain"
        };
        return Err(format!("expected view to {verb} '{expected}', got:\n{view}").into());
    }

    Ok(())
}

#[then("the summary view contains {text}")]
fn then_summary_view_contains(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
    text: String,
) -> StepResult {
    assert_view_contains(tui_pr_discussion_summary_state, text, true)
}

#[then("the summary view does not contain {text}")]
fn then_summary_view_does_not_contain(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
    text: String,
) -> StepResult {
    assert_view_contains(tui_pr_discussion_summary_state, text, false)
}

#[then("the selected comment id is 1")]
fn then_selected_comment_id_is_one(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
) -> StepResult {
    let selected_id = tui_pr_discussion_summary_state
        .app
        .with_ref(ReviewApp::current_selected_id)
        .ok_or("app should be initialised before reading selection")?;

    let expected_id = 1_u64;
    if selected_id != Some(expected_id) {
        return Err(format!("expected selected id {expected_id}, got {selected_id:?}").into());
    }

    Ok(())
}

#[then("the TUI summary error contains {text}")]
fn then_tui_summary_error_contains(
    tui_pr_discussion_summary_state: &TuiPrDiscussionSummaryState,
    text: String,
) -> StepResult {
    let expected = text.trim_matches('"');
    let error_text = tui_pr_discussion_summary_state
        .app
        .with_ref(|app| app.error_message().map(ToOwned::to_owned))
        .ok_or("app should be initialised before checking the error")?
        .ok_or("expected an error to be present")?;

    if !error_text.contains(expected) {
        return Err(format!("expected error to contain '{expected}', got '{error_text}'").into());
    }

    Ok(())
}

#[scenario(path = "tests/features/tui_pr_discussion_summary.feature", index = 0)]
fn tui_summary_generation_and_link_navigation(
    tui_pr_discussion_summary_state: TuiPrDiscussionSummaryState,
) {
    let _ = tui_pr_discussion_summary_state;
}

#[scenario(path = "tests/features/tui_pr_discussion_summary.feature", index = 1)]
fn tui_summary_failure_surfaces_error(
    tui_pr_discussion_summary_state: TuiPrDiscussionSummaryState,
) {
    let _ = tui_pr_discussion_summary_state;
}
