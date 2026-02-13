//! Process execution helpers for Codex integration.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Sender};

use camino::{Utf8Path, Utf8PathBuf};
use serde_json::{Value, json};

use crate::ai::codex_exec::{
    CodexExecutionHandle, CodexExecutionOutcome, CodexExecutionRequest, CodexExecutionUpdate,
    CodexProgressEvent,
};

use super::transcript::TranscriptWriter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexCommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

const INITIALIZE_REQUEST_ID: u64 = 1;
const THREAD_START_REQUEST_ID: u64 = 2;
const TURN_START_REQUEST_ID: u64 = 3;

pub(crate) fn build_command_spec(command_path: &str) -> CodexCommandSpec {
    CodexCommandSpec {
        program: command_path.to_owned(),
        args: vec!["app-server".to_owned()],
    }
}

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
    match TranscriptWriter::create(transcript_path.to_path_buf()) {
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

/// Handle the completion outcome from streaming, either from app-server or
/// process exit.
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
        match stream_progress(stdout, stdin, &mut stream_context) {
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
        .stderr(Stdio::null());

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

fn stream_progress(
    stdout: ChildStdout,
    mut stdin: Option<ChildStdin>,
    context: &mut StreamProgressContext<'_>,
) -> Result<StreamCompletion, String> {
    let mut app_server_session = maybe_start_app_server_session(stdin.as_mut(), context.prompt);

    for line_result in BufReader::new(stdout).lines() {
        let line = line_result.map_err(|error| format!("failed to read Codex output: {error}"))?;
        context
            .transcript
            .append_line(&line)
            .map_err(|error| format!("failed to write transcript: {error}"))?;

        if context
            .sender
            .send(CodexExecutionUpdate::Progress(parse_progress_event(&line)))
            .is_err()
        {
            return Ok(StreamCompletion::ProcessExit);
        }

        if let Some(completion) = maybe_handle_app_server_message(
            app_server_session.as_mut(),
            stdin.as_mut(),
            line.as_str(),
        )? {
            return Ok(StreamCompletion::AppServer(completion));
        }
    }

    Ok(StreamCompletion::ProcessExit)
}

struct StreamProgressContext<'a> {
    prompt: &'a str,
    transcript: &'a mut TranscriptWriter,
    sender: &'a Sender<CodexExecutionUpdate>,
}

fn maybe_start_app_server_session(
    maybe_stdin: Option<&mut ChildStdin>,
    prompt: &str,
) -> Option<AppServerSession> {
    let session = AppServerSession::new(prompt.to_owned());
    if let Some(stdin_writer) = maybe_stdin
        && start_app_server_protocol(stdin_writer).is_ok()
    {
        return Some(session);
    }

    None
}

fn maybe_handle_app_server_message(
    maybe_session: Option<&mut AppServerSession>,
    maybe_stdin: Option<&mut ChildStdin>,
    line: &str,
) -> Result<Option<AppServerCompletion>, String> {
    let Some(session) = maybe_session else {
        return Ok(None);
    };

    let Some(stdin) = maybe_stdin else {
        return Ok(None);
    };

    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return Ok(None);
    };

    session.handle_message(stdin, &message)
}

enum StreamCompletion {
    AppServer(AppServerCompletion),
    ProcessExit,
}

enum AppServerCompletion {
    Succeeded,
    Failed(String),
}

struct AppServerSession {
    prompt: String,
    thread_started: bool,
}

impl AppServerSession {
    const fn new(prompt: String) -> Self {
        Self {
            prompt,
            thread_started: false,
        }
    }

    fn handle_message(
        &mut self,
        stdin: &mut ChildStdin,
        message: &Value,
    ) -> Result<Option<AppServerCompletion>, String> {
        if let Some(error) = response_error_for_id(message, INITIALIZE_REQUEST_ID) {
            return Ok(Some(AppServerCompletion::Failed(format!(
                "app-server initialize failed: {error}"
            ))));
        }

        if let Some(error) = response_error_for_id(message, THREAD_START_REQUEST_ID) {
            return Ok(Some(AppServerCompletion::Failed(format!(
                "app-server thread/start failed: {error}"
            ))));
        }

        if let Some(error) = response_error_for_id(message, TURN_START_REQUEST_ID) {
            return Ok(Some(AppServerCompletion::Failed(format!(
                "app-server turn/start failed: {error}"
            ))));
        }

        if !self.thread_started && is_response_for_id(message, THREAD_START_REQUEST_ID) {
            let Some(thread_id) = message
                .pointer("/result/thread/id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
            else {
                return Ok(Some(AppServerCompletion::Failed(
                    "app-server thread/start response did not include thread id".to_owned(),
                )));
            };

            write_app_server_message(stdin, &turn_start_request(thread_id, self.prompt.as_str()))?;
            self.thread_started = true;
        }

        if message.get("method").and_then(Value::as_str) != Some("turn/completed") {
            return Ok(None);
        }

        let status = message
            .pointer("/params/turn/status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        let completion = match status {
            "completed" => AppServerCompletion::Succeeded,
            "failed" | "interrupted" | "cancelled" => {
                AppServerCompletion::Failed(turn_failure_message(message, status))
            }
            _ => AppServerCompletion::Failed(format!(
                "codex turn completed with unexpected status: {status}"
            )),
        };

        Ok(Some(completion))
    }
}

fn is_response_for_id(message: &Value, id: u64) -> bool {
    message.get("id").and_then(Value::as_u64) == Some(id)
}

fn response_error_for_id(message: &Value, id: u64) -> Option<String> {
    if !is_response_for_id(message, id) {
        return None;
    }

    message
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn start_app_server_protocol(stdin: &mut ChildStdin) -> Result<(), String> {
    let initialize = initialize_request();
    let initialized = initialized_notification();
    let thread_start = thread_start_request();
    write_app_server_message(stdin, &initialize)?;
    write_app_server_message(stdin, &initialized)?;
    write_app_server_message(stdin, &thread_start)?;
    Ok(())
}

fn write_app_server_message(stdin: &mut ChildStdin, message: &Value) -> Result<(), String> {
    let mut encoded = serde_json::to_vec(message)
        .map_err(|error| format!("failed to encode app-server request: {error}"))?;
    encoded.push(b'\n');
    stdin
        .write_all(&encoded)
        .map_err(|error| format!("failed writing app-server request: {error}"))?;
    stdin
        .flush()
        .map_err(|error| format!("failed flushing app-server request: {error}"))?;
    Ok(())
}

fn initialize_request() -> Value {
    json!({
        "id": INITIALIZE_REQUEST_ID,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": env!("CARGO_PKG_NAME"),
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn initialized_notification() -> Value {
    json!({
        "method": "initialized",
        "params": {}
    })
}

fn thread_start_request() -> Value {
    json!({
        "id": THREAD_START_REQUEST_ID,
        "method": "thread/start",
        "params": {}
    })
}

fn turn_start_request(thread_id: &str, prompt: &str) -> Value {
    json!({
        "id": TURN_START_REQUEST_ID,
        "method": "turn/start",
        "params": {
            "threadId": thread_id,
            "input": [
                {
                    "type": "text",
                    "text": prompt
                }
            ]
        }
    })
}

fn turn_failure_message(message: &Value, status: &str) -> String {
    message
        .pointer("/params/turn/error/message")
        .and_then(Value::as_str)
        .or_else(|| {
            message
                .pointer("/params/error/message")
                .and_then(Value::as_str)
        })
        .map_or_else(
            || format!("codex turn failed with status: {status}"),
            ToOwned::to_owned,
        )
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

fn terminate_child(child: &mut Child) {
    if let Ok(None) = child.try_wait() {
        let _kill = child.kill();
        let _wait = child.wait();
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

    if let Some(pr_url) = request.pr_url()
        && writeln!(prompt, "Pull request URL: {pr_url}").is_err()
    {
        return prompt;
    }

    prompt.push_str(request.comments_jsonl());
    prompt
}

pub(crate) fn parse_progress_event(line: &str) -> CodexProgressEvent {
    serde_json::from_str::<Value>(line).map_or_else(
        |_| CodexProgressEvent::ParseWarning {
            raw_line: line.to_owned(),
        },
        |json| CodexProgressEvent::Status {
            message: format_json_event(&json),
        },
    )
}

fn format_json_event(event: &Value) -> String {
    if let Some(method) = event.get("method").and_then(Value::as_str) {
        if let Some(text) = event
            .pointer("/params/delta/text")
            .and_then(Value::as_str)
            .or_else(|| event.pointer("/params/delta").and_then(Value::as_str))
        {
            return format!("{method}: {text}");
        }

        return format!("event: {method}");
    }

    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("event");

    if let Some(message) = event.get("message").and_then(Value::as_str) {
        return format!("{event_type}: {message}");
    }

    if let Some(text) = event.pointer("/delta/text").and_then(Value::as_str) {
        return format!("{event_type}: {text}");
    }

    format!("event: {event_type}")
}
