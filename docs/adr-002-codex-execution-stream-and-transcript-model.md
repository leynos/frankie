# Architectural decision record (ADR) 002: Codex execution stream and transcript model

## Status

Accepted.

## Date

2026-03-02.

## Context and problem statement

The review TUI must be able to trigger `codex app-server` directly from
filtered comments, display live progress, and preserve machine-readable
execution transcripts for diagnostics.

The solution must keep process execution and stream parsing separate from TUI
state transitions to retain testability and prevent UI concerns from leaking
into domain behaviour.

## Decision outcome / proposed direction

Integrate Codex execution through a dedicated AI service module that runs
`codex app-server` via the JSON-RPC protocol, polls progress updates in the TUI
loop, and writes one JSONL transcript file per run to the local state directory.

## Rationale

1. Boundary clarity: process execution and stream parsing live in `src/ai/` so
   TUI state transitions remain in `src/tui/`.
2. Deterministic persistence: transcript files use a deterministic naming
   pattern `<owner>-<repo>-pr-<number>-<utc-yyyymmddThhmmssZ>.jsonl` under
   `${XDG_STATE_HOME:-$HOME/.local/state}/frankie/codex-transcripts/`.
3. Operational visibility: the TUI status bar shows streamed progress events
   while runs are active and maps non-zero exits into explicit error messages,
   including exit code and transcript path.

## Consequences

- Codex can be launched from the review list view with a single key.
- Transcripts are retained on disk for both successful and failed runs.
- Non-zero Codex exits are surfaced immediately in the interface.

## References

- `docs/frankie-design.md` §5.3.6 (ADR index)
- `src/ai/` (Codex execution and transcript persistence)
- `src/tui/` (polling and UI presentation)
