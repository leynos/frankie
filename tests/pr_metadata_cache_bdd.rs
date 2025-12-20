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
use rstest_bdd_macros::{ScenarioState, StepArgs, given, scenario, then, when};
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;
use tempfile::TempDir;
use tokio::runtime::Runtime;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use support::create_temp_dir;

#[derive(Clone, Debug)]
struct MockPrMetadata {
    pr: u64,
    title: String,
    etag: String,
    last_modified: String,
}

impl MockPrMetadata {
    const fn new(pr: u64, title: String, etag: String, last_modified: String) -> Self {
        Self {
            pr,
            title,
            etag,
            last_modified,
        }
    }
}

#[derive(Clone, Debug)]
struct MockPrInvalidation {
    pr: u64,
    old_title: String,
    new_title: String,
    etag1: String,
    etag2: String,
}

impl MockPrInvalidation {
    fn new(pr: u64, titles: (String, String), etags: (String, String)) -> Self {
        let (old_title, new_title) = titles;
        let (etag_one, etag_two) = etags;
        Self {
            pr,
            old_title,
            new_title,
            etag1: etag_one,
            etag2: etag_two,
        }
    }
}

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

#[derive(Clone, Debug, StepArgs)]
struct MockInvalidationConfig {
    pr: u64,
    old_title: String,
    new_title: String,
    etag1: String,
    etag2: String,
    count: u64,
}

impl MockInvalidationConfig {
    fn new(pr: u64, titles: (String, String), etag_values: (String, String), count: u64) -> Self {
        let (old_title, new_title) = titles;
        let (etag_one, etag_two) = etag_values;
        Self {
            pr,
            old_title,
            new_title,
            etag1: etag_one.trim_matches('"').to_owned(),
            etag2: etag_two.trim_matches('"').to_owned(),
            count,
        }
    }
}

#[derive(Clone, Debug, StepArgs)]
struct MockRevalidationConfig {
    pr: u64,
    title: String,
    etag: String,
    last_modified: String,
    count: u64,
}

impl MockRevalidationConfig {
    fn new(pr: u64, title: String, validators: (String, String), count: u64) -> Self {
        let (etag, last_modified) = validators;
        Self {
            pr,
            title,
            etag: etag.trim_matches('"').to_owned(),
            last_modified: last_modified.trim_matches('"').to_owned(),
            count,
        }
    }
}

#[derive(ScenarioState, Default)]
struct CacheState {
    runtime: Slot<SharedRuntime>,
    server: Slot<MockServer>,
    token: Slot<String>,
    database_url: Slot<String>,
    temp_dir: Slot<TempDir>,
    ttl_seconds: Slot<u64>,
    expected_metadata_path: Slot<String>,
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

fn expected_request_path(api_base_path: &str, api_path: &str) -> String {
    let trimmed_prefix = api_base_path.trim_end_matches('/');
    let prefix = if trimmed_prefix == "/" {
        ""
    } else {
        trimmed_prefix
    };
    format!("{prefix}{api_path}")
}

fn create_mock_comments(count: u64) -> Vec<serde_json::Value> {
    (0..count)
        .map(|index| {
            json!({
                "id": index + 1,
                "body": format!("comment {index}"),
                "user": { "login": "reviewer" }
            })
        })
        .collect()
}

fn create_pr_body(pr: u64, title: &str) -> serde_json::Value {
    json!({
        "number": pr,
        "title": title,
        "state": "open",
        "html_url": "http://example.invalid",
        "user": { "login": "octocat" }
    })
}

fn mount_mocks(cache_state: &CacheState, runtime: &SharedRuntime, mocks: Vec<Mock>) {
    cache_state
        .server
        .with_ref(|server| {
            for mock in mocks {
                runtime.block_on(mock.mount(server));
            }
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

fn build_comments_mock(pr: u64, count: u64, expected_calls: u64) -> Mock {
    let comments = create_mock_comments(count);
    let comments_path = format!("/api/v3/repos/owner/repo/issues/{pr}/comments");

    Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(comments))
        .up_to_n_times(expected_calls)
        .expect(expected_calls)
        .named("Issue comments (two loads)")
}

fn assert_error_variant_contains(
    cache_state: &CacheState,
    variant_name: &str,
    message_fragment: &str,
) {
    match variant_name {
        "Api" => {
            let error = cache_state
                .error
                .with_ref(Clone::clone)
                .unwrap_or_else(|| panic!("expected API error"));

            let IntakeError::Api { message } = error else {
                panic!("expected Api variant, got {error:?}");
            };

            assert!(
                message.contains(message_fragment),
                "unexpected error message: {message}"
            );
        }
        "Configuration" => {
            let error = cache_state
                .error
                .with_ref(Clone::clone)
                .unwrap_or_else(|| panic!("expected configuration error"));

            let IntakeError::Configuration { message } = error else {
                panic!("expected Configuration variant, got {error:?}");
            };

            assert!(
                message.contains(message_fragment),
                "expected error message to mention {message_fragment}, got: {message}"
            );
        }
        _ => panic!("unknown error variant name: {variant_name}"),
    }
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

    let comments = create_mock_comments(count);
    let pr_body = create_pr_body(pr, &title);

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
fn mock_server_with_revalidation(
    #[step_args] config: MockRevalidationConfig,
    cache_state: &CacheState,
) {
    let MockRevalidationConfig {
        pr,
        title,
        etag,
        last_modified,
        count,
    } = config;

    let normalised_config = MockRevalidationConfig::new(pr, title, (etag, last_modified), count);
    let metadata = MockPrMetadata::new(
        normalised_config.pr,
        normalised_config.title,
        normalised_config.etag,
        normalised_config.last_modified,
    );
    mock_server_with_revalidation_impl(cache_state, metadata, normalised_config.count);
}

fn mock_server_with_revalidation_impl(
    cache_state: &CacheState,
    metadata: MockPrMetadata,
    count: u64,
) {
    let MockPrMetadata {
        pr,
        title,
        etag,
        last_modified,
    } = metadata;

    let runtime = ensure_runtime_and_server(cache_state);

    let pr_body = create_pr_body(pr, &title);

    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{pr}");

    let initial = Mock::given(method("GET"))
        .and(path(pr_path.clone()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&pr_body)
                .insert_header("ETag", etag)
                .insert_header("Last-Modified", last_modified),
        )
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata initial fetch");

    let revalidated = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .named("PR metadata conditional 304");

    let comments_mock = build_comments_mock(pr, count, 2);
    mount_mocks(
        cache_state,
        &runtime,
        vec![initial, revalidated, comments_mock],
    );
}

#[given(
    "a mock GitHub API server that updates pull request {pr:u64} from title {old_title} to title {new_title} with ETag {etag1} then {etag2} and {count:u64} comments"
)]
fn mock_server_with_invalidation(
    #[step_args] config: MockInvalidationConfig,
    cache_state: &CacheState,
) {
    let MockInvalidationConfig {
        pr,
        old_title,
        new_title,
        etag1,
        etag2,
        count,
    } = config;

    let normalised_config =
        MockInvalidationConfig::new(pr, (old_title, new_title), (etag1, etag2), count);
    let invalidation = MockPrInvalidation::new(
        normalised_config.pr,
        (normalised_config.old_title, normalised_config.new_title),
        (normalised_config.etag1, normalised_config.etag2),
    );
    mock_server_with_invalidation_impl(cache_state, invalidation, normalised_config.count);
}

fn mock_server_with_invalidation_impl(
    cache_state: &CacheState,
    invalidation: MockPrInvalidation,
    count: u64,
) {
    let MockPrInvalidation {
        pr,
        old_title,
        new_title,
        etag1,
        etag2,
    } = invalidation;

    let runtime = ensure_runtime_and_server(cache_state);

    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{pr}");

    let old_body = create_pr_body(pr, &old_title);
    let new_body = create_pr_body(pr, &new_title);

    let initial = Mock::given(method("GET"))
        .and(path(pr_path.clone()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&old_body)
                .insert_header("ETag", etag1.clone()),
        )
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata initial fetch (etag-1)");

    let changed = Mock::given(method("GET"))
        .and(path(pr_path))
        .and(header("if-none-match", etag1))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&new_body)
                .insert_header("ETag", etag2),
        )
        .expect(1)
        .named("PR metadata refresh with new ETag");

    let comments_mock = build_comments_mock(pr, count, 2);
    mount_mocks(cache_state, &runtime, vec![initial, changed, comments_mock]);
}

#[given(
    "a mock GitHub API server that returns 304 Not Modified for pull request {pr:u64} with {count:u64} comments"
)]
fn mock_server_uncached_not_modified(cache_state: &CacheState, pr: u64, count: u64) {
    let runtime = ensure_runtime_and_server(cache_state);
    let _ = count;

    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{pr}");

    let metadata_mock = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(ResponseTemplate::new(304))
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata unexpected 304");

    mount_mocks(cache_state, &runtime, vec![metadata_mock]);
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

    let pull_request_path = format!(
        "/repos/{}/{}/pulls/{}",
        locator.owner().as_str(),
        locator.repository().as_str(),
        locator.number().get()
    );
    let metadata_path = expected_request_path(locator.api_base().path(), &pull_request_path);
    cache_state.expected_metadata_path.set(metadata_path);

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

    let expected_path = cache_state
        .expected_metadata_path
        .get()
        .unwrap_or_else(|| panic!("expected metadata request path missing from scenario state"));

    let metadata_requests: Vec<_> = requests
        .into_iter()
        .filter(|request| {
            request.method.as_str() == "GET" && request.url.path() == expected_path.as_str()
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

#[then("an API error mentions an unexpected 304 response")]
fn assert_uncached_304_error(cache_state: &CacheState) {
    assert_error_variant_contains(
        cache_state,
        "Api",
        "unexpected 304 for uncached pull request",
    );
}

#[then("a configuration error mentions running migrations")]
fn assert_schema_error(cache_state: &CacheState) {
    assert_error_variant_contains(cache_state, "Configuration", "--migrate-db");
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

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 4)]
fn uncached_not_modified_returns_error(cache_state: CacheState) {
    let _ = cache_state;
}
