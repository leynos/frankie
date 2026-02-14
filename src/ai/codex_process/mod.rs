//! Process execution helpers for Codex integration.
//!
//! Manages the lifecycle of a `codex app-server` child process:
//! spawning, stdin/stdout plumbing, stream polling, and graceful
//! termination.

mod app_server;
mod stream;

use std::fmt::Write as _;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Sender};

use camino::{Utf8Path, Utf8PathBuf};

use crate::ai::codex_exec::{
    CodexExecutionHandle, CodexExecutionOutcome, CodexExecutionRequest, CodexExecutionUpdate,
};

use super::transcript::TranscriptWriter;

use self::app_server::AppServerCompletion;
use self::stream::{StreamCompletion, StreamProgressContext};

#[cfg(test)]
pub(crate) use self::stream::parse_progress_event;

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
    transcript_path: &Utf8Path,
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

/// Spawn the Codex child process and capture stdout and stdin.
///
/// Returns `None` on failure (failure already sent via sender).
fn setup_codex_process(
    command_spec: &CodexCommandSpec,
    sender: &Sender<CodexExecutionUpdate>,
    transcript_path: Utf8PathBuf,
) -> Option<(Child, ChildStdout, Option<ChildStdin>)> {
    let mut child = spawn_child(command_spec, sender, transcript_path.clone())?;
    let stdout = take_stdout(&mut child, sender, transcript_path)?;
    let stdin = child.stdin.take();
    Some((child, stdout, stdin))
}

/// Handle the completion outcome from streaming, either from
/// app-server or process exit.
fn handle_completion(
    completion: StreamCompletion,
    mut child: Child,
    sender: &Sender<CodexExecutionUpdate>,
    transcript_path: Utf8PathBuf,
) {
    match completion {
        StreamCompletion::AppServer(outcome) => {
            terminate_child(&mut child);
            match outcome {
                AppServerCompletion::Succeeded => {
                    drop(sender.send(CodexExecutionUpdate::Finished(
                        CodexExecutionOutcome::Succeeded { transcript_path },
                    )));
                }
                AppServerCompletion::Failed(message) => {
                    send_failure(sender, message, None, Some(transcript_path));
                }
            }
        }
        StreamCompletion::ProcessExit => {
            let runtime_path = transcript_path.clone();
            complete_with_exit_status(child, sender, runtime_path, transcript_path);
        }
    }
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

    let Some((child, stdout, stdin)) =
        setup_codex_process(&command_spec, sender, transcript.path().to_path_buf())
    else {
        return;
    };

    let completion = {
        let mut stream_context = StreamProgressContext {
            prompt: prompt.as_str(),
            transcript: &mut transcript,
            sender,
        };
        match stream::stream_progress(stdout, stdin, &mut stream_context) {
            Ok(completion) => completion,
            Err(error) => {
                send_failure(sender, error, None, Some(transcript.path().to_path_buf()));
                return;
            }
        }
    };

    if let Err(error) = transcript.flush() {
        send_failure(
            sender,
            format!("failed to flush transcript: {error}"),
            None,
            Some(transcript.path().to_path_buf()),
        );
        return;
    }

    handle_completion(completion, child, sender, transcript_path);
}

fn spawn_child(
    command_spec: &CodexCommandSpec,
    sender: &Sender<CodexExecutionUpdate>,
    transcript_path: Utf8PathBuf,
) -> Option<Child> {
    let mut command = Command::new(&command_spec.program);
    command
        .args(&command_spec.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    match command.spawn() {
        Ok(child) => Some(child),
        Err(error) => {
            send_failure(
                sender,
                format!("failed to launch Codex: {error}"),
                None,
                Some(transcript_path),
            );
            None
        }
    }
}

fn take_stdout(
    child: &mut Child,
    sender: &Sender<CodexExecutionUpdate>,
    transcript_path: Utf8PathBuf,
) -> Option<ChildStdout> {
    let Some(stdout) = child.stdout.take() else {
        send_failure(
            sender,
            "codex stdout stream was unavailable".to_owned(),
            None,
            Some(transcript_path),
        );
        return None;
    };

    Some(stdout)
}

fn complete_with_exit_status(
    mut child: Child,
    sender: &Sender<CodexExecutionUpdate>,
    transcript_runtime_path: Utf8PathBuf,
    transcript_path: Utf8PathBuf,
) {
    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => {
            send_failure(
                sender,
                format!("failed waiting for Codex exit: {error}"),
                None,
                Some(transcript_runtime_path),
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
        "codex exited with a non-zero status".to_owned(),
        status.code(),
        Some(transcript_runtime_path),
    );
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
