//! Behavioural tests for pull request intake.

use frankie::{
    IntakeError, OctocrabGateway, PersonalAccessToken, PullRequestDetails, PullRequestIntake,
    PullRequestLocator,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use serde_json::json;
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[path = "support/runtime.rs"]
mod runtime;

use runtime::SharedRuntime;

#[derive(ScenarioState, Default)]
struct IntakeState {
    runtime: Slot<SharedRuntime>,
    server: Slot<MockServer>,
    token: Slot<String>,
    details: Slot<PullRequestDetails>,
    error: Slot<IntakeError>,
}

#[fixture]
fn intake_state() -> IntakeState {
    IntakeState::default()
}

/// Ensures the runtime and server are initialised in `IntakeState`.
///
/// # Errors
///
/// Returns an error if the Tokio runtime cannot be created.
fn ensure_runtime_and_server(intake_state: &IntakeState) -> Result<SharedRuntime, std::io::Error> {
    if intake_state.runtime.with_ref(|_| ()).is_none() {
        let runtime = Runtime::new()?;
        intake_state.runtime.set(SharedRuntime::new(runtime));
    }

    let shared_runtime = intake_state
        .runtime
        .get()
        .ok_or_else(|| std::io::Error::other("runtime not initialised after set"))?;

    if intake_state.server.with_ref(|_| ()).is_none() {
        intake_state
            .server
            .set(shared_runtime.block_on(MockServer::start()));
    }

    Ok(shared_runtime)
}

#[given(
    "a mock GitHub API server with pull request {pr:u64} titled {title} and \
     {count:u64} comments"
)]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn seed_successful_server(intake_state: &IntakeState, pr: u64, title: String, count: u64) {
    let runtime = ensure_runtime_and_server(intake_state)
        .unwrap_or_else(|error| panic!("failed to create Tokio runtime: {error}"));

    let comments: Vec<_> = (0..count)
        .map(|index| {
            json!({
                "id": index + 1,
                "body": format!("comment {index}"),
                "user": { "login": "reviewer" }
            })
        })
        .collect();

    let pr_body = json!({
        "number": pr,
        "title": title,
        "state": "open",
        "html_url": "http://example.invalid",
        "user": { "login": "octocat" }
    });

    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{pr}");
    let comments_path = format!("/api/v3/repos/owner/repo/issues/{pr}/comments");

    let pr_mock = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&pr_body));

    let comments_mock = Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&comments));

    intake_state
        .server
        .with_ref(|server| {
            runtime.block_on(pr_mock.mount(server));
            runtime.block_on(comments_mock.mount(server));
        })
        .expect("mock server not initialised");
}

#[given("a mock GitHub API server that rejects token for pull request {pr:u64}")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn seed_rejecting_server(intake_state: &IntakeState, pr: u64) {
    let runtime = ensure_runtime_and_server(intake_state)
        .unwrap_or_else(|error| panic!("failed to create Tokio runtime: {error}"));

    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{pr}");
    let response =
        ResponseTemplate::new(401).set_body_json(json!({ "message": "Bad credentials" }));

    let mock = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(response);

    intake_state
        .server
        .with_ref(|server| {
            runtime.block_on(mock.mount(server));
        })
        .expect("mock server not initialised");
}

#[given("a personal access token {token}")]
fn remember_token(intake_state: &IntakeState, token: String) {
    intake_state.token.set(token);
}

#[when("the client loads pull request {pr_url}")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn load_pull_request(intake_state: &IntakeState, pr_url: String) {
    let server_url = intake_state
        .server
        .with_ref(MockServer::uri)
        .expect("mock server URL missing");

    let cleaned_pr_url = pr_url.trim_matches('"');

    let resolved_url = if cleaned_pr_url.contains("://SERVER") {
        cleaned_pr_url
            .replace("https://SERVER", &server_url)
            .replace("http://SERVER", &server_url)
    } else {
        cleaned_pr_url.replace("SERVER", &server_url)
    };
    let locator = PullRequestLocator::parse(&resolved_url)
        .unwrap_or_else(|error| panic!("{resolved_url}: {error}"));

    let locator_clone = locator.clone();

    let runtime = intake_state.runtime.get().expect("runtime not initialised");

    let result = runtime.block_on(async {
        let token_value = intake_state.token.get().ok_or(IntakeError::MissingToken)?;
        let token = PersonalAccessToken::new(token_value)?;

        let gateway = OctocrabGateway::for_token(&token, &locator_clone)?;
        let intake = PullRequestIntake::new(&gateway);
        intake.load(&locator_clone).await
    });

    match result {
        Ok(details) => {
            drop(intake_state.error.take());
            intake_state.details.set(details);
        }
        Err(error) => {
            drop(intake_state.details.take());
            intake_state.error.set(error);
        }
    }
}

#[then("the response includes the title {expected}")]
fn assert_title(intake_state: &IntakeState, expected: String) {
    let expected_title = expected.trim_matches('"');

    let matches = intake_state
        .details
        .with_ref(|details| details.metadata.title.as_deref() == Some(expected_title))
        .unwrap_or(false);

    assert!(matches, "expected title {expected_title:?} not found");
}

#[then("the response includes {count:u64} comments")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_comment_count(intake_state: &IntakeState, count: u64) {
    let actual = intake_state
        .details
        .with_ref(|details| details.comments.len() as u64)
        .expect("pull request details missing");

    assert_eq!(actual, count, "comment count mismatch");
}

#[then("the error message mentions authentication failure")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_authentication_error(intake_state: &IntakeState) {
    let error = intake_state
        .error
        .with_ref(Clone::clone)
        .expect("expected authentication error");

    let IntakeError::Authentication { message } = error else {
        panic!("expected Authentication variant, got {error:?}");
    };

    assert!(
        message.to_lowercase().contains("rejected")
            || message.to_lowercase().contains("credentials"),
        "authentication error did not mention rejection: {message}"
    );
}

#[scenario(path = "tests/features/pr_intake.feature", index = 0)]
fn load_pull_request_success(intake_state: IntakeState) {
    let _ = intake_state;
}

#[scenario(path = "tests/features/pr_intake.feature", index = 1)]
fn load_pull_request_auth_error(intake_state: IntakeState) {
    let _ = intake_state;
}
