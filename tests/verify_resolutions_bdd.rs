//! Behavioural tests for CLI automated resolution verification.

use std::process::{Command, Output};

use git2::{ErrorCode, Oid, Repository};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use frankie::persistence::{ReviewCommentVerificationCache, migrate_database};
use frankie::telemetry::NoopTelemetrySink;

#[path = "support/runtime.rs"]
mod runtime;

use runtime::SharedRuntime;

type TestError = Box<dyn std::error::Error>;
type StepResult = Result<(), TestError>;

#[derive(ScenarioState, Default)]
struct VerifyResolutionsState {
    runtime: Slot<SharedRuntime>,
    server: Slot<MockServer>,
    database_dir: Slot<TempDir>,
    database_url: Slot<String>,
    repo_dir: Slot<TempDir>,
    repo_path: Slot<String>,
    old_sha: Slot<String>,
    head_sha: Slot<String>,
    pr_url: Slot<String>,
    comment_id: Slot<u64>,
    output: Slot<Output>,
}

#[fixture]
fn verify_state() -> VerifyResolutionsState {
    VerifyResolutionsState::default()
}

fn binary_path() -> std::path::PathBuf {
    let mut path = std::env::current_exe()
        .unwrap_or_else(|error| panic!("failed to get current exe path: {error}"));
    path.pop();
    path.pop();
    path.push("frankie");
    path
}

fn run_frankie(args: &[String]) -> Output {
    let mut command = Command::new(binary_path());
    command.args(args);

    command
        .env_remove("FRANKIE_DATABASE_URL")
        .env_remove("FRANKIE_MIGRATE_DB")
        .env_remove("FRANKIE_PR_URL")
        .env_remove("FRANKIE_TOKEN")
        .env_remove("FRANKIE_OWNER")
        .env_remove("FRANKIE_REPO")
        .env_remove("GITHUB_TOKEN");

    command
        .output()
        .unwrap_or_else(|error| panic!("failed to execute binary: {error}"))
}

fn create_commit(
    repo: &Repository,
    message: &str,
    files: &[(&str, &str)],
) -> Result<Oid, TestError> {
    let sig = repo.signature()?;
    let mut index = repo.index()?;

    let workdir = repo
        .workdir()
        .ok_or("repository has no working directory")?;
    for (path, content) in files {
        let full = workdir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, content)?;
        index.add_path(std::path::Path::new(path))?;
    }

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let parent: Option<git2::Commit<'_>> = match repo.head() {
        Ok(head_ref) => Some(head_ref.peel_to_commit()?),
        Err(e) if e.code() == ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };
    let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();

    Ok(repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?)
}

#[derive(Debug, Clone, Copy)]
struct ReviewCommentsMount<'a> {
    pr: u64,
    comment_id: u64,
    commit_id: &'a str,
}

fn mount_review_comments(
    runtime: &SharedRuntime,
    server: &MockServer,
    mount: ReviewCommentsMount<'_>,
) {
    let comments_path = format!("/api/v3/repos/owner/repo/pulls/{}/comments", mount.pr);
    let response = ResponseTemplate::new(200).set_body_json(serde_json::json!([
        {
            "id": mount.comment_id,
            "body": "Please update this line",
            "user": { "login": "alice" },
            "path": "src/main.rs",
            "line": 2,
            "original_line": 2,
            "diff_hunk": "@@ -1,3 +1,3 @@",
            "commit_id": mount.commit_id,
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

#[given("the GitHub API returns a review comment pointing at the old commit")]
fn given_review_comment_from_api(verify_state: &VerifyResolutionsState) -> StepResult {
    let runtime = runtime::ensure_runtime_and_server(&verify_state.runtime, &verify_state.server)?;
    let server_uri = verify_state
        .server
        .with_ref(MockServer::uri)
        .ok_or("wiremock server should be initialised")?;

    let pr_url = format!("{server_uri}/owner/repo/pull/1");
    verify_state.pr_url.set(pr_url);

    let old_sha = verify_state
        .old_sha
        .get()
        .ok_or("old sha should be set before mounting mocks")?;

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
                    commit_id: &old_sha,
                },
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
    let output = run_frankie(&args);
    verify_state.output.set(output);
    Ok(())
}

#[then("the CLI output marks the comment as verified")]
fn then_output_is_verified(verify_state: &VerifyResolutionsState) -> StepResult {
    assert_verification_output(verify_state, "✓ verified comment 1")
}

#[then("the CLI output marks the comment as unverified")]
fn then_output_is_unverified(verify_state: &VerifyResolutionsState) -> StepResult {
    assert_verification_output(verify_state, "✗ unverified comment 1")
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

#[scenario(
    path = "tests/features/verify_resolutions.feature",
    name = "Verification marks changed lines as verified and persists results"
)]
fn verify_marks_changed_lines_as_verified(verify_state: VerifyResolutionsState) {
    drop(verify_state);
}

#[scenario(
    path = "tests/features/verify_resolutions.feature",
    name = "Verification marks unchanged lines as unverified and persists results"
)]
fn verify_marks_unchanged_lines_as_unverified(verify_state: VerifyResolutionsState) {
    drop(verify_state);
}
