//! Behavioural tests for pull request intake.

use frankie::{
    IntakeError, OctocrabGateway, PersonalAccessToken, PullRequestDetails, PullRequestIntake,
    PullRequestLocator,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::runtime::Runtime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Shared runtime wrapper that can be stored in rstest-bdd Slot.
#[derive(Clone)]
struct SharedRuntime(Rc<RefCell<Runtime>>);

impl SharedRuntime {
    fn new(runtime: Runtime) -> Self {
        Self(Rc::new(RefCell::new(runtime)))
    }

    fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.0.borrow().block_on(future)
    }
}

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
fn ensure_runtime_and_server(intake_state: &IntakeState) -> Result<SharedRuntime, IntakeError> {
    if intake_state.runtime.with_ref(|_| ()).is_none() {
        let runtime = Runtime::new().map_err(|error| IntakeError::Io {
            message: format!("failed to create Tokio runtime: {error}"),
        })?;
        intake_state.runtime.set(SharedRuntime::new(runtime));
    }

    let shared_runtime = intake_state.runtime.get().ok_or_else(|| IntakeError::Api {
        message: "runtime not initialised".to_owned(),
    })?;

    if intake_state.server.with_ref(|_| ()).is_none() {
        intake_state
            .server
            .set(shared_runtime.block_on(MockServer::start()));
    }

    Ok(shared_runtime)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd passes owned step arguments"
)]
#[given(
    "a mock GitHub API server with pull request {pr:u64} titled {title} and \
     {count:u64} comments"
)]
fn seed_successful_server(
    intake_state: &IntakeState,
    pr: u64,
    title: String,
    count: u64,
) -> Result<(), IntakeError> {
    let runtime = ensure_runtime_and_server(intake_state)?;

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
        .ok_or_else(|| IntakeError::Api {
            message: "mock server not initialised".to_owned(),
        })
}

#[given("a mock GitHub API server that rejects token for pull request {pr:u64}")]
fn seed_rejecting_server(intake_state: &IntakeState, pr: u64) -> Result<(), IntakeError> {
    let runtime = ensure_runtime_and_server(intake_state)?;

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
        .ok_or_else(|| IntakeError::Api {
            message: "mock server not initialised".to_owned(),
        })
}

#[given("a personal access token {token}")]
fn remember_token(intake_state: &IntakeState, token: String) {
    intake_state.token.set(token);
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd passes owned step arguments"
)]
#[when("the client loads pull request {pr_url}")]
fn load_pull_request(intake_state: &IntakeState, pr_url: String) -> Result<(), IntakeError> {
    let server_url = intake_state
        .server
        .with_ref(MockServer::uri)
        .ok_or_else(|| IntakeError::InvalidUrl("mock server URL missing".to_owned()))?;

    let cleaned_pr_url = pr_url.trim_matches('"');

    let resolved_url = if cleaned_pr_url.contains("://SERVER") {
        cleaned_pr_url
            .replace("https://SERVER", &server_url)
            .replace("http://SERVER", &server_url)
    } else {
        cleaned_pr_url.replace("SERVER", &server_url)
    };
    let locator = PullRequestLocator::parse(&resolved_url)
        .map_err(|error| IntakeError::InvalidUrl(format!("{resolved_url}: {error}")))?;

    let locator_clone = locator.clone();

    let runtime = intake_state.runtime.get().ok_or_else(|| IntakeError::Api {
        message: "runtime not initialised".to_owned(),
    })?;

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

    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd passes owned step arguments"
)]
#[then("the response includes the title {expected}")]
fn assert_title(intake_state: &IntakeState, expected: String) -> Result<(), IntakeError> {
    let expected_title = expected.trim_matches('"');

    let matches = intake_state
        .details
        .with_ref(|details| details.metadata.title.as_deref() == Some(expected_title))
        .unwrap_or(false);

    if matches {
        Ok(())
    } else {
        Err(IntakeError::Api {
            message: format!("missing expected title {expected}"),
        })
    }
}

#[then("the response includes {count:u64} comments")]
fn assert_comment_count(intake_state: &IntakeState, count: u64) -> Result<(), IntakeError> {
    let actual = intake_state
        .details
        .with_ref(|details| details.comments.len() as u64)
        .ok_or_else(|| IntakeError::Api {
            message: "pull request details missing".to_owned(),
        })?;

    if actual == count {
        Ok(())
    } else {
        Err(IntakeError::Api {
            message: format!("expected {count} comments but found {actual}"),
        })
    }
}

#[then("the error message mentions authentication failure")]
fn assert_authentication_error(intake_state: &IntakeState) -> Result<(), IntakeError> {
    let error = intake_state
        .error
        .with_ref(Clone::clone)
        .ok_or_else(|| IntakeError::Api {
            message: "expected authentication error".to_owned(),
        })?;

    if let IntakeError::Authentication { message } = error {
        if message.to_lowercase().contains("rejected")
            || message.to_lowercase().contains("credentials")
        {
            return Ok(());
        }
        return Err(IntakeError::Api {
            message: format!("authentication error did not mention rejection: {message}"),
        });
    }

    Err(IntakeError::Api {
        message: format!("expected Authentication variant, got {error:?}"),
    })
}

#[scenario(path = "tests/features/pr_intake.feature", index = 0)]
fn load_pull_request_success(intake_state: IntakeState) {
    let _ = intake_state;
}

#[scenario(path = "tests/features/pr_intake.feature", index = 1)]
fn load_pull_request_auth_error(intake_state: IntakeState) {
    let _ = intake_state;
}
