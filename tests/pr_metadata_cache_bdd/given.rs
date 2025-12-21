//! Given steps for pull request metadata cache behavioural tests.

use frankie::persistence::migrate_database;
use frankie::telemetry::NoopTelemetrySink;
use rstest_bdd_macros::given;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::pr_metadata_cache_bdd_state::{
    CacheState, MockInvalidationConfig, MockRevalidationConfig,
};
use crate::pr_metadata_cache_helpers::{
    build_comments_mock, create_database_path, create_pr_body, ensure_runtime_and_server,
    mount_mocks, mount_server_with_invalidation, mount_server_with_revalidation, pull_request_path,
};
use crate::support::create_temp_dir;

#[given("a temporary database file with migrations applied")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn migrated_database(cache_state: &CacheState) {
    let temp_dir = create_temp_dir().expect("failed to create temporary directory");
    let database_url = create_database_path(&temp_dir);

    migrate_database(&database_url, &NoopTelemetrySink).expect("migrations should run");

    cache_state.temp_dir.set(temp_dir);
    cache_state.database_url.set(database_url);
}

#[given("a temporary database file without migrations")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn unmigrated_database(cache_state: &CacheState) {
    let temp_dir = create_temp_dir().expect("failed to create temporary directory");
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
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn mock_server_simple(cache_state: &CacheState, pr: u64, title: String, count: u64) {
    let runtime = ensure_runtime_and_server(&cache_state.runtime, &cache_state.server)
        .expect("failed to create Tokio runtime");

    let pr_body = create_pr_body(pr, &title);
    let pr_path = pull_request_path(pr);

    let pr_mock = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&pr_body))
        .expect(1)
        .named("PR metadata (fresh cache)");

    let comments_mock = build_comments_mock(pr, count, 2);

    cache_state
        .server
        .with_ref(|server| mount_mocks(server, &runtime, vec![pr_mock, comments_mock]))
        .expect("mock server not initialised");
}

#[given(
    "a mock GitHub API server that serves pull request {pr:u64} titled {title} with ETag {etag} and Last-Modified {last_modified} with {count:u64} comments"
)]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn mock_server_with_revalidation(
    #[step_args] config: MockRevalidationConfig,
    cache_state: &CacheState,
) {
    let runtime = ensure_runtime_and_server(&cache_state.runtime, &cache_state.server)
        .expect("failed to create Tokio runtime");

    cache_state
        .server
        .with_ref(|server| {
            mount_server_with_revalidation(server, &runtime, config);
        })
        .expect("mock server not initialised");
}

#[given(
    "a mock GitHub API server that updates pull request {pr:u64} from title {old_title} to title {new_title} with ETag {etag1} then {etag2} and {count:u64} comments"
)]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn mock_server_with_invalidation(
    #[step_args] config: MockInvalidationConfig,
    cache_state: &CacheState,
) {
    let runtime = ensure_runtime_and_server(&cache_state.runtime, &cache_state.server)
        .expect("failed to create Tokio runtime");

    cache_state
        .server
        .with_ref(|server| {
            mount_server_with_invalidation(server, &runtime, config);
        })
        .expect("mock server not initialised");
}

#[given(
    "a mock GitHub API server that returns 304 Not Modified for pull request {pr:u64} with {count:u64} comments"
)]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn mock_server_uncached_not_modified(cache_state: &CacheState, pr: u64, count: u64) {
    let runtime = ensure_runtime_and_server(&cache_state.runtime, &cache_state.server)
        .expect("failed to create Tokio runtime");

    let pr_path = pull_request_path(pr);
    let metadata_mock = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .named("PR metadata unexpected 304");

    let comments_mock = build_comments_mock(pr, count, 0);

    cache_state
        .server
        .with_ref(|server| mount_mocks(server, &runtime, vec![metadata_mock, comments_mock]))
        .expect("mock server not initialised");
}

#[given("a personal access token {token}")]
fn remember_token(cache_state: &CacheState, token: String) {
    cache_state.token.set(token);
}
