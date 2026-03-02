# Architectural decision record (ADR) 003: session resumption for interrupted Codex runs

## Status

Accepted.

## Date

2026-03-02.

## Context and problem statement

Codex runs can be interrupted by process crashes, signals, or server-side
interruptions. Progress is lost and the workflow must restart from scratch,
including re-approval of previously accepted actions.

Resumption must be implemented without requiring additional database schema
changes and should preserve server-side approvals whenever possible.

## Decision outcome / proposed direction

Persist session state in JSON sidecar files alongside transcripts and use the
native `thread/resume` JSON-RPC method to reconnect to a prior server-side
thread when an interrupted session is detected.

## Rationale

1. Sidecar file design: each Codex run creates a `.session.json` file alongside
   its `.jsonl` transcript, recording thread ID, pull request context, status,
   and timestamps. Sidecar files are self-contained and do not require database
   changes.
2. Native protocol usage: `thread/resume` is part of the `codex app-server`
   JSON-RPC protocol. Using it avoids re-implementing conversation state
   management and preserves server-side approvals.
3. Thread ID capture: thread IDs from `thread/start` are stored as soon as they
   are received so resumption is possible even when interruption occurs during
   execution.
4. Resume prompt user experience (UX): the resume prompt is shown inline in the
   status bar (`y`/`n`/`Esc`) rather than as a modal dialog to keep the flow
   consistent with the existing TUI key-driven workflow.

## Consequences

- Interrupted runs can be resumed with preserved approvals and conversation
  history.
- Transcript files accumulate content across sessions, separated by
  `--- session resumed ---` markers.
- Session discovery scans sidecar files on disk; no additional database schema
  is required.
- The most recent interrupted session per pull request is offered for
  resumption.

## References

- `docs/frankie-design.md` §5.3.6 (ADR index)
- `src/ai/session` (sidecar session state persistence and discovery)
