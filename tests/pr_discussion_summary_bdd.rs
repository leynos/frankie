//! Behavioural tests for the CLI PR-discussion summary mode.

use std::path::PathBuf;
use std::process::{Command, Output};

use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[path = "support/runtime.rs"]
mod runtime;
#[path = "support/vidaimock.rs"]
mod vidaimock;

type StepResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(ScenarioState, Default)]
struct PrDiscussionSummaryState {
    runtime: Slot<runtime::SharedRuntime>,
    server: Slot<MockServer>,
    vidai_server: Slot<vidaimock::VidaiServer>,
    skip_vidai: Slot<bool>,
    pr_url: Slot<String>,
    output: Slot<Output>,
}

#[fixture]
fn pr_discussion_summary_state() -> PrDiscussionSummaryState {
    PrDiscussionSummaryState::default()
}

fn binary_path() -> StepResult<PathBuf> {
    let mut path = std::env::current_exe()?;
    path.pop();
    path.pop();
    path.push("frankie");
    Ok(path)
}

fn fixture_config_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vidaimock/pr_discussion_summary")
}

fn empty_config_path() -> PathBuf {
    fixture_config_dir().join("empty.frankie.toml")
}

fn configure_comments(
    pr_discussion_summary_state: &PrDiscussionSummaryState,
    comments: serde_json::Value,
) -> StepResult {
    let shared_runtime = runtime::ensure_runtime_and_server(
        &pr_discussion_summary_state.runtime,
        &pr_discussion_summary_state.server,
    )?;
    let server_uri = pr_discussion_summary_state
        .server
        .with_ref(MockServer::uri)
        .ok_or("wiremock server should be initialised")?;
    let pr_url = format!("{server_uri}/owner/repo/pull/1");
    pr_discussion_summary_state.pr_url.set(pr_url);
    pr_discussion_summary_state
        .server
        .with_ref(|server| {
            shared_runtime.block_on(
                Mock::given(method("GET"))
                    .and(path("/api/v3/repos/owner/repo/pulls/1/comments"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(comments))
                    .mount(server),
            );
        })
        .ok_or("wiremock server should be initialised")?;
    Ok(())
}

fn run_frankie(args: &[String]) -> StepResult<Output> {
    let mut command = Command::new(binary_path()?);
    let mut command_args = args.to_vec();
    command_args.extend([
        "--config-path".to_owned(),
        empty_config_path().display().to_string(),
    ]);
    command.args(&command_args);
    command
        .env_remove("FRANKIE_DATABASE_URL")
        .env_remove("FRANKIE_MIGRATE_DB")
        .env_remove("FRANKIE_PR_URL")
        .env_remove("FRANKIE_TOKEN")
        .env_remove("FRANKIE_OWNER")
        .env_remove("FRANKIE_REPO")
        .env_remove("FRANKIE_EXPORT")
        .env_remove("FRANKIE_OUTPUT")
        .env_remove("FRANKIE_TEMPLATE")
        .env_remove("FRANKIE_REPO_PATH")
        .env_remove("FRANKIE_REPLY_MAX_LENGTH")
        .env_remove("FRANKIE_REPLY_TEMPLATES")
        .env_remove("FRANKIE_AI_REWRITE_MODE")
        .env_remove("FRANKIE_AI_REWRITE_TEXT")
        .env_remove("FRANKIE_AI_BASE_URL")
        .env_remove("FRANKIE_AI_MODEL")
        .env_remove("FRANKIE_AI_TIMEOUT_SECONDS")
        .env_remove("FRANKIE_CONFIG_PATH")
        .env_remove("FRANKIE_AI_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("GITHUB_TOKEN");

    Ok(command.output()?)
}

fn should_skip_vidai(state: &PrDiscussionSummaryState) -> bool {
    state.skip_vidai.get().unwrap_or(false)
}

#[given("the GitHub API returns review comments with replies and general discussion")]
fn given_review_comments_with_replies(
    pr_discussion_summary_state: &PrDiscussionSummaryState,
) -> StepResult {
    configure_comments(
        pr_discussion_summary_state,
        serde_json::json!([
            {
                "id": 1,
                "body": "Please handle the panic path.",
                "user": { "login": "alice" },
                "path": "src/main.rs",
                "line": 12,
                "original_line": 12,
                "diff_hunk": "@@ -1 +1 @@",
                "commit_id": "old",
                "in_reply_to_id": null,
                "created_at": "2026-03-09T00:00:00Z",
                "updated_at": "2026-03-09T00:00:00Z"
            },
            {
                "id": 2,
                "body": "Agreed, this should not panic.",
                "user": { "login": "bob" },
                "path": "src/main.rs",
                "line": 12,
                "original_line": 12,
                "diff_hunk": "@@ -1 +1 @@",
                "commit_id": "old",
                "in_reply_to_id": 1,
                "created_at": "2026-03-09T00:01:00Z",
                "updated_at": "2026-03-09T00:01:00Z"
            },
            {
                "id": 3,
                "body": "Please clarify the module comment.",
                "user": { "login": "carol" },
                "path": null,
                "line": null,
                "original_line": null,
                "diff_hunk": null,
                "commit_id": "old",
                "in_reply_to_id": null,
                "created_at": "2026-03-09T00:02:00Z",
                "updated_at": "2026-03-09T00:02:00Z"
            }
        ]),
    )
}

#[given("the GitHub API returns no review comments")]
fn given_no_review_comments(pr_discussion_summary_state: &PrDiscussionSummaryState) -> StepResult {
    configure_comments(pr_discussion_summary_state, serde_json::json!([]))
}

#[given("a VidaiMock summary server is available")]
fn given_vidaimock_summary_server(
    pr_discussion_summary_state: &PrDiscussionSummaryState,
) -> StepResult {
    match vidaimock::spawn_vidaimock(fixture_config_dir().as_path())? {
        Some(server) => {
            pr_discussion_summary_state.vidai_server.set(server);
            pr_discussion_summary_state.skip_vidai.set(false);
        }
        None => {
            pr_discussion_summary_state.skip_vidai.set(true);
        }
    }

    Ok(())
}

#[when("the user runs PR discussion summary mode")]
fn when_user_runs_summary_mode(
    pr_discussion_summary_state: &PrDiscussionSummaryState,
) -> StepResult {
    let pr_url = pr_discussion_summary_state
        .pr_url
        .get()
        .ok_or("PR URL should be configured")?;
    let mut args = vec![
        "--token".to_owned(),
        "ghp_test".to_owned(),
        "--summarize-discussions".to_owned(),
        "--pr-url".to_owned(),
        pr_url,
    ];

    if !should_skip_vidai(pr_discussion_summary_state) {
        let ai_base_url = pr_discussion_summary_state
            .vidai_server
            .with_ref(|server| format!("{}/v1", server.base_url))
            .ok_or("vidaimock server should be available when not skipping")?;
        args.extend([
            "--ai-base-url".to_owned(),
            ai_base_url,
            "--ai-model".to_owned(),
            "gpt-4".to_owned(),
            "--ai-api-key".to_owned(),
            "sk-test".to_owned(),
        ]);
    }

    pr_discussion_summary_state.output.set(run_frankie(&args)?);
    Ok(())
}

#[then("the command exits successfully")]
fn then_command_exits_successfully(
    pr_discussion_summary_state: &PrDiscussionSummaryState,
) -> StepResult {
    if should_skip_vidai(pr_discussion_summary_state) {
        return Ok(());
    }

    let output = pr_discussion_summary_state
        .output
        .get()
        .ok_or("command output should be captured")?;
    if !output.status.success() {
        return Err(format!(
            "expected success exit status, stderr was: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    Ok(())
}

fn assert_stdout_contains(
    pr_discussion_summary_state: &PrDiscussionSummaryState,
    text: String,
    should_contain: bool,
) -> StepResult {
    if should_skip_vidai(pr_discussion_summary_state) {
        return Ok(());
    }

    let owned_text = text;
    let expected = owned_text.trim_matches('"');
    let output = pr_discussion_summary_state
        .output
        .get()
        .ok_or("command output should be captured")?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains(expected) != should_contain {
        let verb = if should_contain {
            "contain"
        } else {
            "not contain"
        };
        return Err(format!("expected stdout to {verb} '{expected}', got:\n{stdout}").into());
    }

    Ok(())
}

#[then("stdout contains {text}")]
fn then_stdout_contains(
    pr_discussion_summary_state: &PrDiscussionSummaryState,
    text: String,
) -> StepResult {
    assert_stdout_contains(pr_discussion_summary_state, text, true)
}

#[then("stdout does not contain {text}")]
fn then_stdout_does_not_contain(
    pr_discussion_summary_state: &PrDiscussionSummaryState,
    text: String,
) -> StepResult {
    assert_stdout_contains(pr_discussion_summary_state, text, false)
}

#[then("the command fails with stderr containing {text}")]
fn then_command_fails_with_stderr(
    pr_discussion_summary_state: &PrDiscussionSummaryState,
    text: String,
) -> StepResult {
    let expected = text.trim_matches('"');
    let output = pr_discussion_summary_state
        .output
        .get()
        .ok_or("command output should be captured")?;
    if output.status.success() {
        return Err("expected failure exit status".into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains(expected) {
        return Err(format!("expected stderr to contain '{expected}', got:\n{stderr}").into());
    }

    Ok(())
}

#[scenario(path = "tests/features/pr_discussion_summary.feature", index = 0)]
fn cli_summary_groups_comments(pr_discussion_summary_state: PrDiscussionSummaryState) {
    let _ = pr_discussion_summary_state;
}

#[scenario(path = "tests/features/pr_discussion_summary.feature", index = 1)]
fn cli_summary_rejects_empty_comment_sets(pr_discussion_summary_state: PrDiscussionSummaryState) {
    let _ = pr_discussion_summary_state;
}
