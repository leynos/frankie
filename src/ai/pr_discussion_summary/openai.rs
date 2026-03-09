//! OpenAI-compatible HTTP implementation for PR-discussion summaries.

use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::Serialize;

use crate::github::IntakeError;

use super::model::{DiscussionSeverity, PrDiscussionSummary, PrDiscussionSummaryRequest};
use super::service::{
    PrDiscussionSummaryService, ThreadSummaryDraft, ThreadSummaryProvider,
    ThreadSummaryProviderRequest, summarize_with_provider,
};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TIMEOUT_SECS: u64 = 20;

/// Configuration for [`OpenAiPrDiscussionSummaryService`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiPrDiscussionSummaryConfig {
    /// Base API URL (for example `https://api.openai.com/v1`).
    pub base_url: String,
    /// Model identifier sent in chat-completions requests.
    pub model: String,
    /// API key used for bearer authentication.
    pub api_key: Option<String>,
    /// HTTP timeout.
    pub timeout: Duration,
    /// Additional request headers used by deterministic tests.
    #[cfg(any(test, feature = "test-support"))]
    pub additional_headers: Vec<(String, String)>,
}

impl Default for OpenAiPrDiscussionSummaryConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            api_key: None,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            #[cfg(any(test, feature = "test-support"))]
            additional_headers: Vec::new(),
        }
    }
}

impl OpenAiPrDiscussionSummaryConfig {
    /// Constructs configuration with explicit API settings.
    #[must_use]
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            api_key,
            timeout,
            #[cfg(any(test, feature = "test-support"))]
            additional_headers: Vec::new(),
        }
    }

    /// Adds one extra HTTP header.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn with_additional_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.additional_headers.push((name.into(), value.into()));
        self
    }
}

/// OpenAI-compatible PR-discussion summary service.
#[derive(Debug, Clone, Default)]
pub struct OpenAiPrDiscussionSummaryService {
    config: OpenAiPrDiscussionSummaryConfig,
}

impl OpenAiPrDiscussionSummaryService {
    /// Creates a service from explicit configuration.
    #[must_use]
    pub const fn new(config: OpenAiPrDiscussionSummaryConfig) -> Self {
        Self { config }
    }

    fn extract_api_key(&self) -> Result<&str, IntakeError> {
        self.config
            .api_key
            .as_deref()
            .ok_or_else(|| IntakeError::Configuration {
                message: concat!(
                    "AI API key is required (use --ai-api-key, ",
                    "FRANKIE_AI_API_KEY, or OPENAI_API_KEY)"
                )
                .to_owned(),
            })
    }

    fn create_http_client(&self) -> Result<Client, IntakeError> {
        Client::builder()
            .timeout(self.config.timeout)
            .build()
            .map_err(|error| IntakeError::Configuration {
                message: format!("failed to configure AI HTTP client: {error}"),
            })
    }
}

impl PrDiscussionSummaryService for OpenAiPrDiscussionSummaryService {
    fn summarize(
        &self,
        request: &PrDiscussionSummaryRequest,
    ) -> Result<PrDiscussionSummary, IntakeError> {
        summarize_with_provider(self, request)
    }
}

impl ThreadSummaryProvider for OpenAiPrDiscussionSummaryService {
    fn summarize_threads(
        &self,
        request: &ThreadSummaryProviderRequest<'_>,
    ) -> Result<Vec<ThreadSummaryDraft>, IntakeError> {
        let api_key = self.extract_api_key()?;
        let client = self.create_http_client()?;
        let payload = build_chat_request(self.config.model.as_str(), request)?;
        let request_spec = SummaryRequestSpec {
            endpoint: self.chat_completions_endpoint(),
            api_key,
            payload: &payload,
        };
        let response_payload = self.send_chat_request(&client, &request_spec)?;

        parse_summary_response(&response_payload)
    }
}

struct SummaryRequestSpec<'a> {
    endpoint: String,
    api_key: &'a str,
    payload: &'a ChatCompletionsRequest<'a>,
}

#[derive(Debug, Serialize)]
struct ChatCompletionsRequest<'a> {
    model: &'a str,
    messages: Vec<ChatCompletionsMessage>,
}

#[derive(Debug, Serialize)]
struct ChatCompletionsMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, serde::Deserialize)]
struct ChatCompletionsResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, serde::Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum ChatContent {
    Text(String),
    Parts(Vec<ChatContentPart>),
}

#[derive(Debug, serde::Deserialize)]
struct ChatContentPart {
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

impl OpenAiPrDiscussionSummaryService {
    fn chat_completions_endpoint(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }

    fn send_chat_request(
        &self,
        client: &Client,
        request_spec: &SummaryRequestSpec<'_>,
    ) -> Result<ChatCompletionsResponse, IntakeError> {
        #[cfg(any(test, feature = "test-support"))]
        let mut request_builder = client
            .post(request_spec.endpoint.as_str())
            .bearer_auth(request_spec.api_key)
            .json(request_spec.payload);
        #[cfg(not(any(test, feature = "test-support")))]
        let request_builder = client
            .post(request_spec.endpoint.as_str())
            .bearer_auth(request_spec.api_key)
            .json(request_spec.payload);
        #[cfg(any(test, feature = "test-support"))]
        for (name, value) in &self.config.additional_headers {
            request_builder = request_builder.header(name, value);
        }

        let response = request_builder
            .send()
            .map_err(|error| IntakeError::Network {
                message: format!("AI request transport failed: {error}"),
            })?;

        handle_response(response)
    }
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

fn build_chat_request<'a>(
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

fn build_prompt(request: &ThreadSummaryProviderRequest<'_>) -> Result<String, IntakeError> {
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

fn parse_content_value(content: &ChatContent) -> Option<&str> {
    match content {
        ChatContent::Text(text) => Some(text.as_str()),
        ChatContent::Parts(parts) => parts
            .iter()
            .find_map(|part| part.text.as_deref().or(part.content.as_deref())),
    }
}

fn handle_response(
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

fn parse_summary_response(
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

#[cfg(test)]
#[path = "openai_tests.rs"]
mod tests;
