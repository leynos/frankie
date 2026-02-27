//! Unit tests for the OpenAI-compatible comment rewrite adapter.

use rstest::rstest;

use crate::ai::comment_rewrite::{CommentRewriteRequest, CommentRewriteService};
use crate::github::IntakeError;

use super::{ChatContent, OpenAiCommentRewriteService, parse_content_value};
use rewrite_request_fixture::rewrite_request;

mod rewrite_request_fixture;

#[test]
fn parse_content_value_supports_string_and_array() {
    let as_string: ChatContent =
        serde_json::from_value(serde_json::json!("hello")).expect("string content should decode");
    let as_array: ChatContent =
        serde_json::from_value(serde_json::json!([{"text":"first"}, {"text":"second"}]))
            .expect("array content should decode");

    assert_eq!(parse_content_value(&as_string), Some("hello"));
    assert_eq!(parse_content_value(&as_array), Some("first"));
}

#[rstest]
fn rewrite_text_requires_api_key(rewrite_request: CommentRewriteRequest) {
    let service = OpenAiCommentRewriteService::default();
    let error = service
        .rewrite_text(&rewrite_request)
        .expect_err("missing key should be rejected");

    assert!(
        matches!(error, IntakeError::Configuration { .. }),
        "expected missing API key to map to Configuration error, got {error:?}"
    );
}
