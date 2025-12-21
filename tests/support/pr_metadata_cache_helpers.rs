//! Helpers for pull request metadata cache behavioural tests.

use tempfile::TempDir;
use tokio::runtime::Runtime;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use frankie::IntakeError;
use serde_json::json;

#[derive(Clone, Debug)]
pub struct RevalidationMocks {
    pub pr: u64,
    pub title: String,
    pub etag: String,
    pub last_modified: String,
    pub comment_count: u64,
}

#[derive(Clone, Debug)]
pub struct InvalidationMocks {
    pub pr: u64,
    pub old_title: String,
    pub new_title: String,
    pub etag1: String,
    pub etag2: String,
    pub comment_count: u64,
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

pub fn create_mock_comments(count: u64) -> Vec<serde_json::Value> {
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

pub fn create_pr_body(pr: u64, title: &str) -> serde_json::Value {
    json!({
        "number": pr,
        "title": title,
        "state": "open",
        "html_url": "http://example.invalid",
        "user": { "login": "octocat" }
    })
}

pub fn mount_mocks(server: &MockServer, runtime: &Runtime, mocks: Vec<Mock>) {
    for mock in mocks {
        runtime.block_on(mock.mount(server));
    }
}

pub fn build_comments_mock(pr: u64, count: u64, expected_calls: u64) -> Mock {
    let comments = create_mock_comments(count);
    let comments_path = format!("/api/v3/repos/owner/repo/issues/{pr}/comments");

    Mock::given(method("GET"))
        .and(path(comments_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(comments))
        .up_to_n_times(expected_calls)
        .expect(expected_calls)
        .named("Issue comments (two loads)")
}

pub fn mount_server_with_revalidation(
    server: &MockServer,
    runtime: &Runtime,
    config: RevalidationMocks,
) {
    let pr_body = create_pr_body(config.pr, &config.title);
    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{}", config.pr);

    let initial = Mock::given(method("GET"))
        .and(path(pr_path.clone()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&pr_body)
                .insert_header("ETag", config.etag)
                .insert_header("Last-Modified", config.last_modified),
        )
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata initial fetch");

    let revalidated = Mock::given(method("GET"))
        .and(path(pr_path))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .named("PR metadata conditional 304");

    let comments_mock = build_comments_mock(config.pr, config.comment_count, 2);
    mount_mocks(server, runtime, vec![initial, revalidated, comments_mock]);
}

pub fn mount_server_with_invalidation(
    server: &MockServer,
    runtime: &Runtime,
    config: InvalidationMocks,
) {
    let pr_path = format!("/api/v3/repos/owner/repo/pulls/{}", config.pr);

    let old_body = create_pr_body(config.pr, &config.old_title);
    let new_body = create_pr_body(config.pr, &config.new_title);

    let initial = Mock::given(method("GET"))
        .and(path(pr_path.clone()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&old_body)
                .insert_header("ETag", config.etag1.clone()),
        )
        .up_to_n_times(1)
        .expect(1)
        .named("PR metadata initial fetch (etag-1)");

    let changed = Mock::given(method("GET"))
        .and(path(pr_path))
        .and(header("if-none-match", config.etag1.clone()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(&new_body)
                .insert_header("ETag", config.etag2),
        )
        .expect(1)
        .named("PR metadata refresh with new ETag");

    let comments_mock = build_comments_mock(config.pr, config.comment_count, 2);
    mount_mocks(server, runtime, vec![initial, changed, comments_mock]);
}

pub fn assert_error_variant_contains(
    error: &IntakeError,
    variant_name: &str,
    message_fragment: &str,
) {
    match variant_name {
        "Api" => {
            let IntakeError::Api { message } = error else {
                panic!("expected Api variant, got {error:?}");
            };
            assert!(
                message.contains(message_fragment),
                "unexpected error message: {message}"
            );
        }
        "Configuration" => {
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

const _: () = {
    let _revalidation_mocks_size = std::mem::size_of::<RevalidationMocks>();
    let _invalidation_mocks_size = std::mem::size_of::<InvalidationMocks>();
    let _ = create_database_path;
    let _ = expected_request_path;
    let _ = create_mock_comments;
    let _ = create_pr_body;
    let _ = mount_mocks;
    let _ = build_comments_mock;
    let _ = mount_server_with_revalidation;
    let _ = mount_server_with_invalidation;
    let _ = assert_error_variant_contains;
};
