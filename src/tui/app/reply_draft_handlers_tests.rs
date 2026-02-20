//! Tests for reply-drafting handlers.

use rstest::{fixture, rstest};

use crate::github::models::ReviewComment;
use crate::tui::ReplyDraftConfig;
use crate::tui::messages::AppMsg;

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

    let draft = app.reply_draft.as_ref().expect("draft should be created");
    assert_eq!(draft.comment_id(), 1);
    assert_eq!(draft.text(), "");
    assert!(!draft.is_ready_to_send());
}

#[rstest]
fn insert_template_renders_comment_fields(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews).with_reply_draft_config(ReplyDraftConfig::new(
        200,
        vec!["Thanks {{ reviewer }} for {{ file }}:{{ line }}".to_owned()],
    ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertTemplate { template_index: 0 });

    let draft = app.reply_draft.as_ref().expect("draft should exist");
    assert_eq!(draft.text(), "Thanks alice for src/main.rs:12");
    assert!(app.error_message().is_none());
}

#[rstest]
fn insert_template_rejects_unconfigured_index(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews)
        .with_reply_draft_config(ReplyDraftConfig::new(200, vec!["Template one".to_owned()]));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertTemplate { template_index: 1 });

    let error = app.error_message().unwrap_or_default();
    assert!(error.contains("not configured"));
}

#[rstest]
fn insertion_enforces_length_limit(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews).with_reply_draft_config(ReplyDraftConfig::new(
        5,
        vec!["This template is too long".to_owned()],
    ));

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertTemplate { template_index: 0 });

    let draft = app.reply_draft.as_ref().expect("draft should exist");
    assert_eq!(draft.text(), "");

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

    let draft = app.reply_draft.as_ref().expect("draft should exist");
    assert!(draft.is_ready_to_send());
}

#[rstest]
fn editing_after_ready_clears_ready_state(sample_reviews: Vec<ReviewComment>) {
    let mut app = ReviewApp::new(sample_reviews);

    app.handle_message(&AppMsg::StartReplyDraft);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('o'));
    app.handle_message(&AppMsg::ReplyDraftRequestSend);
    app.handle_message(&AppMsg::ReplyDraftInsertChar('k'));

    let draft = app.reply_draft.as_ref().expect("draft should exist");
    assert!(!draft.is_ready_to_send());
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
