//! Integration tests for the `OpenAI` rewrite adapter using `vidaimock`.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use frankie::ai::{
    CommentRewriteContext, CommentRewriteMode, CommentRewriteOutcome, CommentRewriteRequest,
    CommentRewriteService, OpenAiCommentRewriteConfig, OpenAiCommentRewriteService,
    rewrite_with_fallback,
};
use rstest::{fixture, rstest};

struct VidaiServer {
    base_url: String,
    child: Child,
}

impl Drop for VidaiServer {
    fn drop(&mut self) {
        let _kill_ignored = self.child.kill();
        let _wait_ignored = self.child.wait();
    }
}

#[fixture]
fn rewrite_request() -> CommentRewriteRequest {
    CommentRewriteRequest::new(
        CommentRewriteMode::Expand,
        "Please fix this",
        CommentRewriteContext::default(),
    )
}

#[rstest]
fn rewrite_text_reads_mock_response_from_vidaimock(rewrite_request: CommentRewriteRequest) {
    let Some(server) = spawn_vidaimock() else {
        return;
    };

    let config = OpenAiCommentRewriteConfig::new(
        format!("{}/v1", server.base_url),
        "gpt-4",
        Some("sk-test".to_owned()),
        Duration::from_secs(2),
    );
    let service = OpenAiCommentRewriteService::new(config);
    let result = service.rewrite_text(&rewrite_request);

    assert!(result.is_ok(), "expected successful rewrite from vidaimock");
    let text = result.unwrap_or_default();
    assert!(
        text.contains("mock response"),
        "expected mock output, got: {text}"
    );
}

#[rstest]
fn rewrite_with_fallback_handles_vidaimock_malformed_json(rewrite_request: CommentRewriteRequest) {
    let Some(server) = spawn_vidaimock() else {
        return;
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

    assert!(matches!(outcome, CommentRewriteOutcome::Fallback(_)));
}

fn spawn_vidaimock() -> Option<VidaiServer> {
    if !vidaimock_available() {
        return None;
    }

    let port = reserve_port().ok()?;
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

    let child = command.spawn().ok()?;
    let base_url = format!("http://127.0.0.1:{port}");

    wait_for_server(base_url.as_str());

    Some(VidaiServer { base_url, child })
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
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn wait_for_server(base_url: &str) {
    let metrics_url = format!("{base_url}/metrics");
    for _ in 0..40 {
        if reqwest::blocking::get(metrics_url.as_str()).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}
