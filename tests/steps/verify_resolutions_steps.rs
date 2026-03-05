//! Step implementations for verify-resolutions behavioural tests.

use std::process::{Command, Output};

use git2::Repository;
use rstest_bdd_macros::{given, then, when};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use frankie::persistence::{ReviewCommentVerificationCache, migrate_database};
use frankie::telemetry::NoopTelemetrySink;

use super::runtime;
use super::verify_resolutions_helpers::{
    ReviewCommentsMount, count_cache_rows, create_commit, mount_review_comments,
};
use super::{StepResult, VerifyResolutionsState};

fn binary_path() -> StepResult<std::path::PathBuf> {
    let mut path = std::env::current_exe()?;
    path.pop();
    path.pop();
    path.push("frankie");
    Ok(path)
}

fn run_frankie(args: &[String]) -> StepResult<Output> {
    let mut command = Command::new(binary_path()?);
    command.args(args);

    command
        .env_remove("FRANKIE_DATABASE_URL")
        .env_remove("FRANKIE_MIGRATE_DB")
        .env_remove("FRANKIE_PR_URL")
        .env_remove("FRANKIE_TOKEN")
        .env_remove("FRANKIE_OWNER")
        .env_remove("FRANKIE_REPO")
        .env_remove("GITHUB_TOKEN");

    Ok(command.output()?)
}

fn configure_review_comment_mock(
    verify_state: &VerifyResolutionsState,
    commit_id: &str,
) -> StepResult {
    let runtime = runtime::ensure_runtime_and_server(&verify_state.runtime, &verify_state.server)?;
    let server_uri = verify_state
        .server
        .with_ref(MockServer::uri)
        .ok_or("wiremock server should be initialised")?;

    let pr_url = format!("{server_uri}/owner/repo/pull/1");
    verify_state.pr_url.set(pr_url);
    let comment_id = 1_u64;
    verify_state.comment_id.set(comment_id);
    verify_state
        .server
        .with_ref(|server| {
            mount_review_comments(
                &runtime,
                server,
                ReviewCommentsMount {
                    pr: 1,
                    comment_id,
                    commit_id,
                },
            );
        })
        .ok_or("wiremock server should be initialised")?;
    Ok(())
}

fn setup_test_repository(
    verify_state: &VerifyResolutionsState,
    old_content: &str,
    new_content: &str,
) -> StepResult {
    let repo_dir = TempDir::new()?;
    let repo = Repository::init(repo_dir.path())?;

    let mut config = repo.config()?;
    config.set_str("user.name", "Test User")?;
    config.set_str("user.email", "test@example.com")?;

    repo.remote("origin", "https://127.0.0.1/owner/repo.git")?;

    let old = create_commit(&repo, "old", &[("src/main.rs", old_content)])?;
    let head = create_commit(&repo, "head", &[("src/main.rs", new_content)])?;

    verify_state.old_sha.set(old.to_string());
    verify_state.head_sha.set(head.to_string());
    verify_state
        .repo_path
        .set(repo_dir.path().to_string_lossy().to_string());
    verify_state.repo_dir.set(repo_dir);
    Ok(())
}

fn assert_verification_output(
    verify_state: &VerifyResolutionsState,
    expected_marker: &str,
) -> StepResult {
    let output = verify_state
        .output
        .get()
        .ok_or("output should be captured")?;
    if !output.status.success() {
        return Err("expected success exit status".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains(expected_marker) {
        return Err(
            format!("expected stdout to contain '{expected_marker}', got: {stdout}").into(),
        );
    }
    Ok(())
}

#[given("a migrated database for verification")]
fn given_migrated_database(verify_state: &VerifyResolutionsState) -> StepResult {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("frankie.sqlite");
    let database_url = db_path.to_string_lossy().to_string();
    migrate_database(&database_url, &NoopTelemetrySink)?;
    verify_state.database_dir.set(temp_dir);
    verify_state.database_url.set(database_url);
    Ok(())
}

#[given("a local repository where the referenced line changes between commits")]
fn given_repo_with_changed_line(verify_state: &VerifyResolutionsState) -> StepResult {
    setup_test_repository(
        verify_state,
        "fn main() {\nlet x = 1;\n}\n",
        "fn main() {\nlet x = 2;\n}\n",
    )
}

#[given("a local repository where the referenced line is unchanged between commits")]
fn given_repo_with_unchanged_line(verify_state: &VerifyResolutionsState) -> StepResult {
    setup_test_repository(
        verify_state,
        "fn main() {\nlet x = 1;\n}\n",
        "fn main() {\nlet x = 1;\n}\n",
    )
}

#[given("a local repository where the referenced line is deleted between commits")]
fn given_repo_with_deleted_line(verify_state: &VerifyResolutionsState) -> StepResult {
    setup_test_repository(
        verify_state,
        "fn main() {\nlet x = 1;\nlet y = 2;\n}\n",
        "fn main() {\nlet y = 2;\n}\n",
    )
}

#[given("the GitHub API returns a review comment pointing at the old commit")]
fn given_review_comment_from_api(verify_state: &VerifyResolutionsState) -> StepResult {
    let old_sha = verify_state
        .old_sha
        .get()
        .ok_or("old sha should be set before mounting mocks")?;
    configure_review_comment_mock(verify_state, &old_sha)
}

#[given("the GitHub API returns a review comment pointing at an unknown commit")]
fn given_review_comment_with_unknown_commit(verify_state: &VerifyResolutionsState) -> StepResult {
    configure_review_comment_mock(verify_state, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
}

#[given("the GitHub API returns a review comment missing verification metadata")]
fn given_review_comment_missing_metadata(verify_state: &VerifyResolutionsState) -> StepResult {
    let runtime = runtime::ensure_runtime_and_server(&verify_state.runtime, &verify_state.server)?;
    let server_uri = verify_state
        .server
        .with_ref(MockServer::uri)
        .ok_or("wiremock server should be initialised")?;

    let pr_url = format!("{server_uri}/owner/repo/pull/1");
    verify_state.pr_url.set(pr_url);
    let comment_id = 1_u64;
    verify_state.comment_id.set(comment_id);
    verify_state
        .server
        .with_ref(|server| {
            let comments_path = "/api/v3/repos/owner/repo/pulls/1/comments";
            let response = ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": comment_id,
                    "body": "Please update this line",
                    "user": { "login": "alice" },
                    "path": "src/main.rs",
                    "line": 2,
                    "original_line": 2,
                    "diff_hunk": "@@ -1,3 +1,3 @@",
                    "commit_id": null,
                    "in_reply_to_id": null,
                    "created_at": "2026-03-02T00:00:00Z",
                    "updated_at": "2026-03-02T00:00:00Z"
                }
            ]));
            runtime.block_on(
                Mock::given(method("GET"))
                    .and(path(comments_path))
                    .respond_with(response)
                    .mount(server),
            );
        })
        .ok_or("wiremock server should be initialised")?;
    Ok(())
}

#[when("the user runs resolution verification")]
fn when_user_runs_verification(verify_state: &VerifyResolutionsState) -> StepResult {
    let database_url = verify_state
        .database_url
        .get()
        .ok_or("database url should be set")?;
    let repo_path = verify_state
        .repo_path
        .get()
        .ok_or("repo path should be set")?;
    let pr_url = verify_state.pr_url.get().ok_or("pr url should be set")?;

    let args = vec![
        "--verify-resolutions".to_owned(),
        "--pr-url".to_owned(),
        pr_url,
        "--token".to_owned(),
        "token".to_owned(),
        "--database-url".to_owned(),
        database_url,
        "--repo-path".to_owned(),
        repo_path,
    ];
    let output = run_frankie(&args)?;
    verify_state.output.set(output);
    Ok(())
}

#[when("the user runs resolution verification with a positional PR number")]
fn when_user_runs_verification_with_positional_pr(
    verify_state: &VerifyResolutionsState,
) -> StepResult {
    let database_url = verify_state
        .database_url
        .get()
        .ok_or("database url should be set")?;
    let repo_path = verify_state
        .repo_path
        .get()
        .ok_or("repo path should be set")?;

    let args = vec![
        "1".to_owned(),
        "--verify-resolutions".to_owned(),
        "--token".to_owned(),
        "token".to_owned(),
        "--database-url".to_owned(),
        database_url,
        "--repo-path".to_owned(),
        repo_path,
    ];
    let output = run_frankie(&args)?;
    verify_state.output.set(output);
    Ok(())
}

#[when("the user runs resolution verification twice")]
fn when_user_runs_verification_twice(verify_state: &VerifyResolutionsState) -> StepResult {
    when_user_runs_verification(verify_state)?;
    when_user_runs_verification(verify_state)
}

#[then("the CLI output marks the comment as verified")]
fn then_output_is_verified(verify_state: &VerifyResolutionsState) -> StepResult {
    assert_verification_output(verify_state, "✓ verified comment 1")
}

#[then("the CLI output marks the comment as unverified")]
fn then_output_is_unverified(verify_state: &VerifyResolutionsState) -> StepResult {
    assert_verification_output(verify_state, "✗ unverified comment 1")
}

#[then("the CLI output explains repository data is unavailable")]
fn then_output_mentions_repository_data_issue(verify_state: &VerifyResolutionsState) -> StepResult {
    let output = verify_state
        .output
        .get()
        .ok_or("output should be captured")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("repository data unavailable") || stdout.contains("insufficient metadata") {
        return Ok(());
    }
    Err(format!("expected repository-data explanation in stdout, got: {stdout}").into())
}

#[then("the verification status is persisted in the local cache")]
fn then_status_is_persisted(verify_state: &VerifyResolutionsState) -> StepResult {
    let database_url = verify_state
        .database_url
        .get()
        .ok_or("database url should be set")?;
    let head_sha = verify_state
        .head_sha
        .get()
        .ok_or("head sha should be set")?;
    let comment_id = verify_state
        .comment_id
        .get()
        .ok_or("comment id should be set")?;

    let cache = ReviewCommentVerificationCache::new(database_url)?;
    let rows = cache.get_for_comments(&[comment_id], &head_sha)?;
    let record = rows.get(&comment_id).ok_or("expected persisted record")?;

    let output = verify_state
        .output
        .get()
        .ok_or("output should be captured")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_status = if stdout.contains("✓ verified comment 1") {
        "verified"
    } else {
        "unverified"
    };
    let actual_status = record.status.as_db_value();
    if actual_status != expected_status {
        return Err(
            format!("expected persisted status {expected_status}, got {actual_status}").into(),
        );
    }

    Ok(())
}

#[then("the cache contains one verification row for the comment and target")]
fn then_cache_contains_single_row(verify_state: &VerifyResolutionsState) -> StepResult {
    let database_url = verify_state
        .database_url
        .get()
        .ok_or("database url should be set")?;
    let head_sha = verify_state
        .head_sha
        .get()
        .ok_or("head sha should be set")?;
    let comment_id = verify_state
        .comment_id
        .get()
        .ok_or("comment id should be set")?;
    let row_count = count_cache_rows(&database_url, comment_id, &head_sha)?;
    if row_count != 1 {
        return Err(format!("expected exactly one cache row, got {row_count}").into());
    }
    Ok(())
}
