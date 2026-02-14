//! Process execution helpers for Codex integration.
//!
//! Manages the lifecycle of a `codex app-server` child process:
//! spawning, stdin/stdout plumbing, stream polling, and graceful
//! termination.

mod app_server;
mod stream;

use std::fmt::Write as _;
use std::io::BufRead;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use camino::Utf8PathBuf;

use crate::ai::codex_exec::{
    CodexExecutionHandle, CodexExecutionOutcome, CodexExecutionRequest, CodexExecutionUpdate,
};

use super::transcript::TranscriptWriter;

use self::app_server::AppServerCompletion;
use self::stream::{StreamCompletion, StreamProgressContext};

#[cfg(test)]
pub(crate) use self::stream::parse_progress_event;

/// Error arising during a Codex child-process run.
///
/// Wraps a human-readable description of the failure so callers can
/// forward it through the TUI channel without matching on variants.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct RunError(String);

impl RunError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Command name and arguments used to launch the Codex process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexCommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

/// Builds the command specification for `codex app-server`.
pub(crate) fn build_command_spec(command_path: &str) -> CodexCommandSpec {
    CodexCommandSpec {
        program: command_path.to_owned(),
        args: vec!["app-server".to_owned()],
    }
}

/// Spawns a background thread that executes Codex and streams progress
/// updates through the returned handle.
pub(crate) fn run_codex(
    command_path: String,
    request: CodexExecutionRequest,
    transcript_path: Utf8PathBuf,
) -> CodexExecutionHandle {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        execute_codex(command_path.as_str(), &request, transcript_path, &sender);
    });

    CodexExecutionHandle::new(receiver)
}

/// Create the transcript writer, sending failure on error.
fn create_transcript(
    transcript_path: &Utf8PathBuf,
    sender: &Sender<CodexExecutionUpdate>,
) -> Option<TranscriptWriter> {
    match TranscriptWriter::create(transcript_path) {
        Ok(writer) => Some(writer),
        Err(error) => {
            send_failure(
                sender,
                format!("failed to create transcript: {error}"),
                None,
                None,
            );
            None
        }
    }
}

/// Spawn the Codex child process with piped stdin, stdout, and stderr.
fn spawn_codex(command_spec: &CodexCommandSpec) -> Result<Child, RunError> {
    let mut command = Command::new(&command_spec.program);
    command
        .args(&command_spec.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    command
        .spawn()
        .map_err(|error| RunError::new(format!("failed to launch Codex: {error}")))
}

fn execute_codex(
    command_path: &str,
    request: &CodexExecutionRequest,
    transcript_path: Utf8PathBuf,
    sender: &Sender<CodexExecutionUpdate>,
) {
    let Some(mut transcript) = create_transcript(&transcript_path, sender) else {
        return;
    };

    let prompt = build_prompt(request);
    let command_spec = build_command_spec(command_path);

    let mut child = match spawn_codex(&command_spec) {
        Ok(child) => child,
        Err(error) => {
            send_failure(
                sender,
                error.to_string(),
                None,
                Some(transcript.path().to_path_buf()),
            );
            return;
        }
    };

    let Some(stdout) = child.stdout.take() else {
        send_failure(
            sender,
            "codex stdout stream was unavailable".to_owned(),
            None,
            Some(transcript.path().to_path_buf()),
        );
        return;
    };

    let stdin = child.stdin.take();
    let mut stderr_capture = StderrCapture::spawn(child.stderr.take());

    let completion = {
        let mut stream_context = StreamProgressContext {
            prompt: prompt.as_str(),
            transcript: &mut transcript,
            sender,
        };
        match stream::stream_progress(stdout, stdin, &mut stream_context) {
            Ok(completion) => completion,
            Err(error) => {
                send_failure(
                    sender,
                    stderr_capture.append_to(error.to_string()),
                    None,
                    Some(transcript.path().to_path_buf()),
                );
                return;
            }
        }
    };

    if let Err(error) = transcript.flush() {
        send_failure(
            sender,
            stderr_capture.append_to(format!("failed to flush transcript: {error}")),
            None,
            Some(transcript.path().to_path_buf()),
        );
        return;
    }

    match completion {
        StreamCompletion::AppServer(outcome) => {
            terminate_child(&mut child);
            handle_app_server_outcome(outcome, sender, transcript_path, &mut stderr_capture);
        }
        StreamCompletion::ProcessExit => {
            complete_with_exit_status(child, sender, transcript_path, &mut stderr_capture);
        }
    }
}

fn handle_app_server_outcome(
    outcome: AppServerCompletion,
    sender: &Sender<CodexExecutionUpdate>,
    transcript_path: Utf8PathBuf,
    stderr_capture: &mut StderrCapture,
) {
    match outcome {
        AppServerCompletion::Succeeded => {
            drop(sender.send(CodexExecutionUpdate::Finished(
                CodexExecutionOutcome::Succeeded { transcript_path },
            )));
        }
        AppServerCompletion::Failed(message) => {
            send_failure(
                sender,
                stderr_capture.append_to(message),
                None,
                Some(transcript_path),
            );
        }
    }
}

fn complete_with_exit_status(
    mut child: Child,
    sender: &Sender<CodexExecutionUpdate>,
    transcript_path: Utf8PathBuf,
    stderr_capture: &mut StderrCapture,
) {
    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => {
            send_failure(
                sender,
                stderr_capture.append_to(format!("failed waiting for Codex exit: {error}")),
                None,
                Some(transcript_path),
            );
            return;
        }
    };

    if status.success() {
        drop(sender.send(CodexExecutionUpdate::Finished(
            CodexExecutionOutcome::Succeeded { transcript_path },
        )));
        return;
    }

    send_failure(
        sender,
        stderr_capture.append_to("codex exited with a non-zero status".to_owned()),
        status.code(),
        Some(transcript_path),
    );
}

/// Maximum number of bytes to capture from stderr (64 KiB).
const STDERR_LIMIT: usize = 65_536;

/// Captured stderr output from a child process.
///
/// Spawns a background thread that drains the child's stderr stream
/// into a bounded buffer. The captured text can later be appended to
/// failure messages via [`StderrCapture::append_to`].
struct StderrCapture {
    buffer: Arc<Mutex<String>>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
}

impl StderrCapture {
    /// Starts capturing stderr from the child process.
    fn spawn(child_stderr: Option<ChildStderr>) -> Self {
        let buffer = Arc::new(Mutex::new(String::new()));
        let reader_thread = child_stderr.map(|readable| {
            let handle = Arc::clone(&buffer);
            std::thread::spawn(move || Self::drain(readable, &handle))
        });
        Self {
            buffer,
            reader_thread,
        }
    }

    /// Reads lines from stderr into the shared buffer up to the size limit.
    fn drain(readable: ChildStderr, buffer: &Mutex<String>) {
        let reader = std::io::BufReader::new(readable);
        for result in reader.lines() {
            let Ok(text) = result else { break };
            let Ok(mut content) = buffer.lock() else {
                break;
            };
            if content.len() + text.len() > STDERR_LIMIT {
                break;
            }
            content.push_str(&text);
            content.push('\n');
        }
    }

    /// Appends any captured stderr to `message`, or returns it unchanged
    /// when stderr is empty. Joins the reader thread first to ensure all
    /// output has been collected.
    fn append_to(&mut self, message: String) -> String {
        if let Some(thread) = self.reader_thread.take() {
            drop(thread.join());
        }

        let captured = self
            .buffer
            .lock()
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.clone());

        match captured {
            Some(text) => format!("{message}\n\nstderr:\n{text}"),
            None => message,
        }
    }
}

fn log_termination_error(action: &str, error: &std::io::Error) {
    tracing::trace!("failed to {action} Codex child process: {error}");
}

fn terminate_child(child: &mut Child) {
    if let Ok(None) = child.try_wait() {
        if let Err(error) = child.kill() {
            log_termination_error("kill", &error);
        }
        if let Err(error) = child.wait() {
            log_termination_error("wait for", &error);
        }
    }
}

fn send_failure(
    sender: &Sender<CodexExecutionUpdate>,
    message: String,
    exit_code: Option<i32>,
    transcript_path: Option<Utf8PathBuf>,
) {
    drop(sender.send(CodexExecutionUpdate::Finished(
        CodexExecutionOutcome::Failed {
            message,
            exit_code,
            transcript_path,
        },
    )));
}

fn build_prompt(request: &CodexExecutionRequest) -> String {
    let mut prompt = format!(
        concat!(
            "Resolve review comments for pull request {}/{} #{}.",
            "\nSummarise key changes and apply fixes where safe.",
            "\nReview comments (JSONL):\n"
        ),
        request.context().owner(),
        request.context().repository(),
        request.context().pr_number(),
    );

    if let Some(pr_url) = request.pr_url() {
        // writeln! on a String is infallible; use write! directly.
        let _infallible = writeln!(prompt, "Pull request URL: {pr_url}");
    }

    prompt.push_str(request.comments_jsonl());
    prompt
}
