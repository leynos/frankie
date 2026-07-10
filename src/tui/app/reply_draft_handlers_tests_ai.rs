//! AI-rewrite preview tests for reply-drafting handlers.

use rstest::rstest;

use super::*;

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

/// Helper to assert that an AI rewrite request on an invalid draft state
/// returns no command and sets the expected error message.
fn assert_ai_rewrite_request_fails_with_error<F>(setup: F, expected_error: &str)
where
    F: FnOnce() -> ReviewApp,
{
    let mut app = setup();
    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });

    assert!(
        maybe_cmd.is_none(),
        "invalid draft state should not dispatch commands"
    );
    assert_eq!(app.error_message(), Some(expected_error));
}

/// Builds an app with a stub rewrite service whose response is unused.
fn app_with_unused_stub() -> ReviewApp {
    ReviewApp::new(sample_reviews())
        .with_comment_rewrite_service(Arc::new(StubCommentRewriteService::success("unused")))
}

fn app_with_empty_draft() -> ReviewApp {
    let mut app = app_with_unused_stub();
    app.handle_message(&AppMsg::StartReplyDraft);
    app
}

fn app_without_selected_comment() -> ReviewApp {
    ReviewApp::empty()
        .with_comment_rewrite_service(Arc::new(StubCommentRewriteService::success("unused")))
}

#[rstest]
#[case::empty_draft(
    app_with_empty_draft as fn() -> ReviewApp,
    "Reply draft is empty; type text before AI rewrite."
)]
#[case::without_active_draft(
    app_with_unused_stub as fn() -> ReviewApp,
    "No active reply draft. Press 'a' to start drafting."
)]
#[case::without_selected_comment(
    app_without_selected_comment as fn() -> ReviewApp,
    "Reply drafting requires a selected comment"
)]
fn ai_rewrite_request_in_invalid_state_sets_error_and_returns_no_command(
    #[case] setup: fn() -> ReviewApp,
    #[case] expected_error: &str,
) {
    assert_ai_rewrite_request_fails_with_error(setup, expected_error);
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
