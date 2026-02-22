//! Behavioural tests for template-based inline reply drafting.

use bubbletea_rs::Model;
use bubbletea_rs::event::KeyMsg;
use crossterm::event::{KeyCode, KeyModifiers};
use frankie::github::models::ReviewComment;
use frankie::github::models::test_support::minimal_review;
use frankie::tui::app::ReviewApp;
use frankie::tui::{ReplyDraftConfig, ReplyDraftMaxLength};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

#[derive(ScenarioState, Default)]
struct ReplyDraftScenarioState {
    app: Slot<ReviewApp>,
    rendered_view: Slot<String>,
}

#[fixture]
fn reply_state() -> ReplyDraftScenarioState {
    ReplyDraftScenarioState::default()
}

type StepResult = Result<(), Box<dyn std::error::Error>>;

fn build_app_with_comments(
    max_length: usize,
    template: &str,
    comments: Vec<ReviewComment>,
) -> ReviewApp {
    ReviewApp::new(comments).with_reply_draft_config(ReplyDraftConfig::new(
        ReplyDraftMaxLength::new(max_length),
        vec![template.to_owned()],
    ))
}

fn sample_comment() -> ReviewComment {
    ReviewComment {
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(12),
        body: Some("Please address this".to_owned()),
        ..minimal_review(1, "Please address this", "alice")
    }
}

fn parse_key(key: &str) -> Result<KeyCode, Box<dyn std::error::Error>> {
    let normalized = key.trim_matches('"');
    let lower = normalized.to_ascii_lowercase();

    let key_code = match lower.as_str() {
        "enter" => KeyCode::Enter,
        "backspace" => KeyCode::Backspace,
        "esc" | "escape" => KeyCode::Esc,
        _ => {
            let mut chars = normalized.chars();
            let Some(character) = chars.next() else {
                return Err("key token must not be empty".into());
            };
            if chars.next().is_some() {
                return Err(format!("unsupported key token: {normalized}").into());
            }
            KeyCode::Char(character)
        }
    };

    Ok(key_code)
}

fn send_key(state: &ReplyDraftScenarioState, key_code: KeyCode) -> StepResult {
    let key_message = KeyMsg {
        key: key_code,
        modifiers: KeyModifiers::empty(),
    };

    state
        .app
        .with_mut(|app| {
            app.update(Box::new(key_message));
        })
        .ok_or("app should be initialised before sending input")?;

    Ok(())
}

fn view_from_state(state: &ReplyDraftScenarioState) -> Result<String, Box<dyn std::error::Error>> {
    state
        .rendered_view
        .with_ref(Clone::clone)
        .ok_or_else(|| "view should be rendered before assertions".into())
}

#[given("a review TUI with one comment, max length {max_length:usize}, and template {template}")]
fn given_tui_with_comment(
    reply_state: &ReplyDraftScenarioState,
    max_length: usize,
    template: String,
) {
    let app = build_app_with_comments(
        max_length,
        template.trim_matches('"'),
        vec![sample_comment()],
    );
    reply_state.app.set(app);
}

#[given("a review TUI with no comments and max length {max_length:usize}")]
fn given_tui_without_comments(reply_state: &ReplyDraftScenarioState, max_length: usize) {
    let app = build_app_with_comments(max_length, "Template", Vec::new());
    reply_state.app.set(app);
}

#[when("the user presses {key}")]
fn when_user_presses_key(reply_state: &ReplyDraftScenarioState, key: String) -> StepResult {
    let key_code = parse_key(&key)?;
    send_key(reply_state, key_code)
}

#[when("the view is rendered")]
fn when_view_is_rendered(reply_state: &ReplyDraftScenarioState) -> StepResult {
    let view = reply_state
        .app
        .with_ref(ReviewApp::view)
        .ok_or("app should be initialised before rendering view")?;
    reply_state.rendered_view.set(view);
    Ok(())
}

#[then("the view contains {text}")]
fn then_view_contains(reply_state: &ReplyDraftScenarioState, text: String) -> StepResult {
    let expected = text.trim_matches('"');
    let view = view_from_state(reply_state)?;
    if !view.contains(expected) {
        return Err(format!("expected view to contain '{expected}', got:\n{view}").into());
    }
    Ok(())
}

#[then("the TUI error contains {text}")]
fn then_error_contains(reply_state: &ReplyDraftScenarioState, text: String) -> StepResult {
    let expected = text.trim_matches('"');
    let error_text = reply_state
        .app
        .with_ref(|app| app.error_message().map(ToOwned::to_owned))
        .ok_or("app should be initialised before checking errors")?
        .ok_or("expected a TUI error to be present")?;

    if !error_text.contains(expected) {
        return Err(format!("expected error to contain '{expected}', got '{error_text}'").into());
    }

    Ok(())
}

#[then("no TUI error is shown")]
fn then_no_error(reply_state: &ReplyDraftScenarioState) -> StepResult {
    let has_error = reply_state
        .app
        .with_ref(|app| app.error_message().is_some())
        .ok_or("app should be initialised before checking errors")?;
    if has_error {
        return Err("expected no TUI error".into());
    }

    Ok(())
}

#[scenario(path = "tests/features/template_reply_drafting.feature", index = 0)]
fn start_reply_and_insert_template(reply_state: ReplyDraftScenarioState) {
    let _ = reply_state;
}

#[scenario(path = "tests/features/template_reply_drafting.feature", index = 1)]
fn reply_template_is_editable_before_send(reply_state: ReplyDraftScenarioState) {
    let _ = reply_state;
}

#[scenario(path = "tests/features/template_reply_drafting.feature", index = 2)]
fn insertion_respects_configured_max_length(reply_state: ReplyDraftScenarioState) {
    let _ = reply_state;
}

#[scenario(path = "tests/features/template_reply_drafting.feature", index = 3)]
fn selecting_unconfigured_slot_reports_error(reply_state: ReplyDraftScenarioState) {
    let _ = reply_state;
}

#[scenario(path = "tests/features/template_reply_drafting.feature", index = 4)]
fn drafting_requires_selected_comment(reply_state: ReplyDraftScenarioState) {
    let _ = reply_state;
}
