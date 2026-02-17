//! Stdout stream processing and progress event parsing for Codex output.
//!
//! Reads JSON Lines from the Codex process stdout, forwards progress
//! events to the TUI channel, and delegates app-server messages to the
//! session handler.

use std::io::{BufRead, BufReader};
use std::process::{ChildStdin, ChildStdout};
use std::sync::mpsc::Sender;

use serde_json::Value;

use crate::ai::codex_exec::{CodexExecutionUpdate, CodexProgressEvent};
use crate::ai::transcript::TranscriptWriter;

use super::RunError;
use super::app_server::{self, AppServerCompletion};

/// Outcome of the stdout streaming loop.
pub(super) enum StreamCompletion {
    /// The app-server protocol reached a terminal state.
    AppServer(AppServerCompletion),
    /// The process exited (stdout EOF).
    ProcessExit,
}

/// Contextual state threaded through the streaming loop.
pub(super) struct StreamProgressContext<'a> {
    pub(super) prompt: &'a str,
    pub(super) transcript: &'a mut TranscriptWriter,
    pub(super) sender: &'a Sender<CodexExecutionUpdate>,
    /// Thread ID captured from the app-server session after streaming.
    pub(super) thread_id: Option<String>,
}

/// Reads lines from Codex stdout, writing them to the transcript and
/// forwarding parsed progress events to the TUI channel.
///
/// Returns the terminal completion state once the stream ends or the
/// app-server protocol signals completion.
pub(super) fn stream_progress(
    stdout: ChildStdout,
    mut stdin: Option<ChildStdin>,
    context: &mut StreamProgressContext<'_>,
) -> Result<StreamCompletion, RunError> {
    let session = app_server::maybe_start_session(stdin.as_mut(), context.prompt);
    stream_with_session(stdout, stdin, context, session)
}

/// Streams stdout from a resumed session (uses `thread/resume` protocol).
pub(super) fn stream_resume_progress(
    stdout: ChildStdout,
    mut stdin: Option<ChildStdin>,
    context: &mut StreamProgressContext<'_>,
    thread_id: &str,
) -> Result<StreamCompletion, RunError> {
    let session = app_server::maybe_start_resume_session(stdin.as_mut(), context.prompt, thread_id);
    stream_with_session(stdout, stdin, context, session)
}

/// Common streaming workflow shared by fresh and resumed sessions.
fn stream_with_session(
    stdout: ChildStdout,
    mut stdin: Option<ChildStdin>,
    context: &mut StreamProgressContext<'_>,
    mut session: Option<app_server::AppServerSession>,
) -> Result<StreamCompletion, RunError> {
    let completion = read_stream_lines(stdout, &mut stdin, context, &mut session)?;
    capture_thread_id(context, session.as_ref());
    Ok(completion)
}

fn read_stream_lines(
    stdout: ChildStdout,
    stdin: &mut Option<ChildStdin>,
    context: &mut StreamProgressContext<'_>,
    session: &mut Option<app_server::AppServerSession>,
) -> Result<StreamCompletion, RunError> {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    while let Some(line_result) = lines.next() {
        let line = line_result
            .map_err(|error| RunError::new(format!("failed to read Codex output: {error}")))?;
        context
            .transcript
            .append_line(&line)
            .map_err(|error| RunError::new(format!("failed to write transcript: {error}")))?;

        if context
            .sender
            .send(CodexExecutionUpdate::Progress(parse_progress_event(&line)))
            .is_err()
        {
            drain_remaining_lines(lines);
            return Ok(StreamCompletion::ProcessExit);
        }

        if let Some(completion) =
            app_server::maybe_handle_message(session.as_mut(), stdin.as_mut(), line.as_str())?
        {
            return Ok(StreamCompletion::AppServer(completion));
        }
    }

    Ok(StreamCompletion::ProcessExit)
}

fn capture_thread_id(
    context: &mut StreamProgressContext<'_>,
    session: Option<&app_server::AppServerSession>,
) {
    if let Some(sess) = session {
        context.thread_id = sess.thread_id().map(ToOwned::to_owned);
    }
}

/// Consumes remaining stdout lines so the child process does not block
/// on a full pipe when `wait()` is called.
fn drain_remaining_lines(lines: std::io::Lines<BufReader<ChildStdout>>) {
    for line in lines {
        if line.is_err() {
            break;
        }
    }
}

/// Parses a raw output line into a progress event for the TUI.
///
/// Lines that are valid JSON are formatted as structured status
/// messages. Non-JSON lines produce a [`CodexProgressEvent::ParseWarning`].
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
