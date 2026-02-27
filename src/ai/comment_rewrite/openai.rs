//! OpenAI-compatible HTTP implementation for AI comment rewriting.

use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::Serialize;

use crate::github::IntakeError;

use super::model::{CommentRewriteMode, CommentRewriteRequest};
use super::service::CommentRewriteService;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TIMEOUT_SECS: u64 = 20;

/// Configuration for [`OpenAiCommentRewriteService`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiCommentRewriteConfig {
    /// Base API URL (e.g., `https://api.openai.com/v1`).
    pub base_url: String,
    /// Model identifier sent in chat-completions requests.
    pub model: String,
    /// API key used for bearer authentication.
    pub api_key: Option<String>,
    /// HTTP timeout.
    pub timeout: Duration,
    /// Additional request headers (primarily useful for deterministic tests).
    #[cfg(any(test, feature = "test-support"))]
    pub additional_headers: Vec<(String, String)>,
}

impl Default for OpenAiCommentRewriteConfig {
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

impl OpenAiCommentRewriteConfig {
    /// Constructs configuration with required API settings.
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

/// OpenAI-compatible rewrite service implementation.
#[derive(Debug, Clone, Default)]
pub struct OpenAiCommentRewriteService {
    config: OpenAiCommentRewriteConfig,
}

impl OpenAiCommentRewriteService {
    /// Creates a service from explicit configuration.
    #[must_use]
    pub const fn new(config: OpenAiCommentRewriteConfig) -> Self {
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

impl CommentRewriteService for OpenAiCommentRewriteService {
    fn rewrite_text(&self, request: &CommentRewriteRequest) -> Result<String, IntakeError> {
        let api_key = self.extract_api_key()?;
        let endpoint = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let payload = ChatCompletionsRequest {
            model: self.config.model.as_str(),
            messages: vec![
                ChatCompletionsMessage {
                    role: "system",
                    content: build_system_prompt(request.mode()),
                },
                ChatCompletionsMessage {
                    role: "user",
                    content: build_prompt(request),
                },
            ],
        };
        let client = self.create_http_client()?;
        #[cfg(any(test, feature = "test-support"))]
        let mut request_builder = client.post(endpoint).bearer_auth(api_key).json(&payload);
        #[cfg(not(any(test, feature = "test-support")))]
        let request_builder = client.post(endpoint).bearer_auth(api_key).json(&payload);
        #[cfg(any(test, feature = "test-support"))]
        for (name, value) in &self.config.additional_headers {
            request_builder = request_builder.header(name, value);
        }

        let response = request_builder
            .send()
            .map_err(|error| IntakeError::Network {
                message: format!("AI request transport failed: {error}"),
            })?;

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

        let response_payload: ChatCompletionsResponse =
            response.json().map_err(|error| IntakeError::Api {
                message: format!("AI response JSON decoding failed: {error}"),
            })?;

        response_payload
            .choices
            .first()
            .and_then(|choice| parse_content_value(&choice.message.content))
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| IntakeError::Api {
                message: "AI response did not contain assistant text".to_owned(),
            })
    }
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

fn build_system_prompt(mode: CommentRewriteMode) -> String {
    let directive = match mode {
        CommentRewriteMode::Expand => {
            "Expand the text into a fuller reply while keeping the same intent."
        }
        CommentRewriteMode::Reword => {
            "Reword the text for clarity and tone while preserving the same intent."
        }
    };

    format!(
        concat!(
            "You rewrite pull-request reply drafts. ",
            "Keep output concise and actionable. ",
            "Do not mention being an AI model. ",
            "{}"
        ),
        directive
    )
}

fn build_prompt(request: &CommentRewriteRequest) -> String {
    let context = request.context();

    let mut prompt = String::new();
    prompt.push_str("Rewrite mode: ");
    prompt.push_str(request.mode().label());
    prompt.push('\n');

    if let Some(reviewer) = context.reviewer.as_deref() {
        prompt.push_str("Reviewer: ");
        prompt.push_str(reviewer);
        prompt.push('\n');
    }

    if let Some(file_path) = context.file_path.as_deref() {
        prompt.push_str("File: ");
        prompt.push_str(file_path);
        prompt.push('\n');
    }

    if let Some(line_number) = context.line_number {
        prompt.push_str("Line: ");
        prompt.push_str(line_number.to_string().as_str());
        prompt.push('\n');
    }

    if let Some(comment_body) = context.comment_body.as_deref() {
        prompt.push_str("Review comment: ");
        prompt.push_str(comment_body);
        prompt.push('\n');
    }

    prompt.push_str("Draft text:\n");
    prompt.push_str(request.source_text());

    prompt
}

fn parse_content_value(content: &ChatContent) -> Option<&str> {
    match content {
        ChatContent::Text(text) => Some(text.as_str()),
        ChatContent::Parts(parts) => parts
            .iter()
            .find_map(|part| part.text.as_deref().or(part.content.as_deref())),
    }
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
