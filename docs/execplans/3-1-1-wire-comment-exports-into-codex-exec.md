# Wire comment exports into `codex app-server`

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Enable artificial intelligence (AI)-assisted resolution directly from Frankie’s
review terminal user interface (TUI) by wiring the existing structured comment
export pipeline into `codex app-server`, showing live progress in the
interface, and persisting a per-run transcript to disk. Success is observable
when a user can launch Codex from the TUI, watch status updates as events
arrive, inspect transcript files after completion, and see a clear error in the
TUI if Codex exits with a non-zero status.

## Constraints

- Reuse existing export-domain types and formatters in `src/export/`
  (re-exported for CLI use in `src/cli/export/`) rather than duplicating
  comment-serialization logic.
- Keep the model-view-update (MVU) boundary intact: TUI state transitions stay
  in `src/tui/`, process execution and stream parsing stay in a new AI-focused
  module.
- Every new Rust module must start with a `//!` module-level comment.
- No file may exceed 400 lines; split handlers/parsers/writers into small
  modules where needed.
- Use capability-oriented filesystem access (`cap_std::fs_utf8`, `camino`) for
  transcript and intermediate export files.
- Preserve existing operation modes and current export command-line interface
  (CLI) behaviour.
- Streaming progress must be derived from `codex app-server` JSON Lines
  (JSONL) events.
- Non-zero Codex exits must be surfaced as TUI-visible failures.
- Unit tests must use `rstest`; behavioural tests must use `rstest-bdd` v0.5.0
  and cover happy/unhappy paths plus relevant edge cases.
- Update `docs/frankie-design.md` with explicit design decisions made for this
  feature.
- Update `docs/users-guide.md` with user-visible interaction and transcript
  behaviour.
- Mark the roadmap entry done in `docs/roadmap.md` only after implementation
  and all validation gates pass.
- Validation gates must pass before completion:
  `make check-fmt`, `make lint`, and `make test`.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 20 files or more than
  1,200 net new lines, stop and escalate.
- Interface: if existing public operation modes or current export flags need
  breaking changes, stop and escalate.
- Dependencies: if any new dependency is needed beyond upgrading
  `rstest-bdd`/`rstest-bdd-macros` to `0.5.0`, stop and escalate.
- Runtime model: if bubbletea-rs cannot support practical progress streaming
  through message polling/dispatch without invasive framework changes, stop and
  escalate with alternatives.
- Test migration: if upgrading to `rstest-bdd` v0.5.0 breaks unrelated
  behaviour-driven development (BDD) suites in ways that cannot be fixed within
  three iterations, stop and escalate.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fails
  after three fix cycles, stop and escalate with logs.

## Risks

- Risk: `codex app-server` argument semantics may differ from assumptions.
  Severity: high. Likelihood: medium. Mitigation: add an early prototype stage
  to validate command invocation and JSONL stream handling before full
  integration.

- Risk: bubbletea-rs command model may make incremental updates awkward.
  Severity: high. Likelihood: medium. Mitigation: use explicit polling messages
  backed by an async channel/queue and keep streaming logic isolated in a
  dedicated handler module.

- Risk: transcript writes may fail due to permission/path issues.
  Severity: medium. Likelihood: medium. Mitigation: validate transcript
  directory before launch and emit actionable TUI errors including attempted
  path.

- Risk: `rstest-bdd` v0.5.0 upgrade may require compatibility updates in
  existing behavioural tests. Severity: medium. Likelihood: medium. Mitigation:
  migrate version first, run full test suite early, and fix macro call-sites
  before Codex-specific behaviour tests are added.

## Progress

- [x] (2026-02-10 00:00Z) Drafted self-contained ExecPlan with constraints,
  tolerances, staged implementation, and validation approach.
- [x] (2026-02-12 00:00Z) Implemented command invocation and JSONL event
  parsing for `codex app-server`.
- [x] (2026-02-12 00:00Z) Implemented Codex execution service and transcript
  persistence in `src/ai/`.
- [x] (2026-02-12 00:00Z) Integrated TUI trigger (`x`), polling-based progress
  streaming, and non-zero exit surfacing.
- [x] (2026-02-12 00:00Z) Added unit and behavioural coverage for happy and
  unhappy paths, including malformed stream and transcript failure cases.
- [x] (2026-02-12 00:00Z) Updated design and user documentation.
- [x] (2026-02-12 00:00Z) Marked roadmap item complete and ran full quality
  gates.

## Surprises & discoveries

- Discovery: repository currently pins `rstest-bdd = "0.4.0"` and
  `rstest-bdd-macros = "0.4.0"`, while this feature requires v0.5.0 coverage.
  Impact: plan includes an explicit dependency/version migration stage.

- Discovery: current TUI message flow is predominantly single-result command
  oriented, so streaming requires explicit polling/event-drain design. Impact:
  plan adds a dedicated streaming integration stage and tests for incremental
  status updates.

## Decision log

- Decision: integrate Codex execution as a dedicated AI module (`src/ai/`) and
  keep TUI code focused on state transitions and rendering. Rationale:
  preserves module responsibilities and keeps process/parsing logic
  unit-testable without full TUI harnesses. Date/Author: 2026-02-10 / plan
  author.

- Decision: trigger Codex execution from Review List mode using a single key
  binding (`x`) and apply it to the current filtered comment set. Rationale:
  aligns with design intent for one-key AI export and avoids introducing a new
  modal in this step. Date/Author: 2026-02-10 / plan author.

- Decision: store one JSONL transcript per Codex run under a deterministic
  local directory and surface the saved path in TUI completion/failure states.
  Rationale: satisfies transcript persistence while keeping recovery and audit
  straightforward for users. Date/Author: 2026-02-10 / plan author.

- Decision: add an architecture decision record (ADR) entry in
  `docs/frankie-design.md` for execution model, transcript storage policy, and
  failure mapping. Rationale: user explicitly requires design decisions to be
  recorded. Date/Author: 2026-02-10 / plan author.

## Outcomes & retrospective

- Implemented `src/ai/` with a dedicated execution path that launches
  `codex app-server`, parses progress events, and persists transcript JSONL.
- Added TUI integration for `x`-triggered execution with periodic poll ticks,
  live status updates, and explicit error surfacing for non-zero exits.
- Added unit tests for transcript directory/path logic, parser behaviour, and
  execution success/failure outcomes using scripted command stubs.
- Added behavioural coverage in `tests/codex_exec_bdd.rs` and
  `tests/features/codex_exec.feature` for:
  - streaming progress visibility
  - transcript persistence visibility
  - non-zero exit propagation
  - malformed event and transcript-write failure handling

## Context and orientation

Current export functionality already exists and should be reused:

- `src/export/model.rs` defines `ExportedComment` and `ExportFormat`.
- `src/export/jsonl.rs` writes JSONL output.
- `src/cli/export/mod.rs` re-exports those types and formatters for CLI
  orchestration.
- `src/cli/export_comments.rs` fetches and sorts review comments, then writes
  output.

Current TUI architecture and entry points:

- `src/cli/review_tui.rs` prepares context and launches `Program<ReviewApp>`.
- `src/tui/app/mod.rs` owns the MVU model and message dispatch.
- `src/tui/messages.rs` defines app messages.
- `src/tui/input.rs` maps key events to `AppMsg`.
- `src/tui/app/rendering.rs` renders status hints and errors.

Documentation targets for this feature:

- `docs/frankie-design.md` for architecture decision capture.
- `docs/users-guide.md` for new key binding and transcript/failure behaviour.
- `docs/roadmap.md` for phase-step completion state after full acceptance.

## Plan of work

Stage A (prototype and dependency alignment): verify `codex app-server`
invocation assumptions and upgrade behavioural test dependencies to
`rstest-bdd` v0.5.0. Do not proceed until the command-shape and test baseline
are stable.

Stage B (service scaffolding and unit tests): implement a Codex execution
service that accepts exported comments, launches Codex, parses JSONL events,
and writes transcript lines to disk. Add unit coverage for parser, transcript
writer, command construction, and exit-code mapping.

Stage C (TUI integration and behavioural tests): wire a one-key TUI action to
execute Codex against the active filtered comments, stream status updates into
the status bar, and show explicit error text for non-zero exits. Add
`rstest-bdd` scenarios for successful streaming, transcript persistence, and
failure propagation.

Stage D (documentation, close-out, and quality gates): update design and user
guides, then mark roadmap item done after all tests and gates pass.

Each stage ends with validation and a go/no-go checkpoint.

## Concrete steps

### Stage A: Prototype + test dependency migration

1. Update `Cargo.toml` dev dependencies:
   - `rstest-bdd = "0.5.0"`
   - `rstest-bdd-macros = { version = "0.5.0", features = [ "compile-time-validation"] }`
1. Run behavioural tests and fix compatibility issues introduced by v0.5.0.
1. Add a small prototype test/module that validates expected `codex app-server`
   process contract (stdout JSONL lines, stderr progress, exit status handling)
   using an injectable fake command runner.
1. Go/no-go:
   - Go when BDD baseline is green on v0.5.0 and the command contract is clear.
   - No-go if contract remains ambiguous after prototype; escalate with options.

### Stage B: Codex execution module + unit coverage

1. Add new AI module files:
   - `src/ai/mod.rs`
   - `src/ai/codex_exec.rs`
   - `src/ai/transcript.rs`
1. Define core types (names may vary, but responsibility must match):
   - execution request from exported comments + PR context
   - stream event enum mapped from JSONL event lines
   - execution result with exit status and transcript path
1. Reuse existing export code to produce JSONL payload from current comments
   before invoking Codex.
1. Implement transcript file lifecycle:
   - deterministic filename per run
   - line-by-line JSONL append
   - flush on completion and failure
1. Unit tests (`rstest`) to cover:
   - event parsing (valid/invalid JSONL)
   - transcript writing and path handling
   - command construction from execution request
   - non-zero exit mapping to TUI-consumable failure detail
   - edge cases (empty export set, malformed stream line, I/O failure)
1. Validation:
   - run module-focused tests first, then full unit suite.

### Stage C: TUI trigger + streaming progress + BDD

1. Extend message model in `src/tui/messages.rs` with Codex lifecycle events:
   start, progress, completion, failure, and poll tick.
1. Extend key mapping in `src/tui/input.rs`:
   - map `x` in `ReviewList` context to Codex execution start message.
1. Add Codex handler module under `src/tui/app/` (for example
   `codex_handlers.rs`) and wire it from `src/tui/app/mod.rs`.
1. Integrate progress streaming:
   - start async execution task
   - drain queued progress events on periodic poll tick messages
   - update status bar text while run is active
1. Integrate failure propagation:
   - if exit code non-zero, set TUI error state with exit code and transcript
     location.
1. Update `src/tui/app/rendering.rs` help/status hints to include new `x` key
   and running-state messaging.
1. Add behavioural tests using `rstest-bdd` v0.5.0:
   - feature file: `tests/features/codex_exec.feature`
   - scenarios:
     - successful run streams multiple status updates and writes transcript
     - non-zero exit surfaces failure in TUI
     - malformed event line is handled without panic and is recorded
     - transcript write failure is surfaced clearly
1. Validation:
   - run Codex-specific BDD test target
   - run full behavioural suite.

### Stage D: Documentation, roadmap, and close-out

1. Update `docs/frankie-design.md`:
   - add ADR entry documenting Codex execution wiring, streaming architecture,
     transcript persistence location, and error surfacing policy.
1. Update `docs/users-guide.md`:
   - new `x` key behaviour
   - where transcripts are written
   - what users see on success and non-zero Codex exits.
1. Mark roadmap item done in `docs/roadmap.md`:
   - set `Wire comment exports into codex app-server …` checklist entry to `[x]`
     only after all acceptance checks pass.
1. Run full validation gates (commands listed below) and capture logs.

## Validation and acceptance

Behavioural acceptance checks:

- Starting Codex from the review TUI emits visible, incremental status updates
  while the process is running.
- A transcript file is written to disk for every run and contains JSONL event
  capture from execution.
- A non-zero Codex exit is shown in the TUI as an error state, including clear
  exit status and transcript path.

Test acceptance checks:

- Unit tests use `rstest` and cover happy/unhappy paths and edge cases.
- Behavioural tests use `rstest-bdd` v0.5.0 and cover end-to-end flow from
  trigger to visible TUI outcome.

Required quality-gate commands (from repository root):

```
set -o pipefail; make check-fmt 2>&1 | tee /tmp/execplan-3-1-1-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/execplan-3-1-1-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/execplan-3-1-1-test.log
```

Expected results:

- Each command exits `0`.
- Logs show no lint warnings promoted to errors and no failing tests.

## Idempotence and recovery

- Transcript directory creation must be idempotent (`create_dir_all` semantics
  via capability-oriented APIs).
- Re-running tests should not require manual cleanup of transcript artefacts;
  tests should use temporary directories and fixtures.
- If a Codex run fails mid-stream, partial transcript data should remain on
  disk for diagnosis and the TUI must return to an actionable idle state.
- If dependency migration causes broad failures, revert only the migration
  commit and retry with compatibility fixes isolated in a separate commit.

## Artefacts and notes

Implementation should preserve concise evidence for reviewers:

- a successful transcript sample (sanitized, no secrets)
- one failing run transcript with non-zero exit
- test output snippets proving streaming and failure surfacing behaviour
- final gate logs from `/tmp/execplan-3-1-1-*.log`

## Interfaces and dependencies

New internal interfaces to define (exact names may vary):

- Codex execution service trait/abstraction for launching command and yielding
  parsed events.
- Event parser from JSONL line to strongly typed event enum.
- Transcript writer abstraction that accepts event lines and final status.

Concrete interface sketch to reduce ambiguity:

```
pub trait CodexExecutionService {
    fn start(
        &self,
        request: CodexExecutionRequest,
    ) -> Result<CodexExecutionHandle, IntakeError>;
}

pub enum CodexProgressEvent {
    ThreadStarted { thread_id: String },
    TurnStarted,
    ItemCompleted { kind: String },
    AgentMessage { text: String },
    ParseWarning { raw_line: String },
}
```

Transcript storage convention:

- Base directory: `${XDG_STATE_HOME:-~/.local/state}/frankie/codex-transcripts/`
- Per-run filename:
  `<owner>-<repo>-pr-<number>-<utc-yyyymmddThhmmssZ>.jsonl`
- Configurability: path may be overridden by a future config key; this step
  treats the default directory as authoritative for acceptance and tests.

Files expected to change (primary):

- `Cargo.toml`
- `src/lib.rs`
- `src/ai/mod.rs`
- `src/ai/codex_exec.rs`
- `src/ai/transcript.rs`
- `src/tui/messages.rs`
- `src/tui/input.rs`
- `src/tui/app/mod.rs`
- `src/tui/app/rendering.rs`
- `src/tui/app/codex_handlers.rs`
- `tests/features/codex_exec.feature`
- `tests/codex_exec_bdd.rs`
- `tests/codex_exec_bdd/mod.rs`
- `tests/codex_exec_bdd/state.rs`
- `docs/frankie-design.md`
- `docs/users-guide.md`
- `docs/roadmap.md`

No additional runtime dependencies are planned beyond aligning
`rstest-bdd`/macros to v0.5.0 for behavioural tests.

## Revision note

Initial draft created for Phase 3, Step "Codex execution integration". This
revision defines the staged implementation approach, codifies constraints and
exception tolerances, and sets explicit acceptance and validation gates for
streaming updates, transcript persistence, and non-zero exit surfacing.
