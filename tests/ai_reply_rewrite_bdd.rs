//! Behavioural tests for AI-assisted reply draft rewrite flows.

use std::sync::Arc;

use bubbletea_rs::Cmd;
use bubbletea_rs::Model;
use frankie::ai::comment_rewrite::test_support::StubCommentRewriteService;
use frankie::ai::{CommentRewriteMode, CommentRewriteService};
use frankie::github::IntakeError;
use frankie::github::models::ReviewComment;
use frankie::github::models::test_support::minimal_review;
use frankie::tui::app::ReviewApp;
use frankie::tui::messages::AppMsg;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

#[derive(ScenarioState, Default)]
struct AiRewriteState {
    app: Slot<ReviewApp>,
    rendered_view: Slot<String>,
    pending_cmd: Slot<Option<Cmd>>,
}

#[fixture]
fn ai_rewrite_state() -> AiRewriteState {
    AiRewriteState::default()
}

type StepResult = Result<(), Box<dyn std::error::Error>>;

fn sample_comment() -> ReviewComment {
    ReviewComment {
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(12),
        body: Some("Please address this".to_owned()),
        ..minimal_review(1, "Please address this", "alice")
    }
}

fn build_app_with_service(service: Arc<dyn CommentRewriteService>) -> ReviewApp {
    ReviewApp::new(vec![sample_comment()]).with_comment_rewrite_service(service)
}

fn parse_mode(mode: &str) -> Result<CommentRewriteMode, Box<dyn std::error::Error>> {
    mode.trim_matches('"')
        .parse::<CommentRewriteMode>()
        .map_err(|error| error.to_string().into())
}

#[given("a review TUI with AI rewrite succeeding to {text}")]
fn given_tui_with_success(ai_rewrite_state: &AiRewriteState, text: String) {
    let app = build_app_with_service(Arc::new(StubCommentRewriteService::success(
        text.trim_matches('"').to_owned(),
    )));
    ai_rewrite_state.app.set(app);
    ai_rewrite_state.pending_cmd.set(None);
}

#[given("a review TUI with AI rewrite failing with {text}")]
fn given_tui_with_failure(ai_rewrite_state: &AiRewriteState, text: String) {
    let app = build_app_with_service(Arc::new(StubCommentRewriteService::failure(
        IntakeError::Network {
            message: text.trim_matches('"').to_owned(),
        },
    )));
    ai_rewrite_state.app.set(app);
    ai_rewrite_state.pending_cmd.set(None);
}

#[when("the user starts reply drafting and types {text}")]
fn when_user_starts_and_types(ai_rewrite_state: &AiRewriteState, text: String) -> StepResult {
    let input = text.trim_matches('"');
    ai_rewrite_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::StartReplyDraft);
            for character in input.chars() {
                app.handle_message(&AppMsg::ReplyDraftInsertChar(character));
            }
        })
        .ok_or("app should be initialised before typing")?;
    Ok(())
}

#[when("the user requests AI {mode} rewrite")]
fn when_user_requests_rewrite(ai_rewrite_state: &AiRewriteState, mode: String) -> StepResult {
    let rewrite_mode = parse_mode(&mode)?;
    let maybe_cmd = ai_rewrite_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite { mode: rewrite_mode })
        })
        .ok_or("app should be initialised before requesting rewrite")?;

    ai_rewrite_state.pending_cmd.set(maybe_cmd);
    Ok(())
}

#[when("the AI rewrite command is executed")]
fn when_rewrite_command_executes(ai_rewrite_state: &AiRewriteState) -> StepResult {
    let maybe_cmd = ai_rewrite_state
        .pending_cmd
        .with_mut(Option::take)
        .ok_or("pending command slot should be initialised")?;
    let cmd = maybe_cmd.ok_or("expected pending AI rewrite command")?;
    let runtime = tokio::runtime::Runtime::new()?;
    let maybe_msg = runtime.block_on(cmd);

    let Some(message) = maybe_msg else {
        return Err("AI rewrite command should return a message".into());
    };

    let app_msg = message
        .downcast::<AppMsg>()
        .map_err(|_| "AI rewrite command returned a non-AppMsg value")?;

    ai_rewrite_state
        .app
        .with_mut(|app| {
            app.handle_message(&app_msg);
        })
        .ok_or("app should be initialised before applying command result")?;

    Ok(())
}

#[when("the user applies the AI preview")]
fn when_user_applies_preview(ai_rewrite_state: &AiRewriteState) -> StepResult {
    ai_rewrite_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::ReplyDraftAiApply);
        })
        .ok_or("app should be initialised before applying preview")?;
    Ok(())
}

#[when("the user discards the AI preview")]
fn when_user_discards_preview(ai_rewrite_state: &AiRewriteState) -> StepResult {
    ai_rewrite_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::ReplyDraftAiDiscard);
        })
        .ok_or("app should be initialised before discarding preview")?;
    Ok(())
}

#[when("the view is rendered")]
fn when_view_is_rendered(ai_rewrite_state: &AiRewriteState) -> StepResult {
    let view = ai_rewrite_state
        .app
        .with_ref(ReviewApp::view)
        .ok_or("app should be initialised before rendering view")?;
    ai_rewrite_state.rendered_view.set(view);
    Ok(())
}

/// Asserts whether the rendered view contains the provided text fragment.
fn assert_view_content(
    ai_rewrite_state: &AiRewriteState,
    text: String,
    should_contain: bool,
) -> StepResult {
    let owned_text = text;
    let expected = owned_text.trim_matches('"');
    let view = ai_rewrite_state
        .rendered_view
        .with_ref(Clone::clone)
        .ok_or("view should be rendered before assertions")?;
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

#[then("the view contains {text}")]
fn then_view_contains(ai_rewrite_state: &AiRewriteState, text: String) -> StepResult {
    assert_view_content(ai_rewrite_state, text, true)
}

#[then("the view does not contain {text}")]
fn then_view_does_not_contain(ai_rewrite_state: &AiRewriteState, text: String) -> StepResult {
    assert_view_content(ai_rewrite_state, text, false)
}

#[then("the TUI error contains {text}")]
fn then_error_contains(ai_rewrite_state: &AiRewriteState, text: String) -> StepResult {
    let expected = text.trim_matches('"');
    let error_text = ai_rewrite_state
        .app
        .with_ref(|app| app.error_message().map(ToOwned::to_owned))
        .ok_or("app should be initialised before checking errors")?
        .ok_or("expected a TUI error to be present")?;

    if !error_text.contains(expected) {
        return Err(format!("expected error to contain '{expected}', got '{error_text}'").into());
    }

    Ok(())
}

#[scenario(path = "tests/features/ai_reply_rewrite.feature", index = 0)]
fn ai_expand_generates_preview_and_applies(ai_rewrite_state: AiRewriteState) {
    let _ = ai_rewrite_state;
}

#[scenario(path = "tests/features/ai_reply_rewrite.feature", index = 1)]
fn ai_rewrite_failure_falls_back(ai_rewrite_state: AiRewriteState) {
    let _ = ai_rewrite_state;
}

#[scenario(path = "tests/features/ai_reply_rewrite.feature", index = 2)]
fn ai_preview_can_be_discarded(ai_rewrite_state: AiRewriteState) {
    let _ = ai_rewrite_state;
}
