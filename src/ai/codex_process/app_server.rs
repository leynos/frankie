//! App-server JSON-RPC session management and protocol messages.
//!
//! Handles the `codex app-server` protocol lifecycle: initialization,
//! thread creation, turn dispatch, and completion detection.

use std::io::Write;
use std::process::ChildStdin;

use serde_json::{Value, json};

const INITIALIZE_REQUEST_ID: u64 = 1;
const THREAD_START_REQUEST_ID: u64 = 2;
const TURN_START_REQUEST_ID: u64 = 3;

/// Outcome of an app-server protocol exchange.
pub(super) enum AppServerCompletion {
    Succeeded,
    Failed(String),
}

/// Tracks the state of an active app-server JSON-RPC session.
pub(super) struct AppServerSession {
    prompt: String,
    thread_started: bool,
}

impl AppServerSession {
    pub(super) const fn new(prompt: String) -> Self {
        Self {
            prompt,
            thread_started: false,
        }
    }

    /// Processes a single JSON-RPC message from the app-server.
    ///
    /// Returns `Some(completion)` when the turn has reached a terminal
    /// state, or `None` if the session should continue reading.
    pub(super) fn handle_message(
        &mut self,
        stdin: &mut ChildStdin,
        message: &Value,
    ) -> Result<Option<AppServerCompletion>, String> {
        if let Some(failure) = check_error_responses(message) {
            return Ok(Some(failure));
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

            write_message(stdin, &turn_start_request(thread_id, self.prompt.as_str()))?;
            self.thread_started = true;
        }

        Ok(check_turn_completion(message))
    }
}

/// Attempts to start an app-server session if stdin is available.
///
/// Returns `None` when stdin is unavailable or the handshake fails.
pub(super) fn maybe_start_session(
    maybe_stdin: Option<&mut ChildStdin>,
    prompt: &str,
) -> Option<AppServerSession> {
    let session = AppServerSession::new(prompt.to_owned());
    if let Some(stdin_writer) = maybe_stdin
        && start_protocol(stdin_writer).is_ok()
    {
        return Some(session);
    }

    None
}

/// Dispatches a single line to an active app-server session.
///
/// Returns `None` if no session is active, stdin is missing, or the line
/// is not valid JSON.
pub(super) fn maybe_handle_message(
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

/// Checks whether any expected response carries an error payload.
fn check_error_responses(message: &Value) -> Option<AppServerCompletion> {
    const ERROR_CHECKS: &[(u64, &str)] = &[
        (INITIALIZE_REQUEST_ID, "initialize"),
        (THREAD_START_REQUEST_ID, "thread/start"),
        (TURN_START_REQUEST_ID, "turn/start"),
    ];

    for &(id, label) in ERROR_CHECKS {
        if let Some(error) = response_error_for_id(message, id) {
            return Some(AppServerCompletion::Failed(format!(
                "app-server {label} failed: {error}"
            )));
        }
    }

    None
}

/// Checks whether a `turn/completed` notification has arrived.
fn check_turn_completion(message: &Value) -> Option<AppServerCompletion> {
    if message.get("method").and_then(Value::as_str) != Some("turn/completed") {
        return None;
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

    Some(completion)
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

fn start_protocol(stdin: &mut ChildStdin) -> Result<(), String> {
    write_message(stdin, &initialize_request())?;
    write_message(stdin, &initialized_notification())?;
    write_message(stdin, &thread_start_request())?;
    Ok(())
}

fn write_message(stdin: &mut ChildStdin, message: &Value) -> Result<(), String> {
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
