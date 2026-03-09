//! Unit tests for the OpenAI-compatible PR-discussion summary adapter.

use std::time::Duration;

use rstest::rstest;

use crate::ai::pr_discussion_summary::{
    OpenAiPrDiscussionSummaryConfig, OpenAiPrDiscussionSummaryService, PrDiscussionSummaryRequest,
    PrDiscussionSummaryService,
};
use crate::github::IntakeError;
use crate::github::models::test_support::minimal_review;

use super::{ChatContent, build_prompt, parse_content_value};

#[test]
fn parse_content_value_supports_string_and_array() {
    let as_string: ChatContent =
        serde_json::from_value(serde_json::json!("hello")).expect("string content should decode");
    let as_array: ChatContent =
        serde_json::from_value(serde_json::json!([{"text":"first"}, {"content":"second"}]))
            .expect("array content should decode");

    assert_eq!(parse_content_value(&as_string), Some("hello"));
    assert_eq!(parse_content_value(&as_array), Some("first"));
}

#[rstest]
fn summarize_requires_api_key() {
    let service = OpenAiPrDiscussionSummaryService::default();
    let request = PrDiscussionSummaryRequest::new(
        42,
        Some("Title".to_owned()),
        vec![minimal_review(1, "body", "alice")],
    );

    let error = service
        .summarize(&request)
        .expect_err("missing key should be rejected");

    assert!(
        matches!(error, IntakeError::Configuration { .. }),
        "expected missing API key to map to Configuration error, got {error:?}"
    );
}

#[test]
fn build_prompt_serializes_thread_context() {
    let service = OpenAiPrDiscussionSummaryService::new(OpenAiPrDiscussionSummaryConfig::new(
        "http://localhost:8100/v1",
        "gpt-4",
        Some("sk-test".to_owned()),
        Duration::from_secs(1),
    ));
    let request = PrDiscussionSummaryRequest::new(
        42,
        Some("Title".to_owned()),
        vec![minimal_review(1, "body", "alice")],
    );
    let threads =
        crate::ai::pr_discussion_summary::service::summarize_with_provider(&service, &request)
            .expect_err("without server this path should still build a prompt before transport");
    let prompt = build_prompt(
        &crate::ai::pr_discussion_summary::service::ThreadSummaryProviderRequest {
            pr_number: 42,
            pr_title: Some("Title"),
            threads: &crate::ai::pr_discussion_summary::threads::build_discussion_threads(&request),
        },
    )
    .expect("prompt should serialize");

    assert!(prompt.contains("\"pr_number\": 42"));
    assert!(prompt.contains("\"root_comment_id\": 1"));
    let _ = threads;
}
