//! OpenAI-compatible HTTP implementation for AI comment rewriting.

use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::Value;

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
    pub additional_headers: Vec<(String, String)>,
}

impl Default for OpenAiCommentRewriteConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            api_key: None,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
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
            additional_headers: Vec::new(),
        }
    }

    /// Adds one extra HTTP header.
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

    fn build_endpoint(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }

    fn build_chat_payload(&self, request: &CommentRewriteRequest) -> ChatCompletionsRequest<'_> {
        let prompt = build_prompt(request);

        ChatCompletionsRequest {
            model: self.config.model.as_str(),
            messages: vec![
                ChatCompletionsMessage {
                    role: "system",
                    content: build_system_prompt(request.mode()),
                },
                ChatCompletionsMessage {
                    role: "user",
                    content: prompt,
                },
            ],
        }
    }

    fn create_http_client(&self) -> Result<Client, IntakeError> {
        Client::builder()
            .timeout(self.config.timeout)
            .build()
            .map_err(|error| IntakeError::Configuration {
                message: format!("failed to configure AI HTTP client: {error}"),
            })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Signature is required by the requested refactor contract"
    )]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Signature is required by the requested refactor contract"
    )]
    fn build_request(
        &self,
        client: &Client,
        endpoint: &str,
        api_key: &str,
        payload: &ChatCompletionsRequest<'_>,
    ) -> Result<reqwest::blocking::RequestBuilder, IntakeError> {
        let mut request_builder = client.post(endpoint).bearer_auth(api_key).json(payload);
        for (name, value) in &self.config.additional_headers {
            request_builder = request_builder.header(name, value);
        }
        Ok(request_builder)
    }

    #[expect(
        clippy::unused_self,
        reason = "Signature is required by the requested refactor contract"
    )]
    fn handle_error_response(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<String, IntakeError> {
        let status = response.status();
        let body = response.text().map_or_else(
            |_| "(failed to read error response body)".to_owned(),
            |content| truncate_for_message(content.as_str(), 160),
        );
        Err(IntakeError::Api {
            message: format!("AI request failed with status {}: {body}", status.as_u16()),
        })
    }

    #[expect(
        clippy::unused_self,
        reason = "Signature is required by the requested refactor contract"
    )]
    fn extract_response_text(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<String, IntakeError> {
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

impl CommentRewriteService for OpenAiCommentRewriteService {
    fn rewrite_text(&self, request: &CommentRewriteRequest) -> Result<String, IntakeError> {
        let api_key = self.extract_api_key()?;
        let endpoint = self.build_endpoint();
        let payload = self.build_chat_payload(request);
        let client = self.create_http_client()?;
        let request_builder = self.build_request(&client, endpoint.as_str(), api_key, &payload)?;

        let response = request_builder
            .send()
            .map_err(|error| IntakeError::Network {
                message: format!("AI request transport failed: {error}"),
            })?;

        if response.status() != StatusCode::OK {
            return self.handle_error_response(response);
        }

        self.extract_response_text(response)
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
struct ChatChoiceMessage {
    content: Value,
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

fn parse_content_value(content: &Value) -> Option<&str> {
    if let Some(text) = content.as_str() {
        return Some(text);
    }

    content.as_array().and_then(|items| {
        items.iter().find_map(|item| {
            item.get("text")
                .and_then(Value::as_str)
                .or_else(|| item.get("content").and_then(Value::as_str))
        })
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
mod tests {
    use std::net::TcpListener;
    use std::process::{Child, Command, Stdio};
    use std::thread;
    use std::time::Duration;

    use rstest::{fixture, rstest};

    use crate::ai::comment_rewrite::{
        CommentRewriteContext, CommentRewriteMode, CommentRewriteOutcome, CommentRewriteRequest,
        CommentRewriteService, rewrite_with_fallback,
    };

    use super::{OpenAiCommentRewriteConfig, OpenAiCommentRewriteService, parse_content_value};

    struct VidaiServer {
        base_url: String,
        child: Child,
    }

    impl Drop for VidaiServer {
        fn drop(&mut self) {
            let _kill_ignored = self.child.kill();
            let _wait_ignored = self.child.wait();
        }
    }

    #[fixture]
    fn rewrite_request() -> CommentRewriteRequest {
        CommentRewriteRequest::new(
            CommentRewriteMode::Expand,
            "Please fix this",
            CommentRewriteContext::default(),
        )
    }

    #[test]
    fn parse_content_value_supports_string_and_array() {
        let as_string = serde_json::json!("hello");
        let as_array = serde_json::json!([
            {"type":"text", "text":"first"},
            {"type":"text", "text":"second"}
        ]);

        assert_eq!(parse_content_value(&as_string), Some("hello"));
        assert_eq!(parse_content_value(&as_array), Some("first"));
    }

    #[rstest]
    fn rewrite_text_requires_api_key(rewrite_request: CommentRewriteRequest) {
        let service = OpenAiCommentRewriteService::default();
        let result = service.rewrite_text(&rewrite_request);

        assert!(result.is_err(), "missing key should be rejected");
    }

    #[rstest]
    fn rewrite_text_reads_mock_response_from_vidaimock(rewrite_request: CommentRewriteRequest) {
        let Some(server) = spawn_vidaimock() else {
            return;
        };

        let config = OpenAiCommentRewriteConfig::new(
            format!("{}/v1", server.base_url),
            "gpt-4",
            Some("sk-test".to_owned()),
            Duration::from_secs(2),
        );
        let service = OpenAiCommentRewriteService::new(config);
        let result = service.rewrite_text(&rewrite_request);

        assert!(result.is_ok(), "expected successful rewrite from vidaimock");
        let text = result.unwrap_or_default();
        assert!(
            text.contains("mock response"),
            "expected mock output, got: {text}"
        );
    }

    #[rstest]
    fn rewrite_with_fallback_handles_vidaimock_malformed_json(
        rewrite_request: CommentRewriteRequest,
    ) {
        let Some(server) = spawn_vidaimock() else {
            return;
        };

        let config = OpenAiCommentRewriteConfig::new(
            format!("{}/v1", server.base_url),
            "gpt-4",
            Some("sk-test".to_owned()),
            Duration::from_secs(2),
        )
        .with_additional_header("X-Vidai-Chaos-Malformed", "100");

        let service = OpenAiCommentRewriteService::new(config);
        let outcome = rewrite_with_fallback(&service, &rewrite_request);

        assert!(matches!(outcome, CommentRewriteOutcome::Fallback(_)));
    }

    fn spawn_vidaimock() -> Option<VidaiServer> {
        if !vidaimock_available() {
            return None;
        }

        let port = reserve_port().ok()?;
        let mut command = Command::new("vidaimock");
        command
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--format")
            .arg("openai")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = command.spawn().ok()?;
        let base_url = format!("http://127.0.0.1:{port}");

        wait_for_server(base_url.as_str());

        Some(VidaiServer { base_url, child })
    }

    fn vidaimock_available() -> bool {
        Command::new("vidaimock")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }

    fn reserve_port() -> Result<u16, std::io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        Ok(port)
    }

    fn wait_for_server(base_url: &str) {
        let metrics_url = format!("{base_url}/metrics");
        for _ in 0..40 {
            if reqwest::blocking::get(metrics_url.as_str()).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}
