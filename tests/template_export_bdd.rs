//! Behavioural tests for template-driven comment export.

#[path = "template_export_bdd/mod.rs"]
mod template_export_bdd_support;

use frankie::{
    ExportedComment, IntakeError, OctocrabReviewCommentGateway, PersonalAccessToken,
    PullRequestLocator, ReviewCommentGateway, sort_comments, write_template,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use template_export_bdd_support::{
    CommentCount, TemplateExportState, ensure_runtime_and_server, generate_reply_comment,
    generate_review_comments,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[fixture]
fn template_state() -> TemplateExportState {
    TemplateExportState::default()
}

/// Mounts a mock GET endpoint for pull request comments on the test server.
fn mount_comments_mock(
    template_state: &TemplateExportState,
    comments: impl serde::Serialize,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = ensure_runtime_and_server(template_state)?;
    let comments_path = "/api/v3/repos/owner/repo/pulls/42/comments";

    let mock = Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&comments));

    template_state
        .server
        .with_ref(|server| {
            runtime.block_on(mock.mount(server));
        })
        .ok_or("mock server not initialised")?;

    Ok(())
}

#[given(
    "a mock GitHub API server with {count:CommentCount} review comments for owner/repo/pull/42"
)]
fn seed_server_with_comments(
    template_state: &TemplateExportState,
    count: CommentCount,
) -> Result<(), Box<dyn std::error::Error>> {
    mount_comments_mock(template_state, generate_review_comments(count))
}

#[given("a mock GitHub API server with a reply comment for owner/repo/pull/42")]
fn seed_server_with_reply_comment(
    template_state: &TemplateExportState,
) -> Result<(), Box<dyn std::error::Error>> {
    mount_comments_mock(template_state, generate_reply_comment())
}

#[given("a personal access token {token}")]
fn remember_token(template_state: &TemplateExportState, token: String) {
    template_state.token.set(token);
}

#[given("a template {template}")]
fn remember_template(template_state: &TemplateExportState, template: String) {
    // Strip surrounding quotes if present
    let cleaned = template.trim_matches('"');
    template_state.template.set(cleaned.to_owned());
}

#[when("the client exports comments using the template for {pr_url}")]
fn export_with_template(template_state: &TemplateExportState, pr_url: String) {
    let result = run_template_export(template_state, &pr_url);

    match result {
        Ok(output) => {
            drop(template_state.error.take());
            template_state.output.set(output);
        }
        Err(error) => {
            drop(template_state.output.take());
            template_state.error.set(error);
        }
    }
}

fn run_template_export(
    template_state: &TemplateExportState,
    pr_url: &str,
) -> Result<String, IntakeError> {
    let server_url = template_state
        .server
        .with_ref(MockServer::uri)
        .ok_or_else(|| IntakeError::Api {
            message: "mock server URL missing".to_owned(),
        })?;

    let resolved_url = resolve_mock_server_url(&server_url, pr_url);
    let locator = PullRequestLocator::parse(&resolved_url)?;

    let runtime = template_state
        .runtime
        .get()
        .ok_or_else(|| IntakeError::Api {
            message: "runtime not initialised".to_owned(),
        })?;

    runtime.block_on(async {
        let token_value = template_state
            .token
            .get()
            .ok_or(IntakeError::MissingToken)?;
        let token = PersonalAccessToken::new(token_value)?;

        let template_content =
            template_state
                .template
                .get()
                .ok_or_else(|| IntakeError::Configuration {
                    message: "template not set".to_owned(),
                })?;

        let gateway = OctocrabReviewCommentGateway::new(&token, locator.api_base().as_str())?;
        let reviews = gateway.list_review_comments(&locator).await?;

        let mut comments: Vec<ExportedComment> =
            reviews.iter().map(ExportedComment::from).collect();
        sort_comments(&mut comments);

        let mut buffer = Vec::new();
        write_template(&mut buffer, &comments, &resolved_url, &template_content)?;

        String::from_utf8(buffer).map_err(|e| IntakeError::Api {
            message: format!("invalid UTF-8 in output: {e}"),
        })
    })
}

fn resolve_mock_server_url(server_url: &str, pr_url: &str) -> String {
    let cleaned_url = pr_url.trim_matches('"');
    if cleaned_url.contains("://SERVER") {
        cleaned_url
            .replace("https://SERVER", server_url)
            .replace("http://SERVER", server_url)
    } else {
        cleaned_url.replace("SERVER", server_url)
    }
}

fn get_output(template_state: &TemplateExportState) -> Result<String, Box<dyn std::error::Error>> {
    template_state
        .output
        .with_ref(Clone::clone)
        .ok_or_else(|| "output missing".into())
}

fn get_error(
    template_state: &TemplateExportState,
) -> Result<IntakeError, Box<dyn std::error::Error>> {
    template_state
        .error
        .with_ref(Clone::clone)
        .ok_or_else(|| "expected error".into())
}

#[then("the output contains {text}")]
fn assert_output_contains(
    template_state: &TemplateExportState,
    text: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = get_output(template_state)?;
    let expected = text.trim_matches('"');
    if !output.contains(expected) {
        return Err(format!("expected output to contain '{expected}', got:\n{output}").into());
    }
    Ok(())
}

#[then("the error indicates invalid template syntax")]
fn assert_invalid_template_error(
    template_state: &TemplateExportState,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = get_error(template_state)?;

    match error {
        IntakeError::Configuration { message } => {
            if !message.contains("invalid template syntax") {
                return Err(
                    format!("expected invalid template syntax error, got: {message}").into(),
                );
            }
            Ok(())
        }
        other => Err(format!("expected Configuration error, got {other:?}").into()),
    }
}

#[scenario(path = "tests/features/template_export.feature", index = 0)]
fn export_with_simple_template(template_state: TemplateExportState) {
    let _ = template_state;
}

#[scenario(path = "tests/features/template_export.feature", index = 1)]
fn template_with_document_variables(template_state: TemplateExportState) {
    let _ = template_state;
}

#[scenario(path = "tests/features/template_export.feature", index = 2)]
fn template_with_file_and_line(template_state: TemplateExportState) {
    let _ = template_state;
}

#[scenario(path = "tests/features/template_export.feature", index = 3)]
fn status_shows_reply_for_threaded(template_state: TemplateExportState) {
    let _ = template_state;
}

#[scenario(path = "tests/features/template_export.feature", index = 4)]
fn status_shows_comment_for_root(template_state: TemplateExportState) {
    let _ = template_state;
}

#[scenario(path = "tests/features/template_export.feature", index = 5)]
fn empty_comments_with_template(template_state: TemplateExportState) {
    let _ = template_state;
}

#[scenario(path = "tests/features/template_export.feature", index = 6)]
fn invalid_template_syntax_error(template_state: TemplateExportState) {
    let _ = template_state;
}
