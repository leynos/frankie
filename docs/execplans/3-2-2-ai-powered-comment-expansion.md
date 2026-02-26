# Implement artificial intelligence (AI)-powered comment expansion and rewording

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Extend reply drafting with AI-assisted expansion and rewording so users can
improve draft quality without leaving Frankie workflows. After this change, a
user can request AI expansion or rewording for a draft reply, inspect a
side-by-side preview against the current text, see explicit AI-origin labels on
generated content, and apply or discard the suggestion. If the AI request
fails, Frankie keeps the original draft and surfaces a clear fallback message
instead of breaking the flow.

This step must ship as shared library behaviour with both text-based user
interface (TUI) and command-line interface (CLI) adapters, with deterministic
tests for happy/unhappy paths and edge cases.

Success is observable when:

- text-based user interface (TUI) reply drafting can request `expand` and
  `reword` actions.
- Generated text is explicitly labelled as AI-originated.
- A side-by-side diff preview is shown before applying AI text.
- AI call failures degrade gracefully (original draft preserved, actionable
  message shown).
- The same core behaviour is available via a non-interactive command-line
  interface (CLI) path.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.5.0) pass for
  both successful and failing AI interactions.
- `docs/frankie-design.md`, `docs/users-guide.md`, and `docs/roadmap.md` are
  updated.
- `make check-fmt`, `make lint`, and `make test` all pass.

## Constraints

- Keep core AI rewrite logic in shared library modules under `src/ai/`; TUI and
  CLI must stay adapter-only.
- Preserve MVU separation in TUI:
  - input mapping in `src/tui/input.rs`
  - message definitions in `src/tui/messages.rs`
  - state transitions in `src/tui/app/`
  - render queries in `src/tui/components/` and `src/tui/app/rendering.rs`
- Existing Codex execution and session-resumption flows (`x` key) must remain
  behaviourally unchanged.
- New public API must be documented with Rustdoc comments.
- Every new Rust module must start with a `//!` module-level comment.
- No file may exceed 400 lines; split modules/handlers/tests as needed.
- Prefer dependency injection for AI clients/services to keep tests
  deterministic and avoid environment mutation in tests.
- Behavioural tests must use `rstest-bdd` v0.5.0 with feature files under
  `tests/features/`.
- Use `vidaimock` as the local AI simulator for integration/behavioural
  validation.
- Record design decisions in `docs/frankie-design.md`.
- Document user-visible behaviour and CLI/TUI usage updates in
  `docs/users-guide.md`.
- Mark the roadmap checklist entry as done only after all acceptance gates pass.
- Required completion gates: `make check-fmt`, `make lint`, `make test`.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 30 files or more than
  2,000 net new lines, stop and escalate.
- Interface: if existing public CLI semantics must break incompatibly, stop and
  escalate.
- Dependencies: if more than one new runtime dependency is required, stop and
  escalate with alternatives.
- Provider model: if `vidaimock` cannot exercise required failure modes for
  deterministic tests, stop and escalate before introducing external-network
  test dependencies.
- Iterations: if any stage fails validation after three fix cycles, stop and
  escalate with logs.

## Risks

- Risk: AI integration introduces nondeterministic outputs that make tests
  flaky. Severity: high. Likelihood: medium. Mitigation: validate behaviour via
  deterministic fixtures and `vidaimock` scenario controls (including forced
  failures/malformed payloads).
- Risk: side-by-side preview layout can regress narrow-terminal readability.
  Severity: medium. Likelihood: medium. Mitigation: add width-aware rendering
  rules and snapshot coverage for narrow/wide terminal sizes.
- Risk: adding CLI AI mode can conflict with existing operation-mode selection.
  Severity: medium. Likelihood: medium. Mitigation: define explicit,
  precedence-tested mode-selection rules and config validation errors.
- Risk: fallback behaviour can be inconsistently implemented between library,
  TUI, and CLI surfaces. Severity: high. Likelihood: medium. Mitigation: expose
  a single library outcome type (`Generated` vs `Fallback`) consumed by both
  adapters.

## Progress

- [x] (2026-02-25 00:00Z) Drafted ExecPlan with architecture, test strategy,
      and `vidaimock` validation path for step 3-2-2.
- [x] (2026-02-26 00:00Z) Stage A: Finalized shared AI rewrite domain model
      and provider boundaries in `src/ai/comment_rewrite/`.
- [x] (2026-02-26 00:00Z) Stage B: Implemented shared fallback contract and
      side-by-side diff-preview model.
- [x] (2026-02-26 00:00Z) Stage C: Wired TUI AI request/preview/apply/discard
      flow with provenance and fallback handling.
- [x] (2026-02-26 00:00Z) Stage D: Added CLI `AiRewrite` access path with
      generated/fallback preview output.
- [x] (2026-02-26 00:00Z) Stage E: Added unit + behavioural tests, including
      `vidaimock`-driven success and malformed JSON fallback coverage.
- [x] (2026-02-26 00:00Z) Stage F: Updated design/user docs, marked roadmap
      item done, and passed required quality gates.

## Surprises & discoveries

- Discovery: `rstest-bdd` and `rstest-bdd-macros` are already pinned at
  `0.5.0` in `Cargo.toml`, so no version migration is needed for this step.
  Impact: implementation can focus on scenarios and harnesses.
- Discovery: `vidaimock` is available in this environment
  (`/root/.local/bin/vidaimock`) and can be started locally for deterministic
  OpenAI-compatible API simulation. Impact: behavioural tests can validate
  failure fallback and malformed payload handling without external network
  dependencies.
- Discovery: reply drafting currently lives primarily under `src/tui/` and is
  not yet exposed as a stable library API boundary for AI rewrite workflows.
  Impact: this step must extract/introduce reusable library-level contracts to
  satisfy roadmap cross-surface requirements.
- Discovery: TUI startup already uses global `OnceLock` wiring for other
  adapter services; adding rewrite-service injection through
  `set_comment_rewrite_service` preserved the existing bootstrap pattern and
  kept `ReviewApp` testable through explicit DI overrides.

## Decision log

- Decision: implement AI rewrite behaviour behind a shared library service trait
  with deterministic output contracts (`Generated` and `Fallback`). Rationale:
  guarantees parity between TUI and CLI surfaces while keeping tests
  independent of UI details. Date/Author: 2026-02-25 / plan author.
- Decision: require explicit AI-origin labelling in the domain model (not just
  render text) so both adapters cannot omit provenance accidentally. Rationale:
  acceptance requires generated text be labelled AI-originated. Date/Author:
  2026-02-25 / plan author.
- Decision: test OpenAI-compatible protocol/failure semantics with `vidaimock`
  fixtures in behavioural tests. Rationale: deterministic local simulation is
  required and avoids flaky external calls. Date/Author: 2026-02-25 / plan
  author.
- Decision: apply roadmap architecture decision record (ADR) practice by
  adding a new ADR entry in `docs/frankie-design.md` for this feature.
  Rationale: user explicitly requires design-decision capture. Date/Author:
  2026-02-25 / plan author.
- Decision: map AI rewrite keys to uppercase (`E`, `W`, `Y`, `N`) in
  reply-draft mode. Rationale: preserves lowercase free-text editing while
  keeping rewrite/apply/discard actions discoverable. Date/Author: 2026-02-26 /
  implementation author.

## Outcomes & retrospective

Implemented and validated:

- Shared AI rewrite library APIs in `src/ai/comment_rewrite/` with outcome and
  preview models reused by both adapters.
- OpenAI-compatible rewrite adapter with deterministic `vidaimock` tests for
  success and malformed-response fallback.
- New non-interactive CLI mode (`AiRewrite`) with provenance labelling,
  side-by-side preview output, and graceful fallback output.
- TUI reply-draft AI workflow with async rewrite requests, side-by-side preview
  rendering, apply/discard controls, and fallback error handling.
- Unit and behavioural coverage (`rstest`, `rstest-bdd`), including new feature
  scenarios in `tests/ai_reply_rewrite_bdd.rs`.
- Documentation updates in `docs/frankie-design.md`, `docs/users-guide.md`,
  and `docs/roadmap.md`.

Validation snapshot:

- `make test` passed (722 tests).
- `make check-fmt`, `make lint`, and markdown gates were run successfully at
  completion.

## Context and orientation

The current AI and reply infrastructure is split across these modules:

- `src/ai/codex_exec.rs` and `src/ai/codex_process/` provide Codex run
  orchestration and streaming status for the `x` workflow.
- `src/tui/state/reply_draft.rs` and `src/tui/app/reply_draft_handlers.rs`
  implement inline template drafting in TUI mode.
- `src/tui/components/comment_detail.rs` renders inline draft content.
- `src/config/mod.rs` already exposes reply-draft settings and operation-mode
  selection logic.
- `src/main.rs` dispatches operation modes through `src/cli/` handlers.

This feature adds AI text transformation for reply drafts with three guarantees:

1. Shared library API first.
2. Interactive TUI preview/apply flow.
3. Non-interactive CLI execution path.

Terminology used in this plan:

- AI rewrite: AI-generated transformation of draft text in mode `expand` or
  `reword`.
- AI-origin label: explicit provenance marker shown to users and carried in
  model/state.
- Fallback: graceful failure result that keeps original text and includes
  user-actionable failure detail.
- Side-by-side diff preview: original and candidate text rendered in parallel
  columns (or stacked fallback on narrow widths).

## Plan of work

### Stage A: Shared domain and provider contracts

Introduce a dedicated shared module for AI rewrite behaviour, for example:

- `src/ai/comment_rewrite/mod.rs`
- `src/ai/comment_rewrite/model.rs`
- `src/ai/comment_rewrite/service.rs`
- `src/ai/comment_rewrite/preview.rs`

Define stable types and trait boundaries:

- `CommentRewriteMode` (`Expand`, `Reword`).
- `CommentRewriteRequest` (source text plus optional review context fields).
- `CommentRewriteResult` with explicit provenance and preview-ready payload.
- `CommentRewriteOutcome`:
  - `Generated { ... }`
  - `Fallback { original_text, reason }`
- `CommentRewriteService` trait (DI-friendly).

Go/no-go for Stage A:

- Go when unit tests validate mode mapping, provenance labelling, and fallback
  invariants.
- No-go if contracts cannot express fallback and preview requirements without UI
  coupling.

### Stage B: Provider implementation and fallback semantics

Implement provider adapter(s) that satisfy the shared trait. Keep the provider
implementation isolated so TUI/CLI only consume `CommentRewriteOutcome`.

Implementation tasks:

- Add a production provider that executes AI rewrite requests.
- Add provider configuration fields in `FrankieConfig` needed for deterministic
  local test routing (base URL/model/timeout and optional headers map).
- Ensure provider failures map to `Fallback` outcomes rather than panics or
  opaque UI-specific errors.
- Implement preview-diff shaping in the shared module so adapters reuse one
  algorithm.

`vidaimock` usage baked into this stage:

- Verify compatibility baseline against local mock endpoint.
- Exercise failure modes using deterministic headers (`drop`, malformed JSON,
  latency).

Go/no-go for Stage B:

- Go when provider success and fallback paths are unit-tested and a
  `vidaimock`-backed integration test passes.
- No-go if provider contract requires hard dependency on live external network.

### Stage C: TUI integration

Extend reply-draft mode with AI rewrite interactions.

Likely touchpoints:

- `src/tui/messages.rs`: add AI rewrite request/result/apply/cancel messages.
- `src/tui/input.rs`: map new keys in reply-draft context (for example `E` for
  expand, `W` for reword, `Y` apply, `N` discard preview).
- `src/tui/app/reply_draft_handlers.rs`: call shared service and store pending
  candidate state.
- `src/tui/components/comment_detail.rs`: render side-by-side preview and
  explicit AI-origin label.
- `src/tui/app/rendering.rs`: update status/help hints.

TUI acceptance checks:

- Candidate text is visibly labelled AI-originated.
- Side-by-side preview appears before apply.
- Fallback error leaves original draft unchanged.

### Stage D: CLI integration

Add a non-interactive CLI path that exercises the same shared API.

Implementation direction:

- Add config/operation-mode fields for CLI AI rewrite requests.
- Add a new CLI handler module (for example `src/cli/ai_rewrite.rs`) that:
  - accepts rewrite mode and source text/context input,
  - calls shared library service,
  - prints labelled result and preview/diff,
  - returns graceful fallback output when AI fails.
- Update `src/main.rs` mode dispatch and config validation tests.

CLI acceptance checks:

- Same provenance label and fallback semantics as TUI.
- Non-zero process exit is used only for invalid user input/configuration, not
  routine AI fallback.

### Stage E: Tests (unit + behavioural)

Add or extend tests in three layers.

1. Unit tests (`rstest`):
   - request validation and mode handling,
   - provenance labelling,
   - diff-preview shaping,
   - fallback mapping for provider failure cases,
   - config parsing/operation-mode precedence.

2. TUI adapter tests (`rstest` + snapshots where appropriate):
   - message routing and state transitions,
   - preview render content includes AI-origin label,
   - narrow-width rendering degrades predictably.

3. Behavioural tests (`rstest-bdd` v0.5.0):
   - happy path: expand/reword generates labelled suggestion with preview,
   - unhappy path: provider error falls back gracefully,
   - unhappy path: malformed payload falls back gracefully,
   - edge case: generated text identical to source still labels provenance and
     preview indicates no effective change,
   - CLI path parity with TUI semantics.

`vidaimock` test harness requirements:

- Spawn `vidaimock` fixture on an ephemeral local port.
- Probe readiness via `/metrics`.
- Use per-request chaos headers to force deterministic unhappy paths.
- Ensure fixture teardown kills child process and captures logs on failure.

### Stage F: Documentation, roadmap, and close-out

Update docs:

- `docs/frankie-design.md`:
  - add ADR-006 for AI rewrite architecture/fallback/provenance rules,
  - update AI integration and review processor sections where behaviour changes.
- `docs/users-guide.md`:
  - document new TUI keys and preview/apply workflow,
  - document CLI usage and fallback behaviour,
  - document AI-origin labels and interpretation.
- `docs/roadmap.md`:
  - mark step `Add AI-powered comment expansion and rewording` as done after
    all validations pass.

## Concrete steps

Run commands from repository root (`/home/user/project`).

1. Confirm `vidaimock` availability and basic reachability:

```bash
command -v vidaimock
vidaimock --host 127.0.0.1 --port 8110 --format openai
curl -sS http://127.0.0.1:8110/metrics | head
```

Expected indicator:

```plaintext
- `command -v` prints a concrete binary path.
- `/metrics` returns Prometheus-style counters instead of connection errors.
```

1. Implement Stage A/B library modules and focused unit tests.

2. Implement Stage C TUI adapter integration and TUI-level tests/snapshots.

3. Implement Stage D CLI adapter integration and CLI tests.

4. Implement Stage E behavioural tests with `vidaimock` fixtures and failure
   injection.

5. Run quality gates with logs captured via `tee`:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/frankie-check-fmt.log
make lint 2>&1 | tee /tmp/frankie-lint.log
make test 2>&1 | tee /tmp/frankie-test.log
```

Expected indicator:

```plaintext
Each command exits 0, and logs contain no clippy warnings or failing tests.
```

1. Update documentation and roadmap checkbox only after all tests pass.

## Validation and acceptance

Functional acceptance:

- TUI:
  - In reply-draft mode, user can request expand/reword and see a
    side-by-side preview before applying.
  - Generated candidate includes an explicit AI-origin label.
  - Provider failure keeps original draft and shows graceful fallback message.
- CLI:
  - Non-interactive rewrite path exposes same generated/fallback semantics as
    library and TUI.
- Library:
  - Public API returns deterministic generated-or-fallback outcomes without
    UI-specific coupling.

Quality acceptance:

- Unit tests: targeted `rstest` suites for domain, provider mapping, TUI/CLI
  adapters.
- Behavioural tests: `rstest-bdd` v0.5.0 scenarios for happy/unhappy/edge
  paths, including `vidaimock` chaos-driven failures.
- Full gates:
  - `make check-fmt`
  - `make lint`
  - `make test`

Documentation acceptance:

- `docs/frankie-design.md` records new ADR and architectural updates.
- `docs/users-guide.md` documents new behaviour and controls.
- `docs/roadmap.md` item is checked complete only after validation success.

## Idempotence and recovery

- All implementation steps are additive and rerunnable.
- `vidaimock` fixtures must use ephemeral ports to avoid collisions.
- If a stage fails, fix and rerun stage-local tests before resuming next stage.
- Preserve log files in `/tmp/frankie-*.log` for escalation/debugging.

## Artifacts and notes

Capture these artifacts during implementation:

- Unit-test additions for shared AI rewrite logic and adapters.
- BDD feature files and step definitions for TUI/CLI AI rewrite behaviour.
- `vidaimock` harness fixture module and failure-injection tests.
- Documentation diffs for design/user guide/roadmap updates.
- Gate logs from `/tmp/frankie-check-fmt.log`, `/tmp/frankie-lint.log`, and
  `/tmp/frankie-test.log`.

## Interfaces and dependencies

Required interfaces at completion:

- Shared library API under `crate::ai::comment_rewrite` (names may vary but
  responsibilities must match):
  - rewrite request/response types,
  - explicit generated vs fallback outcome,
  - service trait for DI,
  - preview-diff helper consumed by both adapters.
- TUI adapter consumes shared service and renders provenance + preview.
- CLI adapter consumes the same shared service and outputs equivalent semantics.

Dependency stance:

- Reuse existing crates wherever possible.
- If a new runtime crate is necessary for provider transport, keep it singular,
  document why in ADR-006, and test via `vidaimock`.

## Revision note

Initial draft created for roadmap step 3-2-2 with explicit `vidaimock` testing,
shared library-first boundaries, TUI/CLI adapter plan, and completion gates.
