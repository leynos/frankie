//! Helpers for pull request metadata cache behavioural tests.

use tempfile::TempDir;
use wiremock::MockServer;

use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Match, Mock, Request, ResponseTemplate};

use frankie::IntakeError;
use rstest_bdd_macros::StepArgs;
use serde_json::json;

use super::runtime::SharedRuntime;
pub use super::runtime::ensure_runtime_and_server;

#[derive(Clone, Debug)]
struct HeaderAbsentMatcher(http::header::HeaderName);

impl Match for HeaderAbsentMatcher {
    fn matches(&self, request: &Request) -> bool {
        request.headers.get(&self.0).is_none()
    }
}

const fn header_absent(key: &'static str) -> HeaderAbsentMatcher {
    HeaderAbsentMatcher(http::header::HeaderName::from_static(key))
}

#[derive(Clone, Debug, StepArgs)]
pub struct MockRevalidationConfig {
    pub pr: u64,
    pub title: String,
    pub etag: String,
    pub last_modified: String,
    pub count: u64,
}

impl MockRevalidationConfig {
    pub fn new(pr: u64, title: String, validators: (String, String), count: u64) -> Self {
        let (etag, last_modified) = validators;
        Self {
            pr,
            title,
            etag: etag.trim_matches('"').to_owned(),
            last_modified: last_modified.trim_matches('"').to_owned(),
            count,
        }
    }

    pub fn normalise(self) -> Self {
        Self::new(
            self.pr,
            self.title,
            (self.etag, self.last_modified),
            self.count,
        )
    }
}

#[derive(Clone, Debug, StepArgs)]
pub struct MockInvalidationConfig {
    pub pr: u64,
    pub old_title: String,
    pub new_title: String,
    pub etag1: String,
    pub etag2: String,
    pub count: u64,
}

impl MockInvalidationConfig {
    pub fn new(
        pr: u64,
        titles: (String, String),
        etag_values: (String, String),
        count: u64,
    ) -> Self {
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

    pub fn normalise(self) -> Self {
        Self::new(
            self.pr,
            (self.old_title, self.new_title),
            (self.etag1, self.etag2),
            self.count,
        )
    }
}

pub fn create_database_path(temp_dir: &TempDir) -> String {
    temp_dir
        .path()
        .join("frankie.sqlite")
        .to_string_lossy()
        .to_string()
}

pub fn expected_request_path(api_base_path: &str, api_path: &str) -> String {
    let trimmed_prefix = api_base_path.trim_end_matches('/');
    let prefix = if trimmed_prefix == "/" {
        ""
    } else {
        trimmed_prefix
    };
    format!("{prefix}{api_path}")
}

pub fn create_pr_body(pr: u64, title: &str) -> serde_json::Value {
    json!({
        "number": pr,
        "title": title,
        "state": "open",
        "html_url": "http://example.invalid",
        "user": { "login": "octocat" }
    })
}

pub fn pull_request_path(pr: u64) -> String {
    format!("/api/v3/repos/owner/repo/pulls/{pr}")
}

pub fn mount_mocks(server: &MockServer, runtime: &SharedRuntime, mocks: Vec<Mock>) {
    for mock in mocks {
        runtime.block_on(mock.mount(server));
    }
}

pub fn build_comments_mock(pr: u64, count: u64, expected_calls: u64) -> Mock {
    let comments: Vec<_> = (0..count)
        .map(|index| {
            json!({
                "id": index + 1,
                "body": format!("comment {index}"),
                "user": { "login": "reviewer" }
            })
        })
        .collect();
    let comments_path = format!("/api/v3/repos/owner/repo/issues/{pr}/comments");

    let mock = Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(comments))
        .expect(expected_calls)
        .named("Issue comments");

    if expected_calls == 0 {
        mock
    } else {
        mock.up_to_n_times(expected_calls)
    }
}

pub fn mount_server_with_revalidation(
    server: &MockServer,
    runtime: &SharedRuntime,
    config: MockRevalidationConfig,
) {
    let normalised = config.normalise();
    let etag = normalised.etag.clone();
    let last_modified = normalised.last_modified.clone();
    let pr_body = create_pr_body(normalised.pr, &normalised.title);
    let pr_path = pull_request_path(normalised.pr);

    let initial = Mock::given(method("GET"))
        .and(path(pr_path.clone()))
        .and(header_absent("if-none-match"))
        .and(header_absent("if-modified-since"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&pr_body)
                .insert_header("ETag", etag.clone())
                .insert_header("Last-Modified", last_modified.clone()),
        )
        .expect(1)
        .named("PR metadata initial fetch");

    let revalidated = Mock::given(method("GET"))
        .and(path(pr_path))
        .and(header_exists("if-none-match"))
        .and(header_exists("if-modified-since"))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .named("PR metadata conditional 304");

    let comments_mock = build_comments_mock(normalised.pr, normalised.count, 2);
    mount_mocks(server, runtime, vec![initial, revalidated, comments_mock]);
}

pub fn mount_server_with_invalidation(
    server: &MockServer,
    runtime: &SharedRuntime,
    config: MockInvalidationConfig,
) {
    let normalised = config.normalise();
    let pr_path = pull_request_path(normalised.pr);

    let old_body = create_pr_body(normalised.pr, &normalised.old_title);
    let new_body = create_pr_body(normalised.pr, &normalised.new_title);

    let initial = Mock::given(method("GET"))
        .and(path(pr_path.clone()))
        .and(header_absent("if-none-match"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&old_body)
                .insert_header("ETag", normalised.etag1.clone()),
        )
        .expect(1)
        .named("PR metadata initial fetch (etag-1)");

    let changed = Mock::given(method("GET"))
        .and(path(pr_path))
        .and(header("if-none-match", normalised.etag1.clone()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&new_body)
                .insert_header("ETag", normalised.etag2),
        )
        .expect(1)
        .named("PR metadata refresh with new ETag");

    let comments_mock = build_comments_mock(normalised.pr, normalised.count, 2);
    mount_mocks(server, runtime, vec![initial, changed, comments_mock]);
}

pub fn assert_api_error_contains(error: &IntakeError, message_fragment: &str) {
    let IntakeError::Api { message } = error else {
        panic!("expected Api variant, got {error:?}");
    };
    assert!(
        message.contains(message_fragment),
        "unexpected error message: {message}"
    );
}

pub fn assert_configuration_error_contains(error: &IntakeError, message_fragment: &str) {
    let IntakeError::Configuration { message } = error else {
        panic!("expected Configuration variant, got {error:?}");
    };
    assert!(
        message.contains(message_fragment),
        "expected error message to mention {message_fragment}, got: {message}"
    );
}
