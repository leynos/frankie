//! Integration tests for the `OpenAI` PR discussion summary adapter using `vidaimock`.

use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use frankie::ReviewComment;
use frankie::ai::{
    OpenAiPrDiscussionSummaryConfig, OpenAiPrDiscussionSummaryService, PrDiscussionSummaryRequest,
    PrDiscussionSummaryService,
};
use frankie::github::models::test_support::minimal_review;
use rstest::rstest;

#[path = "support/vidaimock.rs"]
mod vidaimock;

type TestResult<T> = Result<T, Box<dyn Error>>;

fn fixture_config_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vidaimock/pr_discussion_summary")
}

fn sample_request() -> PrDiscussionSummaryRequest {
    PrDiscussionSummaryRequest::new(
        42,
        Some("Add summary mode".to_owned()),
        vec![
            ReviewComment {
                file_path: Some("src/main.rs".to_owned()),
                ..minimal_review(1, "Please handle the panic path.", "alice")
            },
            ReviewComment {
                id: 2,
                in_reply_to_id: Some(1),
                file_path: Some("src/main.rs".to_owned()),
                ..minimal_review(2, "Agreed, this should not panic.", "bob")
            },
            ReviewComment {
                file_path: None,
                ..minimal_review(3, "Please clarify the module comment.", "carol")
            },
        ],
    )
}

#[rstest]
fn summarize_reads_structured_response_from_vidaimock() -> TestResult<()> {
    let Some(server) = vidaimock::spawn_vidaimock(fixture_config_dir().as_path())? else {
        return Ok(());
    };

    let config = OpenAiPrDiscussionSummaryConfig::new(
        format!("{}/v1", server.base_url),
        "gpt-4",
        Some("sk-test".to_owned()),
        Duration::from_secs(2),
    );
    let service = OpenAiPrDiscussionSummaryService::new(config);
    let summary = service.summarize(&sample_request())?;

    if summary.item_count() != 2 {
        return Err(format!("expected 2 summary items, got {}", summary.item_count()).into());
    }
    Ok(())
}

#[rstest]
fn summarize_rejects_vidaimock_malformed_json() -> TestResult<()> {
    let Some(server) = vidaimock::spawn_vidaimock(fixture_config_dir().as_path())? else {
        return Ok(());
    };

    let config = OpenAiPrDiscussionSummaryConfig::new(
        format!("{}/v1", server.base_url),
        "gpt-4",
        Some("sk-test".to_owned()),
        Duration::from_secs(2),
    )
    .with_additional_header("X-Vidai-Chaos-Malformed", "100");

    let service = OpenAiPrDiscussionSummaryService::new(config);
    let result = service.summarize(&sample_request());

    if result.is_ok() {
        return Err("expected malformed response to fail".into());
    }
    Ok(())
}

#[rstest]
fn summarize_surfaces_vidaimock_http_failures() -> TestResult<()> {
    let Some(server) = vidaimock::spawn_vidaimock(fixture_config_dir().as_path())? else {
        return Ok(());
    };

    let config = OpenAiPrDiscussionSummaryConfig::new(
        format!("{}/v1", server.base_url),
        "gpt-4",
        Some("sk-test".to_owned()),
        Duration::from_secs(2),
    )
    .with_additional_header("X-Vidai-Chaos-Drop", "100");

    let service = OpenAiPrDiscussionSummaryService::new(config);
    let result = service.summarize(&sample_request());

    match result {
        Err(frankie::IntakeError::Api { message }) => {
            if !message.contains("status 500") {
                return Err(
                    format!("expected HTTP status in error message, got: {message}").into(),
                );
            }
            if !message.contains("Simulated Internal Server Error") {
                return Err(
                    format!("expected response body in error message, got: {message}").into(),
                );
            }
        }
        other => {
            return Err(
                format!("expected IntakeError::Api for HTTP failure, got: {other:?}").into(),
            );
        }
    }

    Ok(())
}
