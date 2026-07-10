//! OpenAI-compatible HTTP implementation for PR-discussion summaries.

use std::time::Duration;

use reqwest::blocking::Client;

use crate::github::IntakeError;

use super::model::{PrDiscussionSummary, PrDiscussionSummaryRequest};
use super::service::{
    PrDiscussionSummaryService, ThreadSummaryDraft, ThreadSummaryProvider,
    ThreadSummaryProviderRequest, summarize_with_provider,
};

#[path = "openai_protocol.rs"]
mod protocol;

use protocol::{
    ChatCompletionsRequest, ChatCompletionsResponse, build_chat_request, handle_response,
    parse_summary_response,
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
    /// Summarizes threads using blocking HTTP calls.
    ///
    /// The blocking duration is bounded by the configured HTTP client timeout,
    /// which defaults to 20 seconds in `create_http_client`. Adjust that
    /// timeout there when callers need a different upper bound.
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
        let request_builder = {
            #[cfg_attr(not(any(test, feature = "test-support")), allow(unused_mut))]
            let mut builder = client
                .post(request_spec.endpoint.as_str())
                .bearer_auth(request_spec.api_key)
                .json(request_spec.payload);
            #[cfg(any(test, feature = "test-support"))]
            for (name, value) in &self.config.additional_headers {
                builder = builder.header(name, value);
            }
            builder
        };

        let response = request_builder
            .send()
            .map_err(|error| IntakeError::Network {
                message: format!("AI request transport failed: {error}"),
            })?;

        handle_response(response)
    }
}

#[cfg(test)]
#[path = "openai_tests.rs"]
mod tests;
