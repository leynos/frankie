//! Given steps for pull request metadata cache behavioural tests.

use frankie::persistence::migrate_database;
use rstest_bdd_macros::given;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::pr_metadata_cache_bdd_state::{
    CacheState, MockInvalidationConfig, MockRevalidationConfig, NoopTelemetry,
    ensure_runtime_and_server,
};
use crate::support::create_temp_dir;
use crate::support::pr_metadata_cache_helpers::{
    InvalidationMocks, RevalidationMocks, create_database_path, create_mock_comments,
    create_pr_body, mount_mocks, mount_server_with_invalidation, mount_server_with_revalidation,
};

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
    let shared_runtime = ensure_runtime_and_server(cache_state);
    let runtime = shared_runtime.borrow();

    let pr_body = create_pr_body(pr, &title);
    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{pr}");

    let pr_mock = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&pr_body))
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata (fresh cache)");

    let comments = create_mock_comments(count);
    let comments_path = format!("/api/v3/repos/owner/repo/issues/{pr}/comments");
    let comments_mock = Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&comments))
        .up_to_n_times(2)
        .expect(2)
        .named("Issue comments (two loads)");

    cache_state
        .server
        .with_ref(|server| mount_mocks(server, &runtime, vec![pr_mock, comments_mock]))
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

    let normalised = MockRevalidationConfig::new(pr, title, (etag, last_modified), count);

    let shared_runtime = ensure_runtime_and_server(cache_state);
    let runtime = shared_runtime.borrow();

    cache_state
        .server
        .with_ref(|server| {
            mount_server_with_revalidation(
                server,
                &runtime,
                RevalidationMocks {
                    pr: normalised.pr,
                    title: normalised.title,
                    etag: normalised.etag,
                    last_modified: normalised.last_modified,
                    comment_count: normalised.count,
                },
            );
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));
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

    let normalised = MockInvalidationConfig::new(pr, (old_title, new_title), (etag1, etag2), count);

    let shared_runtime = ensure_runtime_and_server(cache_state);
    let runtime = shared_runtime.borrow();

    cache_state
        .server
        .with_ref(|server| {
            mount_server_with_invalidation(
                server,
                &runtime,
                InvalidationMocks {
                    pr: normalised.pr,
                    old_title: normalised.old_title,
                    new_title: normalised.new_title,
                    etag1: normalised.etag1,
                    etag2: normalised.etag2,
                    comment_count: normalised.count,
                },
            );
        })
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

#[given(
    "a mock GitHub API server that returns 304 Not Modified for pull request {pr:u64} with {count:u64} comments"
)]
fn mock_server_uncached_not_modified(cache_state: &CacheState, pr: u64, count: u64) {
    let shared_runtime = ensure_runtime_and_server(cache_state);
    let runtime = shared_runtime.borrow();
    let _ = count;

    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{pr}");
    let metadata_mock = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(ResponseTemplate::new(304))
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata unexpected 304");

    cache_state
        .server
        .with_ref(|server| mount_mocks(server, &runtime, vec![metadata_mock]))
        .unwrap_or_else(|| panic!("mock server not initialised"));
}

#[given("a personal access token {token}")]
fn remember_token(cache_state: &CacheState, token: String) {
    cache_state.token.set(token);
}
