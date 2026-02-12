//! Process execution helpers for Codex integration.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Sender};

use camino::Utf8PathBuf;
use serde_json::Value;

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

pub(crate) fn build_command_spec(command_path: &str, prompt: &str) -> CodexCommandSpec {
    CodexCommandSpec {
        program: command_path.to_owned(),
        args: vec!["exec".to_owned(), "--json".to_owned(), prompt.to_owned()],
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

fn execute_codex(
    command_path: &str,
    request: &CodexExecutionRequest,
    transcript_path: Utf8PathBuf,
    sender: &Sender<CodexExecutionUpdate>,
) {
    let mut transcript = match TranscriptWriter::create(transcript_path.clone()) {
        Ok(writer) => writer,
        Err(error) => {
            send_failure(
                sender,
                format!("failed to create transcript: {error}"),
                None,
                None,
            );
            return;
        }
    };

    let command_spec = build_command_spec(command_path, &build_prompt(request));
    let Some(mut child) = spawn_child(&command_spec, sender, transcript.path().to_path_buf())
    else {
        return;
    };

    let Some(stdout) = take_stdout(&mut child, sender, transcript.path().to_path_buf()) else {
        return;
    };

    if let Err(error) = stream_progress(stdout, &mut transcript, sender) {
        send_failure(sender, error, None, Some(transcript.path().to_path_buf()));
        return;
    }

    if let Err(error) = transcript.flush() {
        send_failure(
            sender,
            format!("failed to flush transcript: {error}"),
            None,
            Some(transcript.path().to_path_buf()),
        );
        return;
    }

    complete_with_exit_status(
        child,
        sender,
        transcript.path().to_path_buf(),
        transcript_path,
    );
}

fn spawn_child(
    command_spec: &CodexCommandSpec,
    sender: &Sender<CodexExecutionUpdate>,
    transcript_path: Utf8PathBuf,
) -> Option<Child> {
    let mut command = Command::new(&command_spec.program);
    command
        .args(&command_spec.args)
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
    transcript: &mut TranscriptWriter,
    sender: &Sender<CodexExecutionUpdate>,
) -> Result<(), String> {
    for line_result in BufReader::new(stdout).lines() {
        let line = line_result.map_err(|error| format!("failed to read Codex output: {error}"))?;
        transcript
            .append_line(&line)
            .map_err(|error| format!("failed to write transcript: {error}"))?;

        if sender
            .send(CodexExecutionUpdate::Progress(parse_progress_event(&line)))
            .is_err()
        {
            return Ok(());
        }
    }

    Ok(())
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
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("event");

    if let Some(message) = event.get("message").and_then(Value::as_str) {
        return format!("{event_type}: {message}");
    }

    if let Some(text) = event.pointer("/delta/text").and_then(Value::as_str) {
        return format!("{event_type}: {text}");
    }

    format!("event: {event_type}")
}
