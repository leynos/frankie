//! Behavioural tests for pull request metadata caching.

mod support;

use frankie::persistence::migrate_database;
use frankie::telemetry::{TelemetryEvent, TelemetrySink};
use frankie::{
    IntakeError, OctocrabCachingGateway, PersonalAccessToken, PullRequestDetails,
    PullRequestIntake, PullRequestLocator,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;
use tempfile::TempDir;
use tokio::runtime::Runtime;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use support::create_temp_dir;

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

#[derive(Debug, Default)]
struct NoopTelemetry;

impl TelemetrySink for NoopTelemetry {
    fn record(&self, _event: TelemetryEvent) {}
}

#[derive(ScenarioState, Default)]
struct CacheState {
    runtime: Slot<SharedRuntime>,
    server: Slot<MockServer>,
    token: Slot<String>,
    database_url: Slot<String>,
    temp_dir: Slot<TempDir>,
    ttl_seconds: Slot<u64>,
    details: Slot<PullRequestDetails>,
    error: Slot<IntakeError>,
}

#[fixture]
fn cache_state() -> CacheState {
    CacheState::default()
}

fn ensure_runtime_and_server(state: &CacheState) -> SharedRuntime {
    if state.runtime.with_ref(|_| ()).is_none() {
        let runtime = Runtime::new()
            .unwrap_or_else(|error| panic!("failed to create Tokio runtime: {error}"));
        state.runtime.set(SharedRuntime::new(runtime));
    }

    let shared_runtime = state
        .runtime
        .get()
        .unwrap_or_else(|| panic!("runtime not initialised after set"));

    if state.server.with_ref(|_| ()).is_none() {
        state
            .server
            .set(shared_runtime.block_on(MockServer::start()));
    }

    shared_runtime
}

fn create_database_path(temp_dir: &TempDir) -> String {
    temp_dir
        .path()
        .join("frankie.sqlite")
        .to_string_lossy()
        .to_string()
}

// --- Given steps ---

#[given("a temporary database file with migrations applied")]
fn migrated_database(cache_state: &CacheState) {
    let temp_dir = create_temp_dir();
    let database_url = create_database_path(&temp_dir);

    migrate_database(&database_url, &NoopTelemetry)
        .unwrap_or_else(|error| panic!("migrations should run: {error}"));

    cache_state.temp_dir.set(temp_dir);
    cache_state.database_url.set(database_url);
}

#[given("a temporary database file without migrations")]
fn unmigrated_database(cache_state: &CacheState) {
    let temp_dir = create_temp_dir();
    let database_url = create_database_path(&temp_dir);

    cache_state.temp_dir.set(temp_dir);
    cache_state.database_url.set(database_url);
}

#[given("a cache TTL of {ttl:u64} seconds")]
fn cache_ttl(cache_state: &CacheState, ttl: u64) {
    cache_state.ttl_seconds.set(ttl);
}

#[given(
    "a mock GitHub API server that serves pull request {pr:u64} titled {title} without validators with {count:u64} comments"
)]
fn mock_server_simple(cache_state: &CacheState, pr: u64, title: String, count: u64) {
    let runtime = ensure_runtime_and_server(cache_state);

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
        .respond_with(ResponseTemplate::new(200).set_body_json(&pr_body))
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata (fresh cache)");

    let comments_mock = Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&comments))
        .up_to_n_times(2)
        .expect(2)
        .named("Issue comments (two loads)");

    cache_state
        .server
        .with_ref(|server| {
            runtime.block_on(pr_mock.mount(server));
            runtime.block_on(comments_mock.mount(server));
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

#[given(
    "a mock GitHub API server that serves pull request {pr:u64} titled {title} with ETag {etag} and Last-Modified {last_modified} with {count:u64} comments"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "rstest-bdd step signatures follow feature parameters"
)]
fn mock_server_with_revalidation(
    cache_state: &CacheState,
    pr: u64,
    title: String,
    etag: String,
    last_modified: String,
    count: u64,
) {
    let runtime = ensure_runtime_and_server(cache_state);

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

    let etag_clean = etag.trim_matches('"').to_owned();
    let last_modified_clean = last_modified.trim_matches('"').to_owned();

    let initial = Mock::given(method("GET"))
        .and(path(pr_path.clone()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&pr_body)
                .insert_header("ETag", etag_clean.clone())
                .insert_header("Last-Modified", last_modified_clean.clone()),
        )
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata initial fetch");

    let revalidated = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .named("PR metadata conditional 304");

    let comments_mock = Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&comments))
        .up_to_n_times(2)
        .expect(2)
        .named("Issue comments (two loads)");

    cache_state
        .server
        .with_ref(|server| {
            runtime.block_on(initial.mount(server));
            runtime.block_on(revalidated.mount(server));
            runtime.block_on(comments_mock.mount(server));
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

#[given(
    "a mock GitHub API server that updates pull request {pr:u64} from title {old_title} to title {new_title} with ETag {etag1} then {etag2} and {count:u64} comments"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "rstest-bdd step signatures follow feature parameters"
)]
fn mock_server_with_invalidation(
    cache_state: &CacheState,
    pr: u64,
    old_title: String,
    new_title: String,
    etag1: String,
    etag2: String,
    count: u64,
) {
    let runtime = ensure_runtime_and_server(cache_state);

    let comments: Vec<_> = (0..count)
        .map(|index| {
            json!({
                "id": index + 1,
                "body": format!("comment {index}"),
                "user": { "login": "reviewer" }
            })
        })
        .collect();

    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{pr}");
    let comments_path = format!("/api/v3/repos/owner/repo/issues/{pr}/comments");

    let etag1_clean = etag1.trim_matches('"').to_owned();
    let etag2_clean = etag2.trim_matches('"').to_owned();

    let old_body = json!({
        "number": pr,
        "title": old_title,
        "state": "open",
        "html_url": "http://example.invalid",
        "user": { "login": "octocat" }
    });

    let new_body = json!({
        "number": pr,
        "title": new_title,
        "state": "open",
        "html_url": "http://example.invalid",
        "user": { "login": "octocat" }
    });

    let initial = Mock::given(method("GET"))
        .and(path(pr_path.clone()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&old_body)
                .insert_header("ETag", etag1_clean.clone()),
        )
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata initial fetch (etag-1)");

    let changed = Mock::given(method("GET"))
        .and(path(pr_path))
        .and(header("if-none-match", etag1_clean))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&new_body)
                .insert_header("ETag", etag2_clean),
        )
        .expect(1)
        .named("PR metadata refresh with new ETag");

    let comments_mock = Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&comments))
        .up_to_n_times(2)
        .expect(2)
        .named("Issue comments (two loads)");

    cache_state
        .server
        .with_ref(|server| {
            runtime.block_on(initial.mount(server));
            runtime.block_on(changed.mount(server));
            runtime.block_on(comments_mock.mount(server));
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

#[given("a personal access token {token}")]
fn remember_token(cache_state: &CacheState, token: String) {
    cache_state.token.set(token);
}

// --- When steps ---

#[when("the cached client loads pull request {pr_url} for the first time")]
fn load_pull_request_first_time(cache_state: &CacheState, pr_url: String) {
    let runtime = ensure_runtime_and_server(cache_state);

    let server_url = cache_state
        .server
        .with_ref(MockServer::uri)
        .unwrap_or_else(|| panic!("mock server URL missing"));

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

    let ttl_seconds = cache_state
        .ttl_seconds
        .with_ref(|value| *value)
        .unwrap_or(86_400);

    let result = runtime.block_on(async {
        let token_value = cache_state.token.get().ok_or(IntakeError::MissingToken)?;
        let token = PersonalAccessToken::new(token_value)?;

        let database_url =
            cache_state
                .database_url
                .get()
                .ok_or_else(|| IntakeError::Configuration {
                    message: "database URL missing from test state".to_owned(),
                })?;

        let gateway =
            OctocrabCachingGateway::for_token(&token, &locator, &database_url, ttl_seconds)?;
        let intake = PullRequestIntake::new(&gateway);
        intake.load(&locator).await
    });

    match result {
        Ok(details) => {
            drop(cache_state.error.take());
            cache_state.details.set(details);
        }
        Err(error) => {
            drop(cache_state.details.take());
            cache_state.error.set(error);
        }
    }
}

#[when("the cached client loads pull request {pr_url} again")]
fn load_pull_request_again(cache_state: &CacheState, pr_url: String) {
    load_pull_request_first_time(cache_state, pr_url);
}

// --- Then steps ---

#[then("the response includes the title {expected}")]
fn assert_title(cache_state: &CacheState, expected: String) {
    let expected_title = expected.trim_matches('"');

    let maybe_details = cache_state.details.with_ref(Clone::clone);

    let Some(details) = maybe_details else {
        let error = cache_state.error.with_ref(Clone::clone);
        panic!("pull request details missing; last error: {error:?}");
    };

    let actual = details
        .metadata
        .title
        .as_deref()
        .unwrap_or("<missing title>");

    assert_eq!(actual, expected_title, "unexpected title");
}

#[then("the GitHub API mocks are satisfied")]
fn verify_mocks(cache_state: &CacheState) {
    let runtime = cache_state
        .runtime
        .get()
        .unwrap_or_else(|| panic!("runtime not initialised"));
    cache_state
        .server
        .with_ref(|server| runtime.block_on(server.verify()))
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

#[then(
    "the revalidation request includes If-None-Match {etag} and If-Modified-Since {last_modified}"
)]
fn assert_revalidation_request_headers(
    cache_state: &CacheState,
    etag: String,
    last_modified: String,
) {
    let expected_etag = etag.trim_matches('"');
    let expected_last_modified = last_modified.trim_matches('"');

    let runtime = cache_state
        .runtime
        .get()
        .unwrap_or_else(|| panic!("runtime not initialised"));

    let requests = cache_state
        .server
        .with_ref(|server| runtime.block_on(server.received_requests()))
        .unwrap_or_else(|| panic!("mock server not initialised"))
        .unwrap_or_else(|| panic!("request recording is not enabled"));

    let metadata_requests: Vec<_> = requests
        .into_iter()
        .filter(|request| {
            request.method.as_str() == "GET"
                && request.url.path() == "/api/v3/repos/owner/repo/pulls/7"
        })
        .collect();

    let Some(second_request) = metadata_requests.get(1) else {
        panic!(
            "expected two metadata requests, got {}",
            metadata_requests.len()
        );
    };

    let actual_etag = second_request
        .headers
        .get("if-none-match")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing if-none-match>");
    let actual_last_modified = second_request
        .headers
        .get("if-modified-since")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing if-modified-since>");

    assert_eq!(
        actual_etag, expected_etag,
        "unexpected if-none-match header"
    );
    assert_eq!(
        actual_last_modified, expected_last_modified,
        "unexpected if-modified-since header"
    );
}

#[then("a configuration error mentions running migrations")]
fn assert_schema_error(cache_state: &CacheState) {
    let error = cache_state
        .error
        .with_ref(Clone::clone)
        .unwrap_or_else(|| panic!("expected configuration error"));

    let IntakeError::Configuration { message } = error else {
        panic!("expected Configuration variant, got {error:?}");
    };

    assert!(
        message.contains("--migrate-db"),
        "expected error message to mention --migrate-db, got: {message}"
    );
}

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 0)]
fn fresh_cache_avoids_refetch(cache_state: CacheState) {
    let _ = cache_state;
}

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 1)]
fn expired_cache_revalidates(cache_state: CacheState) {
    let _ = cache_state;
}

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 2)]
fn changed_etag_invalidates(cache_state: CacheState) {
    let _ = cache_state;
}

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 3)]
fn cache_requires_schema(cache_state: CacheState) {
    let _ = cache_state;
}
