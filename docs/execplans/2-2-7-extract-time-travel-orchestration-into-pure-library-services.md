# Extract time-travel orchestration into pure library services

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE (2026-04-24)

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Roadmap item 2.2.7 finishes the library-first extraction of time-travel
behaviour started by 2.2.4, 2.2.5, and 2.2.6. After this change, Frankie will
expose host-safe time-travel orchestration through shared library code, so an
embedding host can load and navigate historical snapshots without importing
`crate::tui`, `bubbletea_rs::Cmd`, or Tokio-specific runtime glue. The TUI will
remain the interactive adapter: it will still own key bindings, loading flags,
`spawn_blocking`, `Cmd` creation, and process-global `OnceLock` context, but it
will no longer own the core load-and-navigate orchestration logic.

Success is observable in five ways. First, `frankie::time_travel` exposes a
pure navigation entry point alongside the existing load entry point, and the
shared surface contains no `bubbletea_rs` types. Second, the current helper
logic in `src/tui/app/time_travel_handlers/mod.rs` that calculates navigation
targets and loads commit snapshots moves into shared library code. Third, the
TUI still behaves the same when a user presses `t`, `h`, `l`, or `Esc`. Fourth,
unit and behavioural tests exercise both initial load and commit navigation
using mocked `GitOperations` without importing `frankie::tui`. Fifth,
`docs/frankie-design.md`, `docs/users-guide.md`, and `docs/roadmap.md` record
the final contract and mark the roadmap item done only after every validation
gate passes.

This slice is intentionally about shared orchestration, not new user-facing
workflow. No new standalone command-line interface (CLI) mode is expected. Per
ADR-005, that exception must be documented explicitly rather than assumed.

## Relevant documentation and skills

The implementer should keep these repository documents open while working:

- `docs/roadmap.md` for the acceptance criteria and dependency chain.
- `docs/adr-005-cross-surface-library-first-delivery.md` for the
  library-first delivery contract.
- `docs/frankie-design.md` for the existing time-travel design notes and the
  ADR index.
- `docs/users-guide.md` for any user-visible or library-consumer-facing wording
  that changes.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rstest-bdd-users-guide.md`, and
  `docs/reliable-testing-in-rust-via-dependency-injection.md` for the required
  testing style.
- `docs/building-idiomatic-terminal-uis-with-bubbletea-rs.md` and
  `docs/snapshot-testing-bubbletea-terminal-uis-with-insta.md` for the adapter
  boundary expectations around Bubble Tea.
- `docs/rust-doctest-dry-guide.md` for public API examples.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for keeping the
  extraction from turning into a second bumpy-road handler.
- `docs/two-tier-testing-strategy-for-an-octocrab-github-client.md` and
  `docs/ortho-config-users-guide.md` for test-layering and configuration
  precedent.

The most relevant skills for the implementation session are `execplans`
(because this document must stay current during implementation), `leta` for
semantic code navigation, and `rust-router` plus whichever Rust sub-skills are
needed once the public API shape is being finalized.

## Constraints

- Keep the shared orchestration in `src/time_travel/`, not under `src/tui/`.
  A public re-export of TUI-defined orchestration does not satisfy
  `docs/adr-005-cross-surface-library-first-delivery.md`.
- The shared surface must not expose or depend on `bubbletea_rs::Cmd`,
  `bubbletea_rs::Model`, `tokio::task::spawn_blocking`, or any TUI
  `OnceLock`-backed storage helper. Those concerns remain in the adapter layer.
- Preserve the current user-visible interaction model for time travel:
  `t` enters time travel, `h` and `l` navigate history, and `Esc` returns to
  the review list. This roadmap item is an extraction, not a redesign.
- Treat the current `src/time_travel::load_time_travel_state` function as the
  baseline, not as dead code. 2.2.7 completes the extraction by pulling the
  remaining navigation orchestration out of
  `src/tui/app/time_travel_handlers/mod.rs`.
- Prefer a dedicated shared service module such as
  `src/time_travel/service.rs` (or equivalent) rather than continuing to grow
  `src/time_travel/mod.rs`, because the file must stay below 400 lines.
- Any newly public API must have Rustdoc comments and examples that compile as
  doctests where practical.
- Every new Rust module must begin with a `//!` module comment.
- Unit tests must use `rstest`.
- Behavioural tests must use `rstest-bdd` v0.5.0 with `#[scenario(path = ...)]`.
- Behavioural helpers returning `Result` must not use `assert!`; return
  explicit errors to satisfy `clippy::panic_in_result_fn`.
- Documentation updates must use en-GB-oxendict spelling and follow
  `docs/documentation-style-guide.md`.
- Do not mark roadmap item 2.2.7 as done until code, tests, docs, and all
  validation gates have passed.
- A standalone CLI mode is out of scope unless implementation discovers a real
  automation use case that cannot be served by the library API. If that
  happens, stop and escalate because it would materially widen the roadmap
  slice.

## Tolerances (exception triggers)

- Scope: if the implementation needs more than 18 files or more than 900 net
  new lines, stop and escalate with a narrower extraction sequence.
- Interface: if the extraction appears to require changing the public shape of
  `GitOperations`, `ReviewComment`, `TimeTravelParams`, or the read-side
  `TimeTravelState` contract from 2.2.5, stop and escalate.
- Semantics: if the shared navigation API cannot express the current boundary
  behaviour cleanly without leaking TUI-specific concepts, stop and document
  the competing contract options before proceeding.
- Runtime: if the shared service can only work by taking a dependency on
  Tokio, Bubble Tea, or process-global storage, stop and escalate because that
  would violate the acceptance criteria.
- Validation: if `make fmt`, `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, or `make test` fail after three focused fix
  cycles, stop and escalate with logs and the current diff.

## Risks

- Risk: the current extraction is already partially complete, so it is easy to
  leave duplicated helpers in both `src/time_travel/` and
  `src/tui/app/time_travel_handlers/mod.rs`. Severity: high. Likelihood:
  medium. Mitigation: explicitly move the existing load helper and the new
  navigation helper into one shared module and delete the redundant TUI-only
  copies.
- Risk: navigation boundary semantics are currently implicit in the handler
  (`return None` when navigation is impossible). A new public API could choose
  the wrong contract and make external hosts guess. Severity: medium.
  Likelihood: medium. Mitigation: document the chosen contract in
  `docs/frankie-design.md` and test it behaviourally.
- Risk: the public API accidentally reuses TUI-only names such as
  `NavigationDirection`, creating future naming churn. Severity: low.
  Likelihood: medium. Mitigation: use a library-scoped name such as
  `TimeTravelNavigationDirection` (or equivalent) in the shared module.
- Risk: existing TUI BDD tests simulate `TimeTravelLoaded` and
  `CommitNavigated` messages rather than exercising the real orchestration
  path. Severity: medium. Likelihood: high. Mitigation: add dedicated
  library-facing behavioural tests for the new service, then keep TUI tests
  focused on adapter behaviour.
- Risk: `docs/users-guide.md` could be skipped because the interactive keys do
  not change, even though the library-facing section should now mention the new
  orchestration API. Severity: low. Likelihood: medium. Mitigation: treat the
  user guide update as mandatory unless implementation can prove there is no
  user- or embedder-visible wording change.

## Progress

- [x] (2026-04-16 00:00Z) Read `docs/roadmap.md`,
      `docs/adr-005-cross-surface-library-first-delivery.md`,
      `docs/frankie-design.md`, `docs/users-guide.md`, the adjacent
      time-travel ExecPlans, and the referenced testing guidance.
- [x] (2026-04-16 00:00Z) Confirmed the current baseline: initial load already
      lives in `src/time_travel::load_time_travel_state`, while navigation
      orchestration remains inside
      `src/tui/app/time_travel_handlers/mod.rs`.
- [x] (2026-04-16 00:00Z) Drafted this ExecPlan for roadmap item 2.2.7.
- [ ] Stage A: finalize the shared service contract and move orchestration into
      `src/time_travel/`.
- [x] (2026-04-24 00:00Z) Stage A: finalized the shared service contract in
      `src/time_travel/service.rs` with public
      `TimeTravelNavigationDirection`, re-exported
      `load_time_travel_state`, and new
      `navigate_time_travel_state`.
- [x] (2026-04-24 00:00Z) Stage B: reduced
      `src/tui/app/time_travel_handlers/mod.rs` to adapter-only logic that
      selects comments, toggles loading state, runs `spawn_blocking`, and
      translates service results into `AppMsg`.
- [x] (2026-04-24 00:00Z) Stage C: moved `load_time_travel_state` unit tests
      into `src/time_travel/service/tests.rs` and added navigation unit
      coverage for success, boundary, error, and line-mapping cases.
- [x] (2026-04-24 00:00Z) Stage D: added
      `tests/time_travel_orchestration_bdd.rs`
      plus `tests/features/time_travel_orchestration.feature` to cover happy,
      unhappy, and edge navigation cases.
- [x] (2026-04-24 00:00Z) Stage E: updated `docs/frankie-design.md`,
      `docs/users-guide.md`, and `docs/roadmap.md`, then passed all required
      validation gates.

## Surprises & Discoveries

- `src/time_travel/mod.rs` already exposes a public
  `load_time_travel_state(git_ops, params, head_sha, commit_history_limit)`
  helper. The roadmap item is therefore not a greenfield extraction; it is the
  completion of an in-progress split.
- The remaining orchestration in
  `src/tui/app/time_travel_handlers/mod.rs` is concentrated in four private
  elements: `NavigationDirection`, `CommitNavigationContext`,
  `verify_line_mapping_optional`, and `load_commit_snapshot`. These are the
  real extraction seam.
- The generic `spawn_load_task` helper is already an adapter-friendly boundary.
  It can stay in the TUI because it is exactly where `Cmd`, `spawn_blocking`,
  and `AppMsg` mapping belong.
- The current unit tests for `load_time_travel_state` live under
  `src/tui/app/time_travel_handlers/tests.rs`, even though the function is now
  public in `src/time_travel`. Moving those tests will be part of completing
  the library-first split.
- `tests/time_travel_bdd.rs` currently proves rendering and message handling,
  but not the real shared orchestration path. A new public API BDD file is
  needed to satisfy the acceptance criteria directly.
- The cleanest boundary contract for navigation is
  `Result<Option<TimeTravelState>, GitOperationError>`. `Ok(None)` preserves
  the existing "do nothing at the history boundary" semantics without inventing
  a fake git error or leaking TUI-specific message concepts into the library
  API.
- The TUI still needs a small navigation pre-check before spawning the
  background task so impossible navigation remains a synchronous no-op instead
  of launching an unnecessary blocking task.

## Decision Log

- Decision (2026-04-16): treat 2.2.7 as the completion of a partial
  extraction, not as a rewrite of time travel from scratch. Rationale: the
  shared load path and public state already exist; redoing them would add risk
  without advancing the roadmap.
- Decision (2026-04-16): place the shared orchestration in a dedicated module
  under `src/time_travel/` and re-export it from `src/time_travel/mod.rs`.
  Rationale: this keeps module responsibilities coherent and avoids breaching
  the 400-line file limit.
- Decision (2026-04-16): expose a library-facing navigation entry point and a
  library-facing navigation direction enum, keeping TUI-specific message
  dispatch out of the shared API. Rationale: external hosts need to invoke
  navigation without depending on Bubble Tea concepts.
- Decision (2026-04-16): keep the adapter split explicit. The TUI remains
  responsible for selecting the comment, setting loading state, building
  user-facing missing-repository errors from `OnceLock` context, running
  `spawn_blocking`, and mapping results into `AppMsg`. Rationale: those are
  surface concerns, not shared domain orchestration.
- Decision (2026-04-16): document in `docs/frankie-design.md` that no
  standalone CLI mode is added for this narrow slice. Rationale: the roadmap
  item extracts reusable library behaviour from an existing interactive flow,
  but does not introduce a distinct non-interactive workflow.
- Decision (2026-04-24): define `navigate_time_travel_state` as
  `Result<Option<TimeTravelState>, GitOperationError>`. Rationale: git failures
  still surface unchanged, while history-boundary navigation remains a
  first-class no-op that hosts can inspect without decoding adapter messages.
- Decision (2026-04-24): keep the TUI-side boundary check using
  `TimeTravelNavigationDirection::can_navigate(&TimeTravelState)` before
  launching `spawn_blocking`. Rationale: this avoids unnecessary worker
  dispatch while still keeping SHA/index calculation in the shared service.

## Outcomes & Retrospective

Implementation is complete. The shipped contract shape is:

- `src/time_travel/service.rs` owns `load_time_travel_state`,
  `navigate_time_travel_state`, and `TimeTravelNavigationDirection`.
- `navigate_time_travel_state` returns `Ok(None)` when navigation is not
  available, and otherwise returns a fresh `TimeTravelState`.
- `src/tui/app/time_travel_handlers/mod.rs` now delegates load and navigation
  orchestration to `crate::time_travel` and retains only adapter duties.
- Shared unit coverage now lives under `src/time_travel/service/tests.rs`.
- Public behavioural coverage now lives in
  `tests/time_travel_orchestration_bdd.rs`.

- The exact public service API that shipped:
  - `frankie::time_travel::load_time_travel_state(...)`
  - `frankie::time_travel::navigate_time_travel_state(...)`
  - `frankie::time_travel::TimeTravelNavigationDirection`
- Contract adjustments made during extraction:
  - Navigation now returns `Result<Option<TimeTravelState>, GitOperationError>`
    so git failures remain unchanged while history-boundary navigation is an
    explicit no-op.
- Tests added or moved:
  - Moved loader unit tests from
    `src/tui/app/time_travel_handlers/tests.rs` to
    `src/time_travel/service/tests.rs`.
  - Added shared navigation unit coverage in
    `src/time_travel/service/tests.rs`.
  - Added public behavioural coverage in
    `tests/time_travel_orchestration_bdd.rs` with
    `tests/features/time_travel_orchestration.feature`.
- Docs changed:
  - `docs/frankie-design.md`
  - `docs/users-guide.md`
  - `docs/roadmap.md`
  - This ExecPlan
  - The design and user documentation explicitly record that no standalone CLI
    mode was added for this extraction slice.
- Final validation commands:
  - `make fmt`
  - `make markdownlint`
  - `make nixie`
  - `make check-fmt`
  - `make lint`
  - `make test`
- Tolerance triggers hit: none.

## Context and orientation

The repository already contains most of the ingredients needed for 2.2.7, but
they are split across shared and TUI-only modules.

### Current shared code

`src/time_travel/mod.rs` currently defines:

- `TimeTravelParams` and `TimeTravelParamsError` from roadmap item 2.2.4.
- `TimeTravelInitParams` and `TimeTravelState` re-exports from roadmap item
  2.2.5.
- `load_time_travel_state(...)`, which already loads the initial snapshot,
  fetches commit history, clamps `commit_history_limit`, and optionally
  verifies line mapping.

`src/time_travel/state.rs` defines the public read-side state container. It
already exposes the navigation inspection methods needed by hosts and renderers
(`can_go_previous`, `can_go_next`, `next_commit_sha`, `previous_commit_sha`,
`commit_history`, `current_index`, and so on), while keeping mutation helpers
crate-internal.

### Current TUI-only code

`src/tui/app/time_travel_handlers/mod.rs` still owns:

- Message dispatch for `EnterTimeTravel`, `ExitTimeTravel`, `NextCommit`,
  `PreviousCommit`, `TimeTravelLoaded`, `TimeTravelFailed`, and
  `CommitNavigated`.
- The pure navigation helper logic: `NavigationDirection`,
  `CommitNavigationContext`, `verify_line_mapping_optional`, and
  `load_commit_snapshot`.
- The adapter runtime helper `spawn_load_task`, which wraps pure work inside
  `tokio::task::spawn_blocking` and maps results into `AppMsg`.
- Missing-repository error formatting via `build_no_repo_error()`, which reads
  the TUI `OnceLock` context.

`src/tui/app/model_impl.rs` and `src/cli/review_tui.rs` are part of the adapter
boundary. They use `OnceLock` storage and builder methods to thread `git_ops`,
`head_sha`, and `commit_history_limit` into `ReviewApp`.

### Current tests

`src/tui/app/time_travel_handlers/tests.rs` includes the current
`load_time_travel_state` unit tests, even though the function itself is already
shared.

`tests/time_travel_bdd.rs` exercises the TUI interaction flow, but it does so
by simulating callback messages rather than invoking the real shared load and
navigation orchestration.

`tests/commit_history_limit_bdd.rs` already demonstrates the preferred public
API testing style for the shared loader: it imports `frankie::time_travel`,
uses mocked `GitOperations`, and never imports `frankie::tui`.

### Recommended target shape

The shared time-travel API should end this roadmap step with one coherent home
for orchestration. A good target is:

1. `src/time_travel/service.rs` for pure load and navigation orchestration plus
   any private line-mapping helpers they share.
2. `src/time_travel/mod.rs` for public type definitions and re-exports.
3. `src/tui/app/time_travel_handlers/mod.rs` reduced to adapter concerns only:
   comment selection, loading-flag mutation, `Cmd` creation, `spawn_blocking`,
   and `AppMsg` translation.

The shared navigation API should accept shared types only. A concrete contract
such as
`navigate_time_travel_state(&dyn GitOperations, &TimeTravelState, direction, head_sha)`
 is preferable to leaking a TUI-only context struct. The exact function and
enum names can be finalized during implementation, but they must live in
`frankie::time_travel`, not in `frankie::tui`.

## Plan of work

### Stage A: establish the shared service contract

Create a dedicated shared orchestration module under `src/time_travel/`, with a
module-level `//!` comment and public re-exports from `src/time_travel/mod.rs`.
Move the current `load_time_travel_state` function and its private line-mapping
helper into that module so both initial load and navigation live side-by-side.

Add a public navigation entry point that consumes only shared types and
`GitOperations`. The function should derive the target commit, preserve commit
history, load the new snapshot, and re-run optional line-mapping verification
without depending on `ReviewApp`, `AppMsg`, `Cmd`, or Tokio. Introduce a
library-facing navigation direction enum rather than reusing the TUI-only
`NavigationDirection`.

Stage A should finish with a coherent, documented public service surface in
`frankie::time_travel`, plus any private helper types needed to keep the code
readable and below the file-size limit.

### Stage B: reduce the TUI handlers to adapters

Refactor `src/tui/app/time_travel_handlers/mod.rs` so the handler methods call
the shared service inside `spawn_load_task` instead of carrying their own pure
orchestration logic. The pure navigation structs and helpers should be deleted
from the TUI module once the shared replacements exist.

The TUI must keep the following responsibilities:

- Selecting the current comment and deriving `TimeTravelParams`.
- Handling missing-comment and missing-repository user-facing errors.
- Setting `TimeTravelState::loading(...)` before dispatching background work.
- Setting `state.set_loading(true)` before navigation commands.
- Running the pure service through `spawn_blocking`.
- Converting service results into `AppMsg::TimeTravelLoaded`,
  `AppMsg::CommitNavigated`, or `AppMsg::TimeTravelFailed`.

Do not move `build_no_repo_error`, `spawn_load_task`, or the `OnceLock` storage
helpers out of the TUI layer. They are adapter concerns by design.

### Stage C: move and extend unit coverage

Move the existing `load_time_travel_state_*` unit tests out of
`src/tui/app/time_travel_handlers/tests.rs` into the shared time-travel test
module so the shared library owns its own contract coverage.

Then add new `rstest` cases covering the extracted navigation path with mocked
`GitOperations`. At minimum the unit suite must cover:

- Successful initial load with history and exact line mapping.
- Successful navigation to an older commit from the newest state.
- Successful navigation back to a newer commit from the middle of history.
- A boundary case where navigation is unavailable and no git call should be
  made.
- An unhappy path where snapshot loading fails during navigation and the error
  is surfaced unchanged.
- Any relevant line-mapping edge case, such as missing `head_sha` or missing
  original line number yielding `None` instead of an error.

Keep TUI unit tests focused on adapter behaviour after this move. For example,
they should still verify loading-state transitions, missing metadata errors,
and `TimeTravelFailed` message handling.

### Stage D: add public API behavioural tests

Add a new behavioural suite such as `tests/time_travel_orchestration_bdd.rs`
with a companion `tests/features/time_travel_orchestration.feature`. This suite
must import only the public library surface from `frankie::time_travel` and
mocked `GitOperations`, not `frankie::tui`.

The Gherkin scenarios should cover both happy and unhappy paths and keep the
observable contract explicit. A good baseline set is:

1. Load initial state from comment-derived parameters and configured history
   limit.
2. Navigate to an older commit and observe the new snapshot SHA and index.
3. Navigate back toward a newer commit and observe the new snapshot SHA and
   index.
4. Attempt navigation at the history boundary and observe the documented
   non-navigation outcome.
5. Surface a missing-commit or missing-snapshot error unchanged from the shared
   service.
6. Skip line-mapping verification cleanly when no `head_sha` is provided.

Keep `tests/time_travel_bdd.rs` green. Extend it only if the adapter wording or
observable TUI behaviour changes.

### Stage E: update docs and close the roadmap item

Update `docs/frankie-design.md` to record the final 2.2.7 contract. The design
document must say clearly that time-travel load and navigation orchestration
now live in shared library code, while the TUI adapter owns `Cmd`,
`spawn_blocking`, `OnceLock`, and view-state mutation.

Update `docs/users-guide.md` if the library-facing time-travel section needs to
mention the new orchestration API or any changed wording around the existing
interactive flow. If there is no end-user-visible TUI change, say so briefly in
the relevant section while still documenting the library-facing addition.

Mark roadmap item 2.2.7 as done in `docs/roadmap.md` only after the docs are
updated and every validation gate has passed.

## Validation and evidence

During implementation, short focused commands are acceptable for fast
iteration. The final evidence, however, must come from the repository Makefile
targets and must capture long output with `tee` plus `set -o pipefail`, per the
project instructions.

Run the documentation gates after the doc edits:

```bash
set -o pipefail
make fmt 2>&1 | tee /tmp/2-2-7-fmt.log
```

```bash
set -o pipefail
make markdownlint 2>&1 | tee /tmp/2-2-7-markdownlint.log
```

```bash
set -o pipefail
make nixie 2>&1 | tee /tmp/2-2-7-nixie.log
```

Run the required code-quality gates after the code changes:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/2-2-7-check-fmt.log
```

```bash
set -o pipefail
make lint 2>&1 | tee /tmp/2-2-7-lint.log
```

```bash
set -o pipefail
make test 2>&1 | tee /tmp/2-2-7-test.log
```

Implementation is complete only when all six commands exit successfully and the
roadmap, design doc, and user guide are in their final post-implementation
state.
