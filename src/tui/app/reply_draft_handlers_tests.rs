//! Tests for reply-drafting handlers.

use std::sync::Arc;

use bubbletea_rs::Cmd;
use rstest::{fixture, rstest};

use crate::ai::{CommentRewriteMode, CommentRewriteRequest, CommentRewriteService};
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

#[derive(Debug)]
struct StubRewriteService {
    response: Result<String, IntakeError>,
}

impl CommentRewriteService for StubRewriteService {
    fn rewrite_text(&self, _request: &CommentRewriteRequest) -> Result<String, IntakeError> {
        self.response.clone()
    }
}

async fn resolve_cmd_to_app_msg(cmd: Cmd) -> Option<AppMsg> {
    let maybe_boxed = cmd.await?;
    maybe_boxed
        .downcast::<AppMsg>()
        .ok()
        .map(|message| *message)
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
async fn ai_rewrite_generated_preview_can_be_applied() {
    let mut app = ReviewApp::new(sample_reviews()).with_comment_rewrite_service(Arc::new(
        StubRewriteService {
            response: Ok("Expanded suggestion".to_owned()),
        },
    ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('h'));
    app.handle_message(&AppMsg::ReplyDraftInsertChar('i'));

    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });
    let Some(cmd) = maybe_cmd else {
        panic!("AI rewrite should return a command");
    };

    let maybe_msg = resolve_cmd_to_app_msg(cmd).await;
    let Some(msg) = maybe_msg else {
        panic!("AI rewrite command should emit a message");
    };
    app.handle_message(&msg);

    assert!(
        app.reply_draft_ai_preview.is_some(),
        "preview should be present"
    );
    app.handle_message(&AppMsg::ReplyDraftAiApply);

    let maybe_draft = app.reply_draft.as_ref();
    assert!(maybe_draft.is_some(), "draft should remain active");
    if let Some(draft) = maybe_draft {
        assert_eq!(draft.text(), "Expanded suggestion");
        assert_eq!(draft.origin_label(), Some("AI-originated"));
    }
    assert!(
        app.reply_draft_ai_preview.is_none(),
        "preview should be cleared"
    );
    assert!(app.error_message().is_none());
}

#[tokio::test]
async fn ai_rewrite_fallback_preserves_original_draft() {
    let mut app = ReviewApp::new(sample_reviews()).with_comment_rewrite_service(Arc::new(
        StubRewriteService {
            response: Err(IntakeError::Network {
                message: "timeout".to_owned(),
            }),
        },
    ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('o'));
    app.handle_message(&AppMsg::ReplyDraftInsertChar('k'));

    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Reword,
    });
    let Some(cmd) = maybe_cmd else {
        panic!("AI rewrite should return a command");
    };

    let maybe_msg = resolve_cmd_to_app_msg(cmd).await;
    let Some(msg) = maybe_msg else {
        panic!("AI rewrite command should emit a message");
    };
    app.handle_message(&msg);

    let maybe_draft = app.reply_draft.as_ref();
    assert!(maybe_draft.is_some(), "draft should remain active");
    if let Some(draft) = maybe_draft {
        assert_eq!(draft.text(), "ok");
    }
    assert!(app.reply_draft_ai_preview.is_none());
    let error_text = app.error_message().unwrap_or_default();
    assert!(error_text.contains("AI request failed"));
}

#[tokio::test]
async fn ai_rewrite_preview_can_be_discarded() {
    let mut app = ReviewApp::new(sample_reviews()).with_comment_rewrite_service(Arc::new(
        StubRewriteService {
            response: Ok("AI candidate".to_owned()),
        },
    ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('x'));
    let maybe_cmd = app.handle_message(&AppMsg::ReplyDraftRequestAiRewrite {
        mode: CommentRewriteMode::Expand,
    });
    let Some(cmd) = maybe_cmd else {
        panic!("AI rewrite should return a command");
    };
    let maybe_msg = resolve_cmd_to_app_msg(cmd).await;
    let Some(msg) = maybe_msg else {
        panic!("AI rewrite command should emit a message");
    };
    app.handle_message(&msg);

    assert!(app.reply_draft_ai_preview.is_some());
    app.handle_message(&AppMsg::ReplyDraftAiDiscard);
    assert!(app.reply_draft_ai_preview.is_none());
}
