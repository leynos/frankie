//! Wire-format types and pure request/response logic for the
//! OpenAI-compatible chat-completions protocol.
//!
//! Keeping serialization shapes and parsing helpers here leaves
//! `openai.rs` focused on configuration and HTTP transport.

use reqwest::StatusCode;
use serde::Serialize;

use crate::github::IntakeError;

use super::super::model::DiscussionSeverity;
use super::super::service::{ThreadSummaryDraft, ThreadSummaryProviderRequest};

/// Chat-completions request payload.
#[derive(Debug, Serialize)]
pub struct ChatCompletionsRequest<'a> {
    model: &'a str,
    messages: Vec<ChatCompletionsMessage>,
}

#[derive(Debug, Serialize)]
struct ChatCompletionsMessage {
    role: &'static str,
    content: String,
}

/// Chat-completions response payload.
#[derive(Debug, serde::Deserialize)]
pub struct ChatCompletionsResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, serde::Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

/// Assistant message content, either plain text or structured parts.
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum ChatContent {
    /// Plain string content.
    Text(String),
    /// Structured multi-part content.
    Parts(Vec<ChatContentPart>),
}

/// One part of a structured assistant message.
#[derive(Debug, serde::Deserialize)]
pub struct ChatContentPart {
    text: Option<String>,
    content: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct ChatChoiceMessage {
    content: ChatContent,
}

#[derive(Debug, Serialize)]
struct PromptPayload<'a> {
    pr_number: u64,
    pr_title: Option<&'a str>,
    threads: Vec<PromptThread<'a>>,
}

#[derive(Debug, Serialize)]
struct PromptThread<'a> {
    root_comment_id: u64,
    file_path: &'a str,
    related_comment_ids: Vec<u64>,
    comments: Vec<PromptComment<'a>>,
}

#[derive(Debug, Serialize)]
struct PromptComment<'a> {
    comment_id: u64,
    is_root: bool,
    author: Option<&'a str>,
    body: &'a str,
    created_at: Option<&'a str>,
    verification_status: Option<&'static str>,
}

#[derive(Debug, serde::Deserialize)]
struct StructuredSummaryResponse {
    summaries: Vec<StructuredSummaryItem>,
}

#[derive(Debug, serde::Deserialize)]
struct StructuredSummaryItem {
    root_comment_id: u64,
    severity: String,
    headline: String,
    rationale: String,
}

fn build_system_prompt() -> String {
    concat!(
        "You summarize pull-request review discussion threads. ",
        "Return strict JSON only with top-level key 'summaries'. ",
        "Each summary must include root_comment_id, severity, headline, and rationale. ",
        "Severity must be one of: high, medium, low. ",
        "Do not invent thread IDs. ",
        "Do not use markdown fences."
    )
    .to_owned()
}

/// Builds the chat-completions request for the given provider request.
pub fn build_chat_request<'a>(
    model: &'a str,
    request: &ThreadSummaryProviderRequest<'_>,
) -> Result<ChatCompletionsRequest<'a>, IntakeError> {
    Ok(ChatCompletionsRequest {
        model,
        messages: vec![
            ChatCompletionsMessage {
                role: "system",
                content: build_system_prompt(),
            },
            ChatCompletionsMessage {
                role: "user",
                content: build_prompt(request)?,
            },
        ],
    })
}

/// Serializes the provider request into a JSON user prompt.
pub fn build_prompt(request: &ThreadSummaryProviderRequest<'_>) -> Result<String, IntakeError> {
    let payload = PromptPayload {
        pr_number: request.pr_number,
        pr_title: request.pr_title,
        threads: request
            .threads
            .iter()
            .map(|thread| PromptThread {
                root_comment_id: thread.root_comment.id,
                file_path: thread.file_path.as_str(),
                related_comment_ids: thread
                    .related_comment_ids
                    .iter()
                    .map(|id| id.as_u64())
                    .collect(),
                comments: thread
                    .comments
                    .iter()
                    .map(|comment| PromptComment {
                        comment_id: comment.comment_id.as_u64(),
                        is_root: comment.is_root,
                        author: comment.author.as_deref(),
                        body: comment.body.as_str(),
                        created_at: comment.created_at.as_deref(),
                        verification_status: comment
                            .verification_status
                            .map(|status| status.as_display_str()),
                    })
                    .collect(),
            })
            .collect(),
    };

    serde_json::to_string_pretty(&payload).map_err(|error| IntakeError::Configuration {
        message: format!("failed to serialize AI summary prompt: {error}"),
    })
}

/// Extracts assistant text from either content representation.
pub fn parse_content_value(content: &ChatContent) -> Option<String> {
    match content {
        ChatContent::Text(text) => Some(text.clone()),
        ChatContent::Parts(parts) => {
            let combined = parts
                .iter()
                .filter_map(|part| part.text.as_deref().or(part.content.as_deref()))
                .collect::<String>();
            (!combined.is_empty()).then_some(combined)
        }
    }
}

/// Converts an HTTP response into a decoded chat-completions payload.
pub fn handle_response(
    response: reqwest::blocking::Response,
) -> Result<ChatCompletionsResponse, IntakeError> {
    if response.status() != StatusCode::OK {
        let status = response.status();
        let body = response.text().map_or_else(
            |_| "(failed to read error response body)".to_owned(),
            |content| truncate_for_message(content.as_str(), 160),
        );
        return Err(IntakeError::Api {
            message: format!("AI request failed with status {}: {body}", status.as_u16()),
        });
    }

    response.json().map_err(|error| IntakeError::Api {
        message: format!("AI response JSON decoding failed: {error}"),
    })
}

/// Parses the structured summary JSON embedded in the assistant reply.
pub fn parse_summary_response(
    response_payload: &ChatCompletionsResponse,
) -> Result<Vec<ThreadSummaryDraft>, IntakeError> {
    let content = response_payload
        .choices
        .first()
        .and_then(|choice| parse_content_value(&choice.message.content))
        .ok_or_else(|| IntakeError::Api {
            message: "AI response did not contain assistant text".to_owned(),
        })?;
    let parsed: StructuredSummaryResponse =
        serde_json::from_str(content.trim()).map_err(|error| IntakeError::Api {
            message: format!("AI summary response schema decoding failed: {error}"),
        })?;

    parsed
        .summaries
        .into_iter()
        .map(parse_structured_summary_item)
        .collect()
}

fn parse_structured_summary_item(
    summary: StructuredSummaryItem,
) -> Result<ThreadSummaryDraft, IntakeError> {
    let severity = summary
        .severity
        .parse::<DiscussionSeverity>()
        .map_err(|error| IntakeError::Api {
            message: error.to_string(),
        })?;

    Ok(ThreadSummaryDraft {
        root_comment_id: summary.root_comment_id.into(),
        severity,
        headline: summary.headline,
        rationale: summary.rationale,
    })
}

fn truncate_for_message(message: &str, max_chars: usize) -> String {
    let mut output = String::new();
    let mut chars = message.chars();

    for _ in 0..max_chars {
        let Some(character) = chars.next() else {
            return output;
        };
        output.push(character);
    }

    if chars.next().is_some() {
        output.push_str("...");
    }

    output
}
