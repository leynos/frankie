//! Tests for reply-drafting handlers.

use std::sync::Arc;

use bubbletea_rs::Cmd;
use rstest::{fixture, rstest};

use crate::ai::CommentRewriteMode;
use crate::ai::comment_rewrite::test_support::StubCommentRewriteService;
use crate::github::IntakeError;
use crate::github::models::ReviewComment;
use crate::tui::messages::AppMsg;
use crate::tui::{ReplyDraftConfig, ReplyDraftMaxLength};

use super::ReviewApp;

#[fixture]
fn sample_reviews() -> Vec<ReviewComment> {
    vec![ReviewComment {
        id: 1,
        author: Some("alice".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(12),
        body: Some("Please split this helper".to_owned()),
        ..ReviewComment::default()
    }]
}

#[fixture]
fn sample_reviews_pair() -> Vec<ReviewComment> {
    vec![
        ReviewComment {
            id: 1,
            author: Some("alice".to_owned()),
            file_path: Some("src/main.rs".to_owned()),
            line_number: Some(12),
            body: Some("Please split this helper".to_owned()),
            ..ReviewComment::default()
        },
        ReviewComment {
            id: 2,
            author: Some("bob".to_owned()),
            file_path: Some("src/lib.rs".to_owned()),
            line_number: Some(42),
            body: Some("Consider extracting this into a utility".to_owned()),
            ..ReviewComment::default()
        },
    ]
}

/// Helper to create an app with a draft and inserted template.
fn app_with_inserted_template(
    reviews: Vec<ReviewComment>,
    max_length: usize,
    template: &str,
) -> ReviewApp {
    let mut app = ReviewApp::new(reviews).with_reply_draft_config(ReplyDraftConfig::new(
        ReplyDraftMaxLength::new(max_length),
        vec![template.to_owned()],
    ));
    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertTemplate { template_index: 0 });
    app
}

/// Helper to assert the readiness state of the active reply draft.
fn assert_draft_readiness(app: &ReviewApp, expected_ready: bool) {
    let maybe_draft = app.reply_draft.as_ref();
    assert!(maybe_draft.is_some(), "draft should exist");
    if let Some(draft) = maybe_draft {
        assert_eq!(
            draft.is_ready_to_send(),
            expected_ready,
            "draft readiness should be {expected_ready}"
        );
    }
}

async fn resolve_cmd_to_app_msg(cmd: Cmd) -> Option<AppMsg> {
    let maybe_boxed = cmd.await?;
    maybe_boxed
        .downcast::<AppMsg>()
        .ok()
        .map(|message| *message)
}

async fn trigger_ai_rewrite(
    app: &mut ReviewApp,
    mode: CommentRewriteMode,
) -> Result<AppMsg, &'static str> {
    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite { mode });
    let cmd = maybe_cmd.ok_or("AI rewrite should return a command")?;
    let maybe_msg = resolve_cmd_to_app_msg(cmd).await;
    maybe_msg.ok_or("AI rewrite command should emit a message")
}

#[rstest]
fn start_reply_draft_requires_selected_comment() {
    let mut app = ReviewApp::empty();

    app.handle_message(&AppMsg::StartReplyDraft);

    let error = app.error_message().unwrap_or_default();
    assert!(error.contains("selected comment"));
}

#[rstest]
fn start_reply_draft_creates_empty_state(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    app.handle_message(&AppMsg::StartReplyDraft);

    let maybe_draft = app.reply_draft.as_ref();
    assert!(maybe_draft.is_some(), "draft should be created");
    if let Some(draft) = maybe_draft {
        assert_eq!(draft.comment_id(), 1);
        assert_eq!(draft.text(), "");
        assert!(!draft.is_ready_to_send());
    }
}

#[rstest]
fn insert_template_renders_comment_fields(sample_reviews: Vec<ReviewComment>) {
    let app = app_with_inserted_template(
        sample_reviews,
        200,
        "Thanks {{ reviewer }} for {{ file }}:{{ line }}",
    );

    let maybe_draft = app.reply_draft.as_ref();
    assert!(maybe_draft.is_some(), "draft should exist");
    if let Some(draft) = maybe_draft {
        assert_eq!(draft.text(), "Thanks alice for src/main.rs:12");
    }
    assert!(app.error_message().is_none());
}

#[rstest]
fn insert_template_rejects_unconfigured_index(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews).with_reply_draft_config(ReplyDraftConfig::new(
        ReplyDraftMaxLength::new(200),
        vec!["Template one".to_owned()],
    ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertTemplate { template_index: 1 });

    let error = app.error_message().unwrap_or_default();
    assert!(error.contains("not configured"));
}

#[rstest]
fn insert_template_requires_active_draft(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews).with_reply_draft_config(ReplyDraftConfig::new(
        ReplyDraftMaxLength::new(200),
        vec!["Template one".to_owned()],
    ));

    app.handle_message(&AppMsg::ReplyDraftInsertTemplate { template_index: 0 });

    assert_eq!(
        app.error_message(),
        Some("No active reply draft. Press 'a' to start drafting.")
    );
}

#[rstest]
fn insert_template_rejects_mismatched_active_draft(sample_reviews_pair: Vec<ReviewComment>) {
    let mut app =
        ReviewApp::new(sample_reviews_pair).with_reply_draft_config(ReplyDraftConfig::new(
            ReplyDraftMaxLength::new(200),
            vec!["Template one".to_owned()],
        ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::CursorDown);
    app.handle_message(&AppMsg::ReplyDraftInsertTemplate { template_index: 0 });

    assert_eq!(
        app.error_message(),
        Some(
            "Active reply draft does not match the selected comment. Cancel and restart drafting."
        )
    );
}

#[rstest]
fn insertion_enforces_length_limit(sample_reviews: Vec<ReviewComment>) {
    let app = app_with_inserted_template(sample_reviews, 5, "This template is too long");

    let maybe_draft = app.reply_draft.as_ref();
    assert!(maybe_draft.is_some(), "draft should exist");
    if let Some(draft) = maybe_draft {
        assert_eq!(draft.text(), "");
    }

    let error = app.error_message().unwrap_or_default();
    assert!(error.contains("exceeds configured limit"));
}

#[rstest]
fn request_send_marks_draft_ready(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('o'));
    app.handle_message(&AppMsg::ReplyDraftInsertChar('k'));
    app.handle_message(&AppMsg::ReplyDraftRequestSend);

    assert_draft_readiness(&app, true);
}

#[rstest]
fn editing_after_ready_clears_ready_state(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('o'));
    app.handle_message(&AppMsg::ReplyDraftRequestSend);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('k'));

    assert_draft_readiness(&app, false);
}

#[rstest]
fn cancel_reply_draft_discards_state(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('x'));
    app.handle_message(&AppMsg::ReplyDraftCancel);

    assert!(app.reply_draft.is_none());
    assert!(app.error_message().is_none());
}

#[tokio::test]
async fn ai_rewrite_generated_preview_can_be_applied() -> Result<(), &'static str> {
    let mut app = ReviewApp::new(sample_reviews()).with_comment_rewrite_service(Arc::new(
        StubCommentRewriteService::success("Expanded suggestion"),
    ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('h'));
    app.handle_message(&AppMsg::ReplyDraftInsertChar('i'));

    let msg = trigger_ai_rewrite(&mut app, CommentRewriteMode::Expand).await?;
    app.handle_message(&msg);

    if app.reply_draft_ai_preview.is_none() {
        return Err("preview should be present");
    }
    app.handle_message(&AppMsg::ReplyDraftAiApply);

    let draft = app
        .reply_draft
        .as_ref()
        .ok_or("draft should remain active")?;
    if draft.text() != "Expanded suggestion" {
        return Err("draft text should match applied AI suggestion");
    }
    if draft.origin_label() != Some("AI-originated") {
        return Err("draft should preserve AI-originated label after apply");
    }
    if app.reply_draft_ai_preview.is_some() {
        return Err("preview should be cleared");
    }
    if app.error_message().is_some() {
        return Err("error should be cleared after apply");
    }
    Ok(())
}

#[tokio::test]
async fn ai_rewrite_fallback_preserves_original_draft() -> Result<(), &'static str> {
    let mut app = ReviewApp::new(sample_reviews()).with_comment_rewrite_service(Arc::new(
        StubCommentRewriteService::failure(IntakeError::Network {
            message: "timeout".to_owned(),
        }),
    ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('o'));
    app.handle_message(&AppMsg::ReplyDraftInsertChar('k'));

    let msg = trigger_ai_rewrite(&mut app, CommentRewriteMode::Reword).await?;
    app.handle_message(&msg);

    let draft = app
        .reply_draft
        .as_ref()
        .ok_or("draft should remain active")?;
    if draft.text() != "ok" {
        return Err("fallback should preserve original draft text");
    }
    if app.reply_draft_ai_preview.is_some() {
        return Err("fallback should clear AI preview");
    }
    let error_text = app.error_message().unwrap_or_default();
    if !error_text.contains("AI request failed") {
        return Err("fallback should surface AI request failure");
    }
    Ok(())
}

#[tokio::test]
async fn ai_rewrite_preview_can_be_discarded() -> Result<(), &'static str> {
    let mut app = ReviewApp::new(sample_reviews())
        .with_comment_rewrite_service(Arc::new(StubCommentRewriteService::success("AI candidate")));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('x'));
    let msg = trigger_ai_rewrite(&mut app, CommentRewriteMode::Expand).await?;
    app.handle_message(&msg);

    if app.reply_draft_ai_preview.is_none() {
        return Err("preview should be present before discard");
    }
    app.handle_message(&AppMsg::ReplyDraftAiDiscard);
    if app.reply_draft_ai_preview.is_some() {
        return Err("preview should be cleared after discard");
    }
    Ok(())
}

#[test]
fn ai_rewrite_request_with_empty_draft_sets_error_and_returns_no_command() {
    let mut app = ReviewApp::new(sample_reviews())
        .with_comment_rewrite_service(Arc::new(StubCommentRewriteService::success("unused")));

    app.handle_message(&AppMsg::StartReplyDraft);

    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });

    assert!(
        maybe_cmd.is_none(),
        "empty drafts should not dispatch commands"
    );
    assert_eq!(
        app.error_message(),
        Some("Reply draft is empty; type text before AI rewrite.")
    );
}

#[test]
fn ai_rewrite_request_without_active_draft_sets_error_and_returns_no_command() {
    let mut app = ReviewApp::new(sample_reviews())
        .with_comment_rewrite_service(Arc::new(StubCommentRewriteService::success("unused")));

    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });

    assert!(
        maybe_cmd.is_none(),
        "missing active draft should not dispatch commands"
    );
    assert_eq!(
        app.error_message(),
        Some("No active reply draft. Press 'a' to start drafting.")
    );
}

#[test]
fn ai_rewrite_request_without_selected_comment_sets_error_and_returns_no_command() {
    let mut app = ReviewApp::empty()
        .with_comment_rewrite_service(Arc::new(StubCommentRewriteService::success("unused")));

    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });

    assert!(
        maybe_cmd.is_none(),
        "missing selection should not dispatch commands"
    );
    assert_eq!(
        app.error_message(),
        Some("Reply drafting requires a selected comment")
    );
}

#[test]
fn ai_rewrite_ready_after_draft_cleared_sets_error_and_keeps_preview_cleared() {
    let mut app = ReviewApp::new(sample_reviews()).with_comment_rewrite_service(Arc::new(
        StubCommentRewriteService::success("Expanded suggestion"),
    ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('h'));
    app.handle_message(&AppMsg::ReplyDraftInsertChar('i'));

    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });
    assert!(maybe_cmd.is_some(), "rewrite should schedule async command");

    app.handle_message(&AppMsg::ReplyDraftCancel);
    assert!(app.reply_draft.is_none(), "draft should be cleared");

    app.handle_message(&AppMsg::ReplyDraftAiRewriteReady {
        request_id: 1,
        mode: CommentRewriteMode::Expand,
        outcome: crate::ai::CommentRewriteOutcome::generated("Expanded suggestion"),
    });

    assert!(
        app.reply_draft.is_none(),
        "late rewrite must not resurrect draft"
    );
    assert!(
        app.reply_draft_ai_preview.is_none(),
        "late rewrite must not keep preview"
    );
    assert!(app.error_message().is_none());
}

#[test]
fn ai_rewrite_task_failure_falls_back_with_error_message() {
    let mut app = ReviewApp::new(sample_reviews())
        .with_comment_rewrite_service(Arc::new(StubCommentRewriteService::success("unused")));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('h'));
    app.handle_message(&AppMsg::ReplyDraftInsertChar('i'));

    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });
    assert!(maybe_cmd.is_some(), "rewrite should schedule async command");

    app.handle_message(&AppMsg::ReplyDraftAiRewriteReady {
        request_id: 1,
        mode: CommentRewriteMode::Expand,
        outcome: crate::ai::CommentRewriteOutcome::fallback(
            "hi",
            "AI rewrite task failed: join error",
        ),
    });

    assert!(
        app.reply_draft_ai_preview.is_none(),
        "fallback should clear pending preview"
    );
    let error = app.error_message().unwrap_or_default();
    assert!(
        error.contains("AI rewrite task failed"),
        "expected task-failure message, got: {error}"
    );

    let maybe_draft = app.reply_draft.as_ref();
    assert!(maybe_draft.is_some(), "draft should remain active");
    if let Some(draft) = maybe_draft {
        assert_eq!(draft.text(), "hi");
    }
}

#[test]
fn backspace_clears_ai_rewrite_preview() {
    let mut app = ReviewApp::new(sample_reviews())
        .with_comment_rewrite_service(Arc::new(StubCommentRewriteService::success("unused")));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('h'));
    app.handle_message(&AppMsg::ReplyDraftInsertChar('i'));
    app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });
    app.handle_message(&AppMsg::ReplyDraftAiRewriteReady {
        request_id: 1,
        mode: CommentRewriteMode::Expand,
        outcome: crate::ai::CommentRewriteOutcome::generated("Expanded suggestion"),
    });
    assert!(
        app.reply_draft_ai_preview.is_some(),
        "preview should be set"
    );

    app.handle_message(&AppMsg::ReplyDraftBackspace);

    assert!(
        app.reply_draft_ai_preview.is_none(),
        "backspace should clear stale AI preview"
    );
}

#[test]
fn stale_ai_rewrite_completion_does_not_replace_latest_preview() {
    let mut app = ReviewApp::new(sample_reviews())
        .with_comment_rewrite_service(Arc::new(StubCommentRewriteService::success("unused")));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('h'));
    app.handle_message(&AppMsg::ReplyDraftInsertChar('i'));

    let first_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });
    assert!(
        first_cmd.is_some(),
        "first request should produce a command"
    );
    let second_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Reword,
    });
    assert!(
        second_cmd.is_some(),
        "second request should produce a command"
    );

    app.handle_message(&AppMsg::ReplyDraftAiRewriteReady {
        request_id: 2,
        mode: CommentRewriteMode::Reword,
        outcome: crate::ai::CommentRewriteOutcome::generated("Second suggestion"),
    });
    let latest_preview = app
        .reply_draft_ai_preview
        .as_ref()
        .map(|preview| preview.rewritten_text.clone());
    assert_eq!(
        latest_preview.as_deref(),
        Some("Second suggestion"),
        "latest request should control preview state"
    );

    app.handle_message(&AppMsg::ReplyDraftAiRewriteReady {
        request_id: 1,
        mode: CommentRewriteMode::Expand,
        outcome: crate::ai::CommentRewriteOutcome::generated("First suggestion"),
    });
    let preview_after_stale = app
        .reply_draft_ai_preview
        .as_ref()
        .map(|preview| preview.rewritten_text.clone());
    assert_eq!(
        preview_after_stale.as_deref(),
        Some("Second suggestion"),
        "stale completion should not overwrite the latest preview"
    );
}
