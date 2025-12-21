//! Then steps for pull request metadata cache behavioural tests.

use rstest_bdd_macros::then;

use crate::pr_metadata_cache_bdd_state::CacheState;
use crate::support::pr_metadata_cache_helpers::{
    assert_api_error_contains, assert_configuration_error_contains,
};

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
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn verify_mocks(cache_state: &CacheState) {
    let runtime = cache_state.runtime.get().expect("runtime not initialised");
    cache_state
        .server
        .with_ref(|server| runtime.block_on(server.verify()))
        .expect("mock server not initialised");
}

#[then(
    "the revalidation request includes If-None-Match {etag} and If-Modified-Since {last_modified}"
)]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_revalidation_request_headers(
    cache_state: &CacheState,
    etag: String,
    last_modified: String,
) {
    let expected_etag = etag.trim_matches('"');
    let expected_last_modified = last_modified.trim_matches('"');

    let runtime = cache_state.runtime.get().expect("runtime not initialised");

    let requests = cache_state
        .server
        .with_ref(|server| runtime.block_on(server.received_requests()))
        .expect("mock server not initialised")
        .expect("request recording is not enabled");

    let expected_path = cache_state
        .expected_metadata_path
        .get()
        .expect("expected metadata request path missing from scenario state");

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
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_uncached_304_error(cache_state: &CacheState) {
    let error = cache_state
        .error
        .with_ref(Clone::clone)
        .expect("expected API error");
    assert_api_error_contains(&error, "unexpected 304 for uncached pull request");
}

#[then("a configuration error mentions running migrations")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn assert_schema_error(cache_state: &CacheState) {
    let error = cache_state
        .error
        .with_ref(Clone::clone)
        .expect("expected configuration error");
    assert_configuration_error_contains(&error, "--migrate-db");
}
