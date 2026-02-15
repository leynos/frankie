# Enable session resumption for interrupted Codex runs

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

When a Codex run is interrupted (process crash, network failure, user
cancellation, or the `turn/completed` notification carries status `interrupted`
or `cancelled`), the transcript and any accumulated approvals are currently
lost. The user must restart from scratch, re-entering the same comments and
losing prior context.

After this change, a user who presses `x` in the review TUI will be offered the
option to resume the most recent interrupted run for the same pull request.
Resuming reuses the prior transcript (replaying it as context for Codex),
preserves any approvals recorded in the transcript, and avoids redundant work.
If no interrupted session exists, pressing `x` behaves exactly as before,
starting a fresh run.

Success is observable when:

- A user interrupts a Codex run (e.g. via the TUI or process signal), then
  presses `x` again, and the TUI offers to resume the previous session.
- Accepting the resume prompt causes Codex to receive the prior transcript as
  context and continue where it left off.
- The prior transcript file is appended to (not overwritten) during
  resumption.
- Approvals recorded in the prior transcript are preserved and not
  re-requested.
- Declining the resume prompt starts a fresh session as before.
- Running `make check-fmt`, `make lint`, and `make test` all pass.

## Constraints

- Preserve the existing `CodexExecutionService` trait boundary: TUI code in
  `src/tui/` must not reach into process management in `src/ai/codex_process/`.
- Reuse existing transcript infrastructure (`TranscriptWriter`,
  `TranscriptMetadata`, `transcript_path()`) rather than duplicating
  persistence logic.
- Maintain the model-view-update (MVU) boundary: TUI state transitions stay
  in `src/tui/`, session discovery and resumption logic stays in `src/ai/`.
- Every new Rust module must start with a `//!` module-level comment.
- No file may exceed 400 lines; split when needed.
- Use capability-oriented filesystem access (`cap_std::fs_utf8`, `camino`)
  for all file operations.
- Existing operation modes, export behaviour, and key bindings must not
  change (except the `x` key, which gains resume-prompt behaviour when an
  interrupted session is detected).
- Unit tests must use `rstest`; behavioural tests must use `rstest-bdd`
  v0.5.0 and cover happy/unhappy paths plus relevant edge cases.
- Update `docs/frankie-design.md` with design decisions made for this
  feature.
- Update `docs/users-guide.md` with user-visible interaction changes.
- Mark the roadmap entry done in `docs/roadmap.md` only after implementation
  and all validation gates pass.
- Validation gates must pass before completion:
  `make check-fmt`, `make lint`, and `make test`.
- Use en-GB-oxendict spelling and grammar in comments, documentation, and
  commit messages.
- Clippy warnings must be denied. No `unwrap()` or `expect()` in production
  code.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 25 files or more than
  1,500 net new lines, stop and escalate.
- Interface: if the public `CodexExecutionService` trait signature must
  change in a breaking way (i.e. existing callers cannot compile without
  modification), stop and escalate. Additive changes (new methods with default
  implementations, new types) are acceptable.
- Dependencies: if any new external dependency is required beyond what is
  already declared in `Cargo.toml`, stop and escalate.
- Iterations: if tests still fail after three fix cycles on any stage, stop
  and escalate with logs.
- Ambiguity: if the Codex `app-server` protocol does not support replaying
  prior transcript content (i.e. there is no way to pass prior context on
  resume), stop and escalate with a description of the limitation and proposed
  alternatives.

## Risks

- Risk: Codex `app-server` JSON-RPC protocol may not support injecting prior
  transcript content into a new session for context continuity. Severity: high.
  Likelihood: medium. Mitigation: the design uses the existing `turn/start`
  input mechanism to prepend prior transcript events as context text in the
  prompt. If the prompt-based approach proves insufficient (e.g. token limits),
  escalate before implementing alternatives.

- Risk: transcript file locking — multiple concurrent Codex runs for the
  same PR could create conflicting session state files. Severity: medium.
  Likelihood: low. Mitigation: the existing guard (`is_codex_running()`)
  prevents concurrent runs in the same TUI session. Session state files use
  unique timestamps. Document that cross-session concurrency is not supported.

- Risk: large transcripts may exceed prompt size limits when replayed.
  Severity: medium. Likelihood: medium. Mitigation: implement a configurable
  maximum replay size (default 64 KiB of transcript text). If the transcript
  exceeds this limit, truncate from the beginning (keeping most recent context)
  and warn the user.

- Risk: interrupted session detection relies on filesystem state that may be
  stale or corrupted. Severity: low. Likelihood: low. Mitigation: validate
  session state file integrity on read; discard invalid or unparseable session
  files silently and treat as "no interrupted session".

## Progress

- [ ] Drafted self-contained ExecPlan with constraints, tolerances, staged
      implementation, and validation approach.
- [ ] Stage A: session state model and persistence layer in `src/ai/`.
- [ ] Stage B: session discovery (find most recent interrupted session for a
      PR) and resumption logic.
- [ ] Stage C: extend `CodexExecutionService` trait and
      `SystemCodexExecutionService` for resume support.
- [ ] Stage D: TUI integration — resume prompt on `x` key when interrupted
      session exists.
- [ ] Stage E: unit tests for session state persistence, discovery, and
      resumption.
- [ ] Stage F: behavioural tests (BDD) for end-to-end resume and fresh-start
      flows.
- [ ] Stage G: documentation updates (`frankie-design.md`, `users-guide.md`,
      `roadmap.md`).
- [ ] Stage H: quality gates pass (`make check-fmt`, `make lint`,
      `make test`).

## Surprises & discoveries

(None yet — to be populated during implementation.)

## Decision log

- Decision: represent session state as a small JSON sidecar file alongside
  each transcript, rather than a SQLite table or in-memory-only state.
  Rationale: transcripts are already written to the filesystem under
  `${XDG_STATE_HOME}/frankie/codex-transcripts/`. A sidecar
  `<transcript-name>.session.json` keeps session metadata co-located with
  transcript data, avoids schema migration complexity, and is inspectable by
  users. The existing `cap_std` filesystem primitives can create, read, and
  update sidecar files with no new dependencies. Date/Author: 2026-02-15 / plan
  author.

- Decision: use prompt-based context injection for resumption rather than
  Codex-native session IDs. Rationale: the Codex `app-server` protocol does not
  expose a "resume session" RPC. Instead, the prior transcript content
  (filtered to substantive events) is prepended to the new turn's prompt text
  inside the `turn/start` `input` payload. This is analogous to
  `codex exec resume` which replays conversation context. This approach
  requires no protocol changes and keeps the integration within Frankie's
  control. If Codex later exposes a native resume API, the sidecar metadata
  will still be useful for locating the relevant transcript. Date/Author:
  2026-02-15 / plan author.

- Decision: detect "interrupted" status from both process-level signals
  (non-zero exit, channel disconnect) and protocol-level signals
  (`turn/completed` with status `interrupted`, `cancelled`, or `failed`).
  Rationale: the existing `app_server.rs` already classifies these statuses in
  `check_turn_completion()`. The session state writer hooks into the existing
  outcome pipeline, recording the terminal status. Date/Author: 2026-02-15 /
  plan author.

- Decision: limit replayed transcript context to 64 KiB by default to
  prevent prompt overflow. Rationale: Codex models have finite context windows.
  Replaying an unbounded transcript risks silent truncation by the model or
  outright rejection. A 64 KiB ceiling keeps context manageable while
  preserving the most recent (and most relevant) execution history. Truncation
  is applied from the beginning, keeping the tail. Date/Author: 2026-02-15 /
  plan author.

- Decision: the TUI resume prompt is synchronous and non-modal — it appears
  as a status bar question with `y/n` key bindings, avoiding a new modal
  overlay. Rationale: consistency with existing TUI patterns (status bar is
  already used for Codex progress and errors). Adding a modal dialog would
  require new view mode plumbing and is over-engineered for a binary yes/no
  decision. Date/Author: 2026-02-15 / plan author.

## Outcomes & retrospective

(To be completed after implementation.)

## Context and orientation

### Codebase layout

The Codex execution integration was implemented in ExecPlan 3-1-1 and lives in
these files:

**AI module** (`src/ai/`):

- `src/ai/mod.rs` — module exports.
- `src/ai/codex_exec.rs` — core types (`CodexExecutionContext`,
  `CodexExecutionRequest`, `CodexProgressEvent`, `CodexExecutionOutcome`,
  `CodexExecutionUpdate`, `CodexExecutionHandle`) and the
  `CodexExecutionService` trait with its `SystemCodexExecutionService`
  implementation. 254 lines.
- `src/ai/codex_exec_tests.rs` — unit tests for execution types and
  end-to-end scripted process tests. 256 lines.
- `src/ai/transcript.rs` — `TranscriptMetadata`, `TranscriptWriter`,
  `transcript_path()`, and `resolve_transcript_base_dir()`. 316 lines.
- `src/ai/codex_process/mod.rs` — process lifecycle: `run_codex()` spawns
  a background thread, `execute_codex()` orchestrates spawn → stream → outcome.
  380 lines.
- `src/ai/codex_process/stream.rs` — `stream_progress()` loop and
  `parse_progress_event()`. 127 lines.
- `src/ai/codex_process/app_server.rs` — JSON-RPC session state machine
  (`AppServerSession`, `start_protocol`, `handle_message`,
  `check_turn_completion`). 246 lines.

**TUI integration** (`src/tui/`):

- `src/tui/messages.rs` — `AppMsg` enum including `StartCodexExecution`,
  `CodexPollTick`, `CodexProgress`, `CodexFinished`. 192 lines.
- `src/tui/input.rs` — maps `x` key to `StartCodexExecution` in
  `ReviewList` context.
- `src/tui/app/mod.rs` — `ReviewApp` struct with `codex_service`,
  `codex_handle`, `codex_status`, `codex_poll_interval` fields. 404 lines.
- `src/tui/app/codex_handlers.rs` — `handle_codex_msg`,
  `handle_start_codex_execution`, `handle_codex_poll_tick`,
  `drain_codex_updates`, `apply_codex_outcome`, `build_codex_request`,
  `arm_codex_poll_timer`. 183 lines.
- `src/tui/app/codex_handlers_tests.rs` — handler unit tests with stub
  service.

**Behavioural tests**:

- `tests/features/codex_exec.feature` — four scenarios covering success,
  non-zero exit, malformed stream, and transcript failure.
- `tests/codex_exec_bdd.rs` — step definitions and scenario bindings.
- `tests/codex_exec_bdd/state.rs` — `CodexExecState`, `StubPlan`,
  `StubCodexExecutionService`, `app_with_plan()`.

### Key types and flow

1. TUI user presses `x` → `AppMsg::StartCodexExecution`.
2. `handle_start_codex_execution()` builds a `CodexExecutionRequest` from
   filtered comments and calls `codex_service.start(request)`.
3. `SystemCodexExecutionService::start()` resolves the transcript path,
   then calls `run_codex()` which spawns a background thread.
4. The background thread creates a `TranscriptWriter`, spawns
   `codex app-server`, writes JSON-RPC initialisation messages to stdin, and
   enters `stream_progress()` which reads stdout lines, writes each to the
   transcript, sends `CodexExecutionUpdate::Progress` to the channel, and
   delegates JSON-RPC responses to `AppServerSession`.
5. When `turn/completed` arrives or the process exits, a
   `CodexExecutionUpdate::Finished` is sent through the channel.
6. The TUI polls the handle on `CodexPollTick` messages and drains updates.

### Transcript storage

Transcripts are written to:
`${XDG_STATE_HOME:-$HOME/.local/state}/frankie/codex-transcripts/`

Filename pattern: `<owner>-<repo>-pr-<number>-<yyyymmddThhmmssZ>.jsonl`

### Error type

All errors flow through `IntakeError` (defined in `src/github/error.rs`), a
`thiserror`-derived enum.

### Terms

- **Session**: a single Codex execution run, identified by its transcript
  file path and associated metadata.
- **Interrupted session**: a session whose outcome was `Failed`,
  `interrupted`, or `cancelled` (as opposed to `Succeeded`).
- **Sidecar file**: a `.session.json` file stored alongside a transcript
  `.jsonl` file, containing session metadata (status, PR context, approvals,
  timestamp).
- **Resume**: starting a new Codex `app-server` process but injecting the
  prior transcript content into the prompt to provide continuity.

## Plan of work

### Stage A: Session state model and persistence

Define a `SessionState` struct and its JSON sidecar persistence. This stage
adds no new behaviour — only data types and file I/O.

Create `src/ai/session.rs` (new file, module-level doc comment required):

1. Define `SessionStatus` enum: `Running`, `Completed`, `Interrupted`,
   `Failed`, `Cancelled`.
2. Define `SessionState` struct: `status: SessionStatus`,
   `transcript_path: Utf8PathBuf`, `owner: String`, `repository: String`,
   `pr_number: u64`, `started_at: DateTime<Utc>`,
   `finished_at: Option<DateTime<Utc>>`, `approvals: Vec<String>` (approval
   event summaries extracted from the transcript).
3. Derive `Serialize`, `Deserialize` for both types using `serde`.
4. Implement `SessionState::sidecar_path(&self) -> Utf8PathBuf` — replaces
   the transcript file's `.jsonl` extension with `.session.json`.
5. Implement `SessionState::write_sidecar(&self) -> Result<(), IntakeError>`
   — writes the JSON sidecar file using `cap_std` filesystem primitives.
6. Implement `SessionState::read_sidecar(path: &Utf8Path) -> Result<Self,
   IntakeError>` — reads and deserialises a sidecar file.
7. Add `serde` and `serde_json` to the import list (both are already in
   `Cargo.toml`).

Add to `src/ai/mod.rs`:

1. Declare `pub mod session;` and re-export key types.

Validation: `make check-fmt && make lint` pass. Unit tests in Stage E.

### Stage B: Session lifecycle hooks

Wire session state creation and updates into the existing execution pipeline so
that every Codex run produces a sidecar file recording its terminal status.

In `src/ai/codex_process/mod.rs`:

1. After `TranscriptWriter::create()` succeeds in `execute_codex()`, create
   a `SessionState` with status `Running` and write its sidecar.
2. Before each `send_failure()` call and before each successful
   `Finished` send, update the session state to the appropriate terminal status
   (`Interrupted`, `Failed`, `Cancelled`, or `Completed`) and rewrite the
   sidecar.
3. Extract approval events from the transcript content. An "approval" is a
   JSON line where the `method` field equals `"turn/approved"` or the `type`
   field equals `"approval"`. Collect these as summary strings in
   `SessionState.approvals`.

Implement a helper `fn update_session_status(state, status)
-> Result<(), IntakeError>` that updates the status, sets `finished_at`,
and writes the sidecar.

Map existing outcome types to session statuses:

- `AppServerCompletion::Succeeded` → `SessionStatus::Completed`
- `AppServerCompletion::Failed(msg)` where the message or the
  `turn/completed` status was `"interrupted"` → `SessionStatus::Interrupted`
- `AppServerCompletion::Failed(msg)` where status was `"cancelled"` →
  `SessionStatus::Cancelled`
- All other failures → `SessionStatus::Failed`
- Channel disconnect → `SessionStatus::Interrupted`

To distinguish interrupted from other failures, extend `AppServerCompletion`
with a new variant or add an `interrupted: bool` field to `Failed`. The
cleanest approach is to refine `AppServerCompletion::Failed` into
`Failed { message: String, interrupted: bool }` so the downstream code can
branch without string matching.

Validation: `make check-fmt && make lint` pass. No new tests yet (Stage E).

### Stage C: Session discovery

Implement the ability to find the most recent interrupted session for a given
PR.

In `src/ai/session.rs`:

1. Implement `fn find_interrupted_session(base_dir: &Utf8Path, owner: &str,
   repository: &str, pr_number: u64) -> Result<Option<SessionState>,
   IntakeError>`:
   - List all `.session.json` files in `base_dir`.
   - Filter to those matching the owner, repository, and PR number.
   - Filter to those with status `Interrupted` (not `Failed`, `Cancelled`,
     or `Completed`).
   - Sort by `started_at` descending.
   - Return the most recent match, or `None`.
2. Use `cap_std::fs_utf8::Dir` for directory listing (consistent with
   existing transcript code).

Validation: unit tests in Stage E.

### Stage D: Resume execution path

Extend `CodexExecutionService` and the process layer to support resumption.

In `src/ai/codex_exec.rs`:

1. Add a new `CodexResumeRequest` struct containing:
   - `session: SessionState` — the interrupted session to resume.
   - `new_comments_jsonl: String` — current comments (may have changed since
     the interrupted run).
   - `pr_url: Option<String>`.
   - `max_replay_bytes: usize` — maximum transcript bytes to replay
     (default 65,536).
2. Add a new method to the `CodexExecutionService` trait:

       fn resume(
           &self,
           request: CodexResumeRequest,
       ) -> Result<CodexExecutionHandle, IntakeError>;

   Provide a default implementation that returns
   `Err(IntakeError::Configuration { message: "resume not supported" })` so
   existing implementors (including test stubs) are not broken.

3. Implement `resume()` on `SystemCodexExecutionService`:
   - Read the prior transcript file content (up to `max_replay_bytes` from
     the tail).
   - Build a prompt that includes:
     a. A preamble: "This is a resumed session. The prior transcript follows
        for context. Preserved approvals: [list]. Continue resolving
        comments."
     b. The tail of the prior transcript content.
     c. The current comments JSONL.
   - Call `run_codex()` with a transcript path that appends to the existing
     transcript file (rather than creating a new one). This requires a small
     change to `TranscriptWriter` to support an `open_append` mode in
     addition to `create`.

In `src/ai/transcript.rs`:

1. Add `TranscriptWriter::open_append(path)` that opens an
   existing file for appending rather than creating a new one.
   Write a separator line (`--- session resumed ---`) to mark
   the resumption boundary in the transcript.
2. Add `read_transcript_tail(path, max_bytes)` that reads up to
   `max_bytes` from the end of a transcript file. If the file
   is smaller than `max_bytes`, return the entire content.

In `src/ai/codex_process/mod.rs`:

1. Add a `run_codex_resume()` function (or parameterise `run_codex()`) that
   uses `TranscriptWriter::open_append()` instead of `create()`, and passes the
   resume-augmented prompt.

Validation: `make check-fmt && make lint` pass.

### Stage E: TUI integration

Wire the resume flow into the TUI's `x` key handler.

In `src/tui/messages.rs`:

1. Add new `AppMsg` variants:
   - `ResumePromptShown` — displayed when an interrupted session is
     detected. Carries the `SessionState`.
   - `ResumeAccepted(SessionState)` — user chose to resume.
   - `ResumeDeclined` — user chose to start fresh.

In `src/tui/input.rs`:

1. When in the `ResumePrompt` input context (new variant of
   `InputContext`), map `y` → `ResumeAccepted`, `n` → `ResumeDeclined`, `Esc` →
   `ResumeDeclined`.

In `src/tui/app/mod.rs`:

1. Add a `resume_prompt: Option<SessionState>` field to `ReviewApp` to
   track when a resume prompt is active.
2. Update `ViewMode` with a `ResumePrompt` variant (or use the existing
   `ReviewList` mode with a sub-state flag, depending on which is cleaner —
   decide during implementation).

In `src/tui/app/codex_handlers.rs`:

1. Modify `handle_start_codex_execution()`:
   - Before starting a fresh run, call
     `find_interrupted_session()` to check for a resumable session.
   - If found, set `self.resume_prompt = Some(session)` and update the
     status bar to show "Interrupted session found. Resume? (y/n)".
   - Return without starting execution (wait for user response).
   - If no interrupted session, proceed as before.

2. Add `handle_resume_accepted(session: SessionState)`:
   - Build a `CodexResumeRequest` from the session and current comments.
   - Call `codex_service.resume(request)`.
   - Store handle, arm poll timer (same as fresh start).
   - Clear `resume_prompt`.

3. Add `handle_resume_declined()`:
   - Clear `resume_prompt`.
   - Proceed with fresh execution (call the existing fresh-start logic).

4. Extend `handle_codex_msg()` to route `ResumeAccepted` and
   `ResumeDeclined` messages.

In `src/tui/app/rendering.rs`:

1. When `resume_prompt` is `Some`, render the resume prompt text in the
   status bar area: "Interrupted session from \<timestamp\>. Resume? [y/n]".

Validation: `make check-fmt && make lint` pass.

### Stage F: Unit tests

Add unit tests using `rstest` for all new functionality.

In `src/ai/session.rs` (inline `#[cfg(test)]` module):

1. `session_state_sidecar_path_replaces_extension` — verify `.jsonl` →
   `.session.json`.
2. `session_state_write_and_read_roundtrip` — write then read a sidecar,
   assert equality.
3. `session_state_read_invalid_json_returns_error` — corrupted sidecar.
4. `find_interrupted_session_returns_most_recent` — multiple sidecar files,
   only the most recent interrupted one is returned.
5. `find_interrupted_session_ignores_completed` — completed sessions are
   not returned.
6. `find_interrupted_session_returns_none_when_empty` — no sessions.

In `src/ai/transcript.rs` (extend existing test module):

1. `transcript_writer_open_append_adds_to_existing_file` — verify append
   mode writes a separator and new content.
2. `read_transcript_tail_returns_full_content_when_small` — file smaller
   than limit.
3. `read_transcript_tail_truncates_from_beginning` — file larger than
   limit.

In `src/ai/codex_exec_tests.rs` (extend existing test module):

1. `resume_request_rejects_empty_comments` — validation.
2. `resume_request_builds_prompt_with_prior_context` — verify prompt
    structure includes transcript tail and approvals.

In `src/tui/app/codex_handlers_tests.rs` (extend existing test module):

1. `start_codex_shows_resume_prompt_when_interrupted_session_exists` —
    verify prompt appears.
2. `resume_accepted_starts_resumed_execution` — verify handle is set.
3. `resume_declined_starts_fresh_execution` — verify fresh start.
4. `no_resume_prompt_when_no_interrupted_session` — verify direct start.

### Stage G: Behavioural tests (BDD)

Add `rstest-bdd` v0.5.0 scenarios for session resumption.

Create `tests/features/codex_session_resume.feature`:

    Feature: Codex session resumption

      Scenario: Resume prompt is shown for interrupted session
        Given an interrupted Codex session exists for the current PR
        When the user presses x to start Codex execution
        Then the status bar shows a resume prompt

      Scenario: Accepting resume reuses prior transcript
        Given an interrupted Codex session exists for the current PR
        When the user presses x to start Codex execution
        And the user accepts the resume prompt
        And I wait 200 milliseconds
        And the Codex poll tick is processed
        Then the status bar contains "Codex execution completed"
        And the transcript file contains "session resumed"
        And no TUI error is shown

      Scenario: Declining resume starts fresh session
        Given an interrupted Codex session exists for the current PR
        When the user presses x to start Codex execution
        And the user declines the resume prompt
        And I wait 200 milliseconds
        And the Codex poll tick is processed
        Then the status bar contains "Codex execution completed"
        And no TUI error is shown

      Scenario: No resume prompt when no interrupted session exists
        Given no interrupted Codex session exists
        When the user presses x to start Codex execution
        And I wait 200 milliseconds
        And the Codex poll tick is processed
        Then the status bar contains "Codex execution completed"
        And no TUI error is shown

      Scenario: Interrupted run creates session state file
        Given a Codex run that is interrupted mid-execution
        When Codex execution is started from the review TUI
        And I wait 200 milliseconds
        And the Codex poll tick is processed
        Then the session state file exists
        And the session state file shows status interrupted

Create `tests/codex_session_resume_bdd.rs` with step definitions and scenario
bindings following the pattern in `tests/codex_exec_bdd.rs`.

Extend the stub infrastructure in `tests/codex_exec_bdd/state.rs`:

- Add `StubPlan::ResumeUpdates` variant for resume scenarios.
- Implement `resume()` on `StubCodexExecutionService`.

### Stage H: Documentation

Update `docs/frankie-design.md`:

1. Add an Architecture Decision Record (ADR) entry documenting:
   - Session state sidecar file design.
   - Prompt-based context injection for resumption.
   - Transcript tail replay with size limits.
   - Resume prompt UX decision (status bar y/n, not modal).

Update `docs/users-guide.md`:

1. Under "Codex execution from the TUI", add a "Session resumption"
   subsection explaining:
   - When a resume prompt appears (after an interrupted run for the same PR).
   - How to accept (`y`) or decline (`n`/`Esc`).
   - What resumption does (reuses prior transcript context, preserves
     approvals, appends to existing transcript file).
   - Where session state files are stored (alongside transcripts).

Update `docs/roadmap.md`:

1. Mark the "Enable session resumption for interrupted Codex runs" item
   as `[x]` (done).

### Stage I: Quality gates and close-out

Run all quality gates from repository root:

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/execplan-3-1-2-check-fmt.log
    make lint 2>&1 | tee /tmp/execplan-3-1-2-lint.log
    make test 2>&1 | tee /tmp/execplan-3-1-2-test.log

Expected: each command exits 0 with no lint warnings promoted to errors and no
failing tests.

## Concrete steps

All commands are run from the repository root (`/home/user/project`).

### Commands: Stage A — session state model

1. Create `src/ai/session.rs` with `SessionStatus`, `SessionState`, sidecar
   read/write methods.
2. Update `src/ai/mod.rs` to export the new module.
3. Run:

       set -o pipefail; make check-fmt 2>&1 | tee /tmp/stage-a-fmt.log
       set -o pipefail; make lint 2>&1 | tee /tmp/stage-a-lint.log

   Expected: both exit 0.

### Commands: Stage B — session lifecycle hooks

1. Extend `AppServerCompletion::Failed` in
   `src/ai/codex_process/app_server.rs` with an `interrupted` field.
2. Update `check_turn_completion()` to set `interrupted: true` when status
   is `"interrupted"` or `"cancelled"`.
3. Update `execute_codex()` in `src/ai/codex_process/mod.rs` to create and
   update `SessionState` at process start and on each terminal outcome.
4. Run:

       set -o pipefail; make check-fmt 2>&1 | tee /tmp/stage-b-fmt.log
       set -o pipefail; make lint 2>&1 | tee /tmp/stage-b-lint.log

   Expected: both exit 0.

### Commands: Stage C — session discovery

1. Implement `find_interrupted_session()` in `src/ai/session.rs`.
2. Run:

       set -o pipefail; make check-fmt 2>&1 | tee /tmp/stage-c-fmt.log
       set -o pipefail; make lint 2>&1 | tee /tmp/stage-c-lint.log

   Expected: both exit 0.

### Commands: Stage D — resume execution path

1. Define `CodexResumeRequest` in `src/ai/codex_exec.rs`.
2. Add `resume()` to `CodexExecutionService` with default implementation.
3. Implement `resume()` on `SystemCodexExecutionService`.
4. Add `TranscriptWriter::open_append()` and `read_transcript_tail()` to
   `src/ai/transcript.rs`.
5. Add `run_codex_resume()` (or parameterise `run_codex()`) in
   `src/ai/codex_process/mod.rs`.
6. Run:

       set -o pipefail; make check-fmt 2>&1 | tee /tmp/stage-d-fmt.log
       set -o pipefail; make lint 2>&1 | tee /tmp/stage-d-lint.log

   Expected: both exit 0.

### Commands: Stage E — TUI integration

1. Add `ResumePromptShown`, `ResumeAccepted`, `ResumeDeclined` to
   `AppMsg` in `src/tui/messages.rs`.
2. Add `ResumePrompt` input context and key mappings in
   `src/tui/input.rs`.
3. Add `resume_prompt` field to `ReviewApp` in `src/tui/app/mod.rs`.
4. Modify `handle_start_codex_execution()` and add resume handlers in
   `src/tui/app/codex_handlers.rs`.
5. Update rendering in `src/tui/app/rendering.rs`.
6. Run:

       set -o pipefail; make check-fmt 2>&1 | tee /tmp/stage-e-fmt.log
       set -o pipefail; make lint 2>&1 | tee /tmp/stage-e-lint.log

   Expected: both exit 0.

### Commands: Stage F — unit tests

1. Add tests to `src/ai/session.rs`, `src/ai/transcript.rs`,
   `src/ai/codex_exec_tests.rs`, and `src/tui/app/codex_handlers_tests.rs`.
2. Run:

       set -o pipefail; make test 2>&1 | tee /tmp/stage-f-test.log

   Expected: exit 0, all tests pass.

### Commands: Stage G — behavioural tests

1. Create `tests/features/codex_session_resume.feature`.
2. Create `tests/codex_session_resume_bdd.rs`.
3. Extend `tests/codex_exec_bdd/state.rs` with resume stub support.
4. Run:

       set -o pipefail; make test 2>&1 | tee /tmp/stage-g-test.log

   Expected: exit 0, all tests pass (including new BDD scenarios).

### Commands: Stage H — documentation

1. Update `docs/frankie-design.md` with ADR.
2. Update `docs/users-guide.md` with session resumption section.
3. Update `docs/roadmap.md` — mark item `[x]`.
4. Run:

       make markdownlint

   Expected: exit 0.

### Commands: Stage I — quality gates

1. Run:

       set -o pipefail; make check-fmt 2>&1 | tee /tmp/execplan-3-1-2-check-fmt.log
       set -o pipefail; make lint 2>&1 | tee /tmp/execplan-3-1-2-lint.log
       set -o pipefail; make test 2>&1 | tee /tmp/execplan-3-1-2-test.log

   Expected: all exit 0.

## Validation and acceptance

Behavioural acceptance checks:

- Pressing `x` when an interrupted session exists for the current PR shows
  a resume prompt in the status bar.
- Pressing `y` on the resume prompt starts a resumed Codex execution that
  injects prior transcript context into the prompt.
- Pressing `n` or `Esc` on the resume prompt starts a fresh Codex execution
  (existing behaviour).
- Pressing `x` when no interrupted session exists starts a fresh execution
  directly (existing behaviour unchanged).
- An interrupted Codex run creates a `.session.json` sidecar file alongside
  the transcript recording the interrupted status.
- A successfully completed run creates a sidecar file with status
  `Completed`.
- The resumed execution appends to the existing transcript file (the file
  contains both the original and resumed content, separated by a marker).
- Approvals from the prior transcript are listed in the resume prompt.

Test acceptance checks:

- Unit tests use `rstest` and cover:
  - Session state serialisation roundtrip.
  - Sidecar read/write including error paths.
  - Interrupted session discovery with multiple candidates.
  - Transcript tail reading with size limits.
  - Resume prompt logic in TUI handlers.
- Behavioural tests use `rstest-bdd` v0.5.0 and cover:
  - Resume prompt shown for interrupted session.
  - Resume accepted reuses prior transcript.
  - Resume declined starts fresh.
  - No prompt when no interrupted session.
  - Interrupted run creates session state file.

Required quality-gate commands (from repository root):

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/execplan-3-1-2-check-fmt.log
    set -o pipefail; make lint 2>&1 | tee /tmp/execplan-3-1-2-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/execplan-3-1-2-test.log

Expected results:

- Each command exits 0.
- Logs show no lint warnings promoted to errors and no failing tests.

## Idempotence and recovery

- Session sidecar creation uses `cap_std` file creation which is safe to
  repeat (overwrites with latest state).
- `TranscriptWriter::open_append()` opens for append, so re-running after a
  partial failure does not lose prior content.
- `find_interrupted_session()` is a read-only scan and can be called
  repeatedly without side effects.
- Tests use `TempDir` fixtures that clean up automatically.
- If any stage fails partway through, the sidecar file for the failed run
  records the failure status, and the next invocation treats it correctly (not
  resumable unless status is `Interrupted`).

## Artefacts and notes

Implementation should preserve concise evidence for reviewers:

- A sample `.session.json` sidecar file (contents sanitised).
- A transcript file showing the "--- session resumed ---" separator.
- Test output snippets proving resume prompt logic.
- Final gate logs from `/tmp/execplan-3-1-2-*.log`.

## Interfaces and dependencies

### New types in `src/ai/session.rs`

    /// Status of a Codex execution session.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum SessionStatus {
        Running,
        Completed,
        Interrupted,
        Failed,
        Cancelled,
    }

    /// Persistent state for a Codex execution session.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct SessionState {
        pub status: SessionStatus,
        pub transcript_path: Utf8PathBuf,
        pub owner: String,
        pub repository: String,
        pub pr_number: u64,
        pub started_at: DateTime<Utc>,
        pub finished_at: Option<DateTime<Utc>>,
        pub approvals: Vec<String>,
    }

### New type in `src/ai/codex_exec.rs`

    /// Request payload for resuming an interrupted Codex session.
    #[derive(Debug, Clone)]
    pub struct CodexResumeRequest {
        pub session: SessionState,
        pub new_comments_jsonl: String,
        pub pr_url: Option<String>,
        pub max_replay_bytes: usize,
    }

### Extended trait in `src/ai/codex_exec.rs`

    pub trait CodexExecutionService: Send + Sync + std::fmt::Debug {
        fn start(
            &self,
            request: CodexExecutionRequest,
        ) -> Result<CodexExecutionHandle, IntakeError>;

        fn resume(
            &self,
            request: CodexResumeRequest,
        ) -> Result<CodexExecutionHandle, IntakeError> {
            let _ = request;
            Err(IntakeError::Configuration {
                message: "resume not supported by this service".to_owned(),
            })
        }
    }

### New function in `src/ai/session.rs`

    pub fn find_interrupted_session(
        base_dir: &Utf8Path,
        owner: &str,
        repository: &str,
        pr_number: u64,
    ) -> Result<Option<SessionState>, IntakeError>

### New methods in `src/ai/transcript.rs`

    impl TranscriptWriter {
        pub fn open_append(path: &Utf8Path) -> Result<Self, IntakeError>;
    }

    pub fn read_transcript_tail(
        path: &Utf8Path,
        max_bytes: usize,
    ) -> Result<String, IntakeError>

### New `AppMsg` variants in `src/tui/messages.rs`

    ResumePromptShown(Box<SessionState>),
    ResumeAccepted(Box<SessionState>),
    ResumeDeclined,

### Extended `AppServerCompletion` in `src/ai/codex_process/app_server.rs`

    pub(super) enum AppServerCompletion {
        Succeeded,
        Failed {
            message: String,
            interrupted: bool,
        },
    }

### Files expected to change

Primary:

- `src/ai/mod.rs` — add `pub mod session` and re-exports.
- `src/ai/session.rs` — new file (session state model and discovery).
- `src/ai/codex_exec.rs` — add `CodexResumeRequest`, extend trait.
- `src/ai/transcript.rs` — add `open_append`, `read_transcript_tail`.
- `src/ai/codex_process/mod.rs` — session lifecycle hooks, resume path.
- `src/ai/codex_process/app_server.rs` — refine `AppServerCompletion`.
- `src/tui/messages.rs` — add resume-related `AppMsg` variants.
- `src/tui/input.rs` — add `ResumePrompt` context and key mappings.
- `src/tui/app/mod.rs` — add `resume_prompt` field.
- `src/tui/app/codex_handlers.rs` — resume prompt and execution logic.
- `src/tui/app/rendering.rs` — render resume prompt.

Tests:

- `src/ai/codex_exec_tests.rs` — resume unit tests.
- `src/tui/app/codex_handlers_tests.rs` — resume handler tests.
- `tests/features/codex_session_resume.feature` — new BDD feature file.
- `tests/codex_session_resume_bdd.rs` — new BDD step definitions.
- `tests/codex_exec_bdd/state.rs` — extend stubs for resume.

Documentation:

- `docs/frankie-design.md` — ADR entry.
- `docs/users-guide.md` — session resumption section.
- `docs/roadmap.md` — mark item done.

No new external dependencies are required. All persistence uses `serde`,
`serde_json`, `cap_std`, `camino`, and `chrono` which are already declared in
`Cargo.toml`.
