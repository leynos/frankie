//! Integration tests for the `OpenAI` rewrite adapter using `vidaimock`.

use std::error::Error;
use std::io::{self, ErrorKind};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use frankie::ai::{
    CommentRewriteContext, CommentRewriteMode, CommentRewriteOutcome, CommentRewriteRequest,
    CommentRewriteService, OpenAiCommentRewriteConfig, OpenAiCommentRewriteService,
    rewrite_with_fallback,
};
use rewrite_request_fixture::rewrite_request;
use rstest::rstest;

#[path = "../src/ai/comment_rewrite/rewrite_request_fixture.rs"]
mod rewrite_request_fixture;

struct VidaiServer {
    base_url: String,
    child: Child,
}

type TestResult<T> = Result<T, Box<dyn Error>>;

impl Drop for VidaiServer {
    fn drop(&mut self) {
        let _kill_ignored = self.child.kill();
        let _wait_ignored = self.child.wait();
    }
}

#[rstest]
fn rewrite_text_reads_mock_response_from_vidaimock(
    rewrite_request: CommentRewriteRequest,
) -> TestResult<()> {
    let Some(server) = spawn_vidaimock()? else {
        return Ok(());
    };

    let config = OpenAiCommentRewriteConfig::new(
        format!("{}/v1", server.base_url),
        "gpt-4",
        Some("sk-test".to_owned()),
        Duration::from_secs(2),
    );
    let service = OpenAiCommentRewriteService::new(config);
    let text = service.rewrite_text(&rewrite_request)?;

    if !text.contains("mock response") {
        return Err(io::Error::other(format!("expected mock output, got: {text}")).into());
    }
    Ok(())
}

#[rstest]
fn rewrite_with_fallback_handles_vidaimock_malformed_json(
    rewrite_request: CommentRewriteRequest,
) -> TestResult<()> {
    let Some(server) = spawn_vidaimock()? else {
        return Ok(());
    };

    let config = OpenAiCommentRewriteConfig::new(
        format!("{}/v1", server.base_url),
        "gpt-4",
        Some("sk-test".to_owned()),
        Duration::from_secs(2),
    )
    .with_additional_header("X-Vidai-Chaos-Malformed", "100");

    let service = OpenAiCommentRewriteService::new(config);
    let outcome = rewrite_with_fallback(&service, &rewrite_request);

    if !matches!(outcome, CommentRewriteOutcome::Fallback(_)) {
        return Err(io::Error::other(format!(
            "expected fallback outcome for malformed response, got {outcome:?}"
        ))
        .into());
    }
    Ok(())
}

fn spawn_vidaimock() -> TestResult<Option<VidaiServer>> {
    if !vidaimock_available() {
        return Ok(None);
    }

    let port = reserve_port()?;
    let mut command = Command::new("vidaimock");
    command
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--format")
        .arg("openai")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command
        .spawn()
        .map_err(|error| io::Error::other(format!("failed to spawn vidaimock process: {error}")))?;
    let base_url = format!("http://127.0.0.1:{port}");

    if let Err(error) = wait_for_server(base_url.as_str()) {
        let _kill_ignored = child.kill();
        let _wait_ignored = child.wait();
        return Err(error);
    }

    Ok(Some(VidaiServer { base_url, child }))
}

fn vidaimock_available() -> bool {
    Command::new("vidaimock")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn reserve_port() -> Result<u16, std::io::Error> {
    // Best-effort ephemeral-port reservation has a TOCTOU race after drop(listener):
    // another process can claim the port before vidaimock binds; true guarantees
    // require binding in the consumer or retrying on bind failure.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn wait_for_server(base_url: &str) -> TestResult<()> {
    let metrics_url = format!("{base_url}/metrics");
    for _ in 0..40 {
        if reqwest::blocking::get(metrics_url.as_str()).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(io::Error::new(
        ErrorKind::TimedOut,
        format!("timed out waiting for vidaimock server readiness at {metrics_url}"),
    )
    .into())
}
