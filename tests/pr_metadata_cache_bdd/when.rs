//! When steps for pull request metadata cache behavioural tests.

use frankie::{
    IntakeError, OctocrabCachingGateway, PersonalAccessToken, PullRequestIntake, PullRequestLocator,
};
use rstest_bdd_macros::when;
use wiremock::MockServer;

use crate::pr_metadata_cache_bdd_state::CacheState;
use crate::support::pr_metadata_cache_helpers::{ensure_runtime_and_server, expected_request_path};

#[when("the cached client loads pull request {pr_url} for the first time")]
#[expect(
    clippy::expect_used,
    reason = "integration test step; allow-expect-in-tests does not cover integration tests"
)]
fn load_pull_request_first_time(cache_state: &CacheState, pr_url: String) {
    let runtime = ensure_runtime_and_server(&cache_state.runtime, &cache_state.server)
        .expect("failed to create Tokio runtime");

    let server_url = cache_state
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

    let locator_error_context = format!("{resolved_url}: locator should parse");
    let locator = PullRequestLocator::parse(&resolved_url).expect(&locator_error_context);

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
            let _had_previous_error = cache_state.error.take().is_some();
            cache_state.details.set(details);
        }
        Err(error) => {
            let _had_previous_details = cache_state.details.take().is_some();
            cache_state.error.set(error);
        }
    }
}

#[when("the cached client loads pull request {pr_url} again")]
fn load_pull_request_again(cache_state: &CacheState, pr_url: String) {
    load_pull_request_first_time(cache_state, pr_url);
}
