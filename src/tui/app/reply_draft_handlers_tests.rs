//! Tests for reply-drafting handlers.

use rstest::{fixture, rstest};

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
