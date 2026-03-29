# Make `TimeTravelState` a stable public type

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Roadmap item 2.2.5 is the second extraction step that turns time-travel state
management from a terminal user interface (TUI)-only implementation detail into
a reusable library capability. After this change, an external caller (for
example an embedded agent host or an alternative command-line interface
surface) will be able to inspect a `TimeTravelState` value using stable,
documented read accessors without importing `crate::tui`. The TUI continues to
use the same type; the only change is that the struct and its read accessors
become part of the public library contract.

Success is observable in three ways. First, a library consumer can import
`frankie::time_travel::TimeTravelState` from a non-TUI module and call every
read accessor that renderers currently depend on. Second, the `#[doc(hidden)]`
attribute no longer appears on `TimeTravelState`, its constructor `new`, or its
mutation method `update_snapshot`, so `cargo doc` includes them. Third,
integration tests under `tests/` exercise the public API without importing
`frankie::tui`, proving the type is genuinely public and not merely visible
within the crate.

This slice is intentionally narrow. It does not extract time-travel
orchestration out of TUI handlers, it does not add configurable history limits,
and it does not introduce a new CLI command. Those concerns belong to roadmap
items 2.2.6 and 2.2.7 respectively.

## Constraints

- The type must live in `src/time_travel/`, not under `src/tui/`. A public
  re-export of a type still defined under `src/tui/` does not satisfy the
  library-first intent of
  `docs/adr-005-cross-surface-library-first-delivery.md`.
- Do not widen the scope to orchestration extraction. This item moves the state
  container and its read accessors; it does not move handler logic, `Cmd`
  wrappers, or `spawn_blocking` calls.
- Preserve current TUI behaviour. The change must swap the source location of
  `TimeTravelState`, not redesign the TUI interaction. All existing TUI and
  behavioural-driven development (BDD) tests must continue to pass without
  semantic changes.
- Public APIs added for this slice must have Rustdoc comments and at least one
  usage example when the example materially improves discoverability.
- Every new Rust module must begin with a `//!` module comment.
- No single source file may exceed 400 lines.
- Unit tests must use `rstest`.
- Behavioural tests must use `rstest-bdd` v0.5.0 and
  `#[scenario(path = ...)]`.
- Behavioural helpers returning `Result` must not use `assert!`; return
  explicit step errors to satisfy the repo's strict Clippy configuration
  (`clippy::panic_in_result_fn` under `-D warnings`).
- Documentation updates must use en-GB-oxendict spelling and follow the docs
  style guide at `docs/documentation-style-guide.md`.
- Do not mark roadmap item 2.2.5 as done until code, tests, design docs, user
  docs, and validation gates have all passed.
- Mutation methods (`set_loading`, `set_error`) and TUI-only factory methods
  (`loading`, `error`) must remain `pub(crate)` because they are internal
  orchestration concerns that belong to the TUI adapter layer and will be
  extracted in 2.2.7. Publishing them now would create a premature public
  contract.
- `TimeTravelInitParams` must also move to `src/time_travel/` alongside
  `TimeTravelState`, because it is the sole constructor input and cannot remain
  in `src/tui/state/` once `TimeTravelState::new` is public.

## Tolerances (exception triggers)

- Scope: if the implementation needs more than 15 files or more than 700 net
  new lines, stop and escalate with a narrower staging proposal.
- Interface: if making `TimeTravelState` public requires changing the public
  shape of `CommitSnapshot`, `LineMappingVerification`, `CommitSha`, or
  `RepoFilePath`, stop and escalate.
- Surface area: if the extraction appears to require making orchestration
  functions (for example `load_time_travel_state`, `spawn_time_travel_load`)
  public in the same change, stop and escalate. That work belongs to 2.2.7.
- Validation: if `make check-fmt`, `make lint`, or `make test` fail after
  three fix cycles, stop and escalate with the logs and current diff.

## Risks

- Risk: the move introduces a circular dependency between `src/time_travel/`
  and `src/tui/state/` because `TimeTravelState` was originally defined in the
  TUI module and used by TUI state. Severity: medium. Likelihood: low.
  Mitigation: `TimeTravelState` depends only on types from `crate::local`
  (`CommitSnapshot`, `CommitSha`, `RepoFilePath`, `LineMappingVerification`),
  not on any TUI type. The dependency direction is clean: `src/tui/state/` will
  import from `src/time_travel/`, not vice versa.

- Risk: existing TUI BDD tests in `tests/time_travel_bdd.rs` import
  `frankie::tui::state::{TimeTravelInitParams, TimeTravelState}`. After the
  move, these import paths break. Severity: medium. Likelihood: high.
  Mitigation: update the TUI state module (`src/tui/state/mod.rs`) to re-export
  both types from `crate::time_travel`, preserving the existing import path for
  downstream TUI test code. New public-API tests will import directly from
  `frankie::time_travel`.

- Risk: the `#[cfg(test)]`-only `error` factory on `TimeTravelState` uses
  `pub(crate)` visibility. Moving the type to `src/time_travel/` could make
  this factory unreachable from TUI test code that currently calls it.
  Severity: low. Likelihood: medium. Mitigation: keep the factory as
  `pub(crate)` with `#[cfg(test)]`; it remains reachable from within the crate.
  The TUI view tests at `src/tui/components/time_travel_view/tests.rs` already
  construct error states via `TimeTravelState::error(...)` and will still
  compile because `pub(crate)` covers all in-crate code.

- Risk: making getters `pub` but leaving mutation methods `pub(crate)` creates
  a lopsided API that could confuse library consumers. Severity: low.
  Likelihood: low. Mitigation: document in Rustdoc that mutation is
  intentionally restricted to in-crate orchestration and will be extracted in a
  future roadmap item.

- Risk: design documentation drifts by implying this item delivered more than
  it actually did. Severity: low. Likelihood: medium. Mitigation: document the
  exact boundary in `docs/frankie-design.md`: public read accessors now, public
  orchestration later.

## Progress

- [x] Read and internalise current codebase state, roadmap, and referenced
      architecture decision records.
- [x] Draft this ExecPlan.
- [x] Stage A: move `TimeTravelState` and `TimeTravelInitParams` to
      `src/time_travel/`, remove `#[doc(hidden)]`, promote read accessors to
      `pub`.
- [x] Stage B: update TUI state module to re-export from `crate::time_travel`
      and adapt all TUI handler and component imports.
- [x] Stage C: add integration-level BDD tests under `tests/` proving the
      public API is usable without `frankie::tui`.
- [x] Stage D: update `docs/frankie-design.md`, `docs/users-guide.md`,
      `docs/roadmap.md`, and run all validation gates.

## Surprises & discoveries

- The first `cargo test --test time_travel_state_bdd` run failed on an
  ambiguous Gherkin step (`the next commit SHA is absent`) because it
  overlapped with the parameterized `the next commit SHA is {expected}` step.
  Renaming the absent-state step removed the ambiguity cleanly.
- `clippy` flagged three issues in the new BDD helper code
  (`semicolon_if_nothing_returned`, `if_not_else`, and
  `redundant_closure_for_method_calls`). Fixing them required only local test
  cleanup; no production API changes were necessary.
- Background `cargo check` work triggered by `leta` competed for the target
  lock during validation. Rerunning `make lint` after the full test suite had
  finished produced the real result cleanly.

## Decision log

- `TimeTravelState` and `TimeTravelInitParams` were moved into a dedicated
  `src/time_travel/state.rs` submodule rather than folded into
  `src/time_travel/mod.rs`, preserving space for later roadmap items without
  growing `mod.rs` into a mixed-responsibility file.
- The TUI compatibility path was preserved by re-exporting
  `crate::time_travel::{TimeTravelInitParams, TimeTravelState}` from
  `src/tui/state/mod.rs`, while internal TUI modules were updated to import the
  public library module directly.
- The new public behavioural coverage focuses on three observable contracts:
  constructor/accessor usability, middle-of-history navigation inspection, and
  `update_snapshot` index clamping. That kept the suite library-facing without
  exposing TUI-only mutation helpers.

## Outcomes & retrospective

- `TimeTravelState` is now a documented public type under
  `frankie::time_travel`, and `TimeTravelInitParams` moved with it so the
  public constructor no longer depends on `crate::tui`.
- Existing TUI behaviour was preserved. The adapter layer still owns loading
  and error mutation helpers, while renderers and tests read the stable public
  accessors from the shared library module.
- Added `tests/time_travel_state_bdd.rs` and
  `tests/features/time_travel_state.feature` to prove the state API can be
  constructed and inspected without importing `frankie::tui`.
- Updated `docs/frankie-design.md` and marked roadmap item 2.2.5 complete in
  `docs/roadmap.md`. `docs/users-guide.md` required no change because
  user-visible behaviour stayed the same.
- Validation completed successfully:
  - `make fmt`
  - `MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint`
  - `make nixie`
  - `make check-fmt`
  - `make lint`
  - `make test`

## Context and orientation

The time-travel feature allows users to view the exact code state when a review
comment was made. It was originally implemented entirely under `src/tui/` as
part of roadmap item 2.2.3. Roadmap item 2.2.4 extracted `TimeTravelParams`
(the parameter object for initiating time travel from a review comment) into a
public `src/time_travel/mod.rs` module. This plan continues that extraction by
moving `TimeTravelState` (the runtime state container) into the same public
module.

### Current file layout

The following files are relevant to this change. All paths are relative to the
repository root.

`src/time_travel/mod.rs` is the public module containing `TimeTravelParams` and
`TimeTravelParamsError`, exported via `pub mod time_travel` in `src/lib.rs`.
Unit tests live in `src/time_travel/tests.rs`.

`src/tui/state/time_travel.rs` defines `TimeTravelInitParams` (a public struct
with public fields used as the constructor input) and `TimeTravelState` (a
public struct marked `#[doc(hidden)]`, with private fields and `pub(crate)`
getters). Unit tests live in `src/tui/state/time_travel/tests.rs`.

`src/tui/state/mod.rs` re-exports `TimeTravelInitParams` and `TimeTravelState`
as public types from the `tui::state` namespace.

`src/tui/app/time_travel_handlers/mod.rs` contains TUI message handlers that
create and mutate `TimeTravelState`. It imports `TimeTravelParams` from
`crate::time_travel` and `TimeTravelInitParams`/`TimeTravelState` from
`crate::tui::state`. Unit tests live in
`src/tui/app/time_travel_handlers/tests.rs`.

`src/tui/components/time_travel_view.rs` is the rendering component that reads
`TimeTravelState` via its `pub(crate)` getters. Unit tests live in
`src/tui/components/time_travel_view/tests.rs`.

`src/tui/app/mod.rs` defines the `ReviewApp` struct, which holds
`time_travel_state: Option<TimeTravelState>` as a field.

`src/tui/messages/mod.rs` defines the `AppMsg` enum, which has variants
`TimeTravelLoaded(Box<TimeTravelState>)` and
`CommitNavigated(Box<TimeTravelState>)`.

`tests/time_travel_bdd.rs` contains TUI-level BDD tests that import
`frankie::tui::state::{TimeTravelInitParams, TimeTravelState}`.

`tests/time_travel_params_bdd.rs` contains public API BDD tests for
`TimeTravelParams` (imports `frankie::time_travel`, not `frankie::tui`). This
file and its companion `tests/features/time_travel_params.feature` serve as a
template for the BDD tests this plan will add.

`src/local/commit.rs` defines `CommitSnapshot`, `CommitMetadata`,
`LineMappingVerification`, `LineMappingStatus`, and `LineMappingRequest`, all
of which are already public.

`src/local/types.rs` defines `CommitSha` and `RepoFilePath`, both public.

### Types involved

`TimeTravelState` has a constructor `new(TimeTravelInitParams) -> Self`
(currently `pub` but `#[doc(hidden)]`), a factory
`loading(RepoFilePath, Option<u32>) -> Self` (`pub(crate)`, used only by TUI
handlers), a factory `error(String, RepoFilePath) -> Self` (`pub(crate)` +
`#[cfg(test)]`, used only by TUI view tests), thirteen read accessors (all
`pub(crate)`), and three mutation methods (`update_snapshot` which is
`pub #[doc(hidden)]`, `set_loading` and `set_error` which are `pub(crate)`).

The thirteen read accessors are: `snapshot`, `file_path`, `original_line`,
`line_mapping`, `commit_history`, `current_index`, `is_loading`,
`error_message`, `commit_count`, `can_go_previous`, `can_go_next`,
`next_commit_sha`, `previous_commit_sha`. All thirteen will be promoted to
`pub` because they all return borrowed or `Copy` data and present no soundness
or backwards-compatibility risk.

`TimeTravelInitParams` has all public fields and is used only as the input to
`TimeTravelState::new`. It must move alongside `TimeTravelState` because it is
part of the constructor's public signature.

## Plan of work

### Stage A: move types to `src/time_travel/` and promote visibility

Move `TimeTravelState` and `TimeTravelInitParams` from
`src/tui/state/time_travel.rs` into `src/time_travel/`. This stage also removes
`#[doc(hidden)]` attributes and promotes read accessor visibility.

The current `src/time_travel/mod.rs` already contains `TimeTravelParams` and
`TimeTravelParamsError`. The params extraction and state management are
distinct concerns. For clarity and to leave headroom for future additions
(2.2.6 configurable limits, 2.2.7 orchestration), split the state into a new
submodule `src/time_travel/state.rs`.

Create `src/time_travel/state.rs` containing a `//!` module comment explaining
it provides the runtime state container for time-travel navigation. Move
`TimeTravelInitParams` verbatim from `src/tui/state/time_travel.rs` lines
11-26, keeping all public fields and derive attributes. Move `TimeTravelState`
from `src/tui/state/time_travel.rs` lines 28-217 with these visibility changes:
remove `#[doc(hidden)]` from the struct, from `new`, and from
`update_snapshot`; change all thirteen read accessors from `pub(crate)` to
`pub`; keep `loading`, `error`, `set_loading`, and `set_error` as `pub(crate)`,
preserving `error`'s `#[cfg(test)]` gate. Move the private `clamp_index`
function alongside. Add Rustdoc to `TimeTravelState` with a short usage example
showing construction via `TimeTravelInitParams` and read accessor usage.
Include a `#[cfg(test)]` test submodule path directive pointing to
`state/tests.rs`.

Update `src/time_travel/mod.rs` to add `mod state;` and re-export the public
types: `pub use state::{TimeTravelInitParams, TimeTravelState};`. This makes
them available as `frankie::time_travel::TimeTravelInitParams` and
`frankie::time_travel::TimeTravelState`.

Move the existing unit tests from `src/tui/state/time_travel/tests.rs` to
`src/time_travel/state/tests.rs`. Update `use super::*` and any imports that
referenced `crate::tui::state` to use `super::*` or `crate::time_travel` as
appropriate.

Delete `src/tui/state/time_travel.rs` entirely since its only remaining content
would be the now-moved types. Delete `src/tui/state/time_travel/tests.rs` since
the tests have moved.

No changes to `src/lib.rs` are needed because `pub mod time_travel` already
exists.

Stage A validation: run `cargo check --workspace` to confirm the move compiles
before proceeding.

### Stage B: update TUI modules to import from `crate::time_travel`

All TUI code that previously imported `TimeTravelState` or
`TimeTravelInitParams` from `crate::tui::state` must now import from
`crate::time_travel`.

In `src/tui/state/mod.rs`, remove the `mod time_travel;` line and its
`pub use time_travel::{TimeTravelInitParams, TimeTravelState};` re-export.
Replace with re-exports from the new location:
`pub use crate::time_travel::{TimeTravelInitParams, TimeTravelState};`. This
preserves the `frankie::tui::state::TimeTravelState` import path for existing
TUI BDD tests and any external code that was using the `#[doc(hidden)]` type
through the TUI namespace.

In `src/tui/app/time_travel_handlers/mod.rs`, change the import from
`use crate::tui::state::{TimeTravelInitParams, TimeTravelState};` to
`use crate::time_travel::{TimeTravelInitParams, TimeTravelState};`.

In `src/tui/components/time_travel_view.rs`, change the import from
`use crate::tui::state::TimeTravelState;` to
`use crate::time_travel::TimeTravelState;`.

In `src/tui/app/mod.rs`, update any import of `TimeTravelState` from
`super::state` to `use crate::time_travel::TimeTravelState;`.

In `src/tui/messages/mod.rs`, update any import of `TimeTravelState` from
`super::state` to `use crate::time_travel::TimeTravelState;`.

In `src/tui/components/time_travel_view/tests.rs`, update the import of
`TimeTravelInitParams` from `crate::tui::state::TimeTravelInitParams` to
`crate::time_travel::TimeTravelInitParams`.

In `tests/time_travel_bdd.rs`, update the import from
`frankie::tui::state::{TimeTravelInitParams, TimeTravelState}` to
`frankie::time_travel::{TimeTravelInitParams, TimeTravelState}` so the test
demonstrates the canonical public path.

Stage B validation: run `make check-fmt`, `make lint`, and `make test`. All
existing tests must pass without semantic changes.

### Stage C: add integration-level BDD tests

Add tests that prove the public API is usable outside `crate::tui`. The
existing unit tests moved in Stage A already cover construction, navigation,
loading/error states, index clamping, and line mapping storage. No additional
unit tests are needed.

Add a new integration-level behavioural suite using `rstest-bdd` v0.5.0. The
step definitions should import the state types from `frankie::time_travel` and
the domain types from `frankie::local` — not from `frankie::tui` — to prove the
public library surface is usable by an external caller.

Create `tests/features/time_travel_state.feature` with five scenarios:

1. "Construct a time-travel state and read its accessors" — verifies that a
   state built from valid `TimeTravelInitParams` exposes snapshot metadata,
   file path, line number, line mapping, commit count, index, loading status,
   and error message through public read accessors.

2. "Update snapshot and verify navigation state" — verifies that calling
   `update_snapshot` with a new snapshot and index changes the current index
   and navigation availability.

3. "Navigation blocked at the oldest commit" — verifies that at the last
   index `can_go_previous()` returns false and `can_go_next()` returns true.

4. "Index clamping on out-of-bounds update" — verifies that
   `update_snapshot` with an out-of-bounds index clamps to the last valid index.

5. "State with absent optional fields" — verifies that a state built with
   `original_line: None` and `line_mapping: None` returns `None` from the
   corresponding accessors.

Create `tests/time_travel_state_bdd.rs` with step definitions following the
pattern established in `tests/time_travel_params_bdd.rs`. The BDD state struct
should derive `ScenarioState` and `Default`, hold `Slot<TimeTravelState>`, and
use `Slot::set` and `Slot::with_ref`. Define an error type
`type StepError = &'static str;` and `type StepResult = Result<(), StepError>;`
as the other BDD test files do. Helpers returning `Result` must use explicit
error returns rather than `assert!` to satisfy `clippy::panic_in_result_fn`.

Stage C validation: run `cargo test --test time_travel_state_bdd` to confirm
the new behavioural suite passes, then run `make test` to confirm no
regressions.

### Stage D: update design and user documentation

Update `docs/frankie-design.md` to record the design decisions taken here. Add
a paragraph under Feature F-007 (after the existing "Library API (roadmap
2.2.4)" subsection) with a heading "##### Library API (roadmap 2.2.5)".
Document that `TimeTravelState` and `TimeTravelInitParams` now live in the
public `frankie::time_travel` module alongside `TimeTravelParams`, that all
read accessors are public and stable while mutation methods remain
crate-internal pending orchestration extraction in 2.2.7, and that no CLI
surface is added for this slice because the work is a type visibility promotion
underpinning an existing interactive feature.

Update `docs/users-guide.md` only if users need to understand behaviour that is
observable from the tool. For this slice, no user-facing behaviour changes. The
time-travel mode works exactly as before. Based on the current guide content,
no update is expected to be necessary — verify during implementation.

After implementation and validation are complete, update `docs/roadmap.md` to
mark item 2.2.5 as done (change `- [ ]` to `- [x]`). Do not change the roadmap
checkbox during the draft phase.

Stage D validation: run the full validation suite (formatting, Markdown lint,
Mermaid diagrams, Clippy, tests).

## Concrete steps

All commands run from the repository root (`/home/user/project`).

Format docs and source:

```bash
set -o pipefail && make fmt 2>&1 | tee /tmp/2-2-5-fmt.log
```

Validate Markdown:

```bash
set -o pipefail && MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint 2>&1 | tee /tmp/2-2-5-markdownlint.log
```

Validate Mermaid diagrams:

```bash
set -o pipefail && make nixie 2>&1 | tee /tmp/2-2-5-nixie.log
```

Verify formatting:

```bash
set -o pipefail && make check-fmt 2>&1 | tee /tmp/2-2-5-check-fmt.log
```

Run Clippy and Rustdoc checks:

```bash
set -o pipefail && make lint 2>&1 | tee /tmp/2-2-5-lint.log
```

Run the full test suite:

```bash
set -o pipefail && make test 2>&1 | tee /tmp/2-2-5-test.log
```

Confirm the new behavioural suite is part of the green run:

```bash
set -o pipefail && cargo test --test time_travel_state_bdd 2>&1 | tee /tmp/2-2-5-bdd.log
```

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes. The new BDD test
  `time_travel_state_bdd` passes. All existing tests in the workspace continue
  to pass.
- Lint/typecheck: `make check-fmt` and `make lint` both pass with zero
  warnings.
- Documentation: `cargo doc --workspace --no-deps` generates documentation
  for `TimeTravelState` and `TimeTravelInitParams` under `frankie::time_travel`
  (not hidden).

Quality method (how we check):

- Run the commands listed in "Concrete steps" above and compare exit codes
  against zero.
- Spot-check `cargo doc` output to confirm `TimeTravelState` is listed in the
  `frankie::time_travel` module documentation.
- Verify that `tests/time_travel_state_bdd.rs` imports from
  `frankie::time_travel`, not from `frankie::tui`, confirming the type is
  genuinely public.

Success criteria for close-out:

- The crate exposes `frankie::time_travel::TimeTravelState` and
  `frankie::time_travel::TimeTravelInitParams` outside `crate::tui`.
- `TimeTravelState` is not `#[doc(hidden)]`.
- All thirteen read accessors are `pub` and documented with Rustdoc.
- Mutation methods (`update_snapshot`, `set_loading`, `set_error`) and
  TUI-only factories (`loading`, `error`) remain `pub(crate)`.
- The TUI still enters time travel for valid comments, navigates history, and
  fails gracefully for invalid ones.
- Unit tests and `rstest-bdd` behavioural tests cover construction, read
  accessors, navigation boundaries, index clamping, and absent optional fields.
- `docs/frankie-design.md` reflects the final behaviour under F-007.
- `docs/roadmap.md` marks item 2.2.5 done only after the full validation
  suite passes.
- All of `make check-fmt`, `make lint`, and `make test` pass.

## Idempotence and recovery

Every stage can be re-run safely. The move is purely a relocation of existing
code with visibility adjustments. If a stage fails midway, reverting the
changed files to their pre-stage state and retrying is safe because no
persistent state (database, configuration files, user data) is affected.

The `git stash` or `git checkout -- <file>` commands provide a safe rollback
path for any individual file.

## Artifacts and notes

The following files are created by this plan:

- `src/time_travel/state.rs` — new submodule (~220 lines).
- `src/time_travel/state/tests.rs` — moved unit tests (~240 lines).
- `tests/time_travel_state_bdd.rs` — new BDD step definitions (~150 lines).
- `tests/features/time_travel_state.feature` — new Gherkin feature file (~40
  lines).

The following files are deleted:

- `src/tui/state/time_travel.rs` — contents moved to
  `src/time_travel/state.rs`.
- `src/tui/state/time_travel/tests.rs` — contents moved to
  `src/time_travel/state/tests.rs`.

Expected net change: approximately 200 new lines (BDD tests and Rustdoc) plus
the relocated lines. Well within the 15-file / 700-line tolerance.

## Interfaces and dependencies

No new external dependencies are required. All types referenced by
`TimeTravelState` are already public in `crate::local`:

- `CommitSnapshot` (from `crate::local::commit`)
- `CommitSha` (from `crate::local::types`)
- `RepoFilePath` (from `crate::local::types`)
- `LineMappingVerification` (from `crate::local::commit`)
- `CommitMetadata` (from `crate::local::commit`, used transitively by
  constructors but not in the `TimeTravelState` accessor surface)

The `chrono` crate is used transitively via `CommitMetadata` but does not
appear in the `TimeTravelState` public surface.

The public interface after this change, in `src/time_travel/state.rs`:

```rust
/// Parameters for initialising a time-travel state.
#[derive(Debug, Clone)]
pub struct TimeTravelInitParams {
    pub snapshot: CommitSnapshot,
    pub file_path: RepoFilePath,
    pub original_line: Option<u32>,
    pub line_mapping: Option<LineMappingVerification>,
    pub commit_history: Vec<CommitSha>,
    pub current_index: usize,
}

/// State container for time-travel navigation.
#[derive(Debug, Clone)]
pub struct TimeTravelState { /* private fields */ }

impl TimeTravelState {
    pub fn new(params: TimeTravelInitParams) -> Self;

    // Read accessors (all pub)
    pub const fn snapshot(&self) -> &CommitSnapshot;
    pub const fn file_path(&self) -> &RepoFilePath;
    pub const fn original_line(&self) -> Option<u32>;
    pub const fn line_mapping(&self) -> Option<&LineMappingVerification>;
    pub fn commit_history(&self) -> &[CommitSha];
    pub const fn current_index(&self) -> usize;
    pub const fn is_loading(&self) -> bool;
    pub fn error_message(&self) -> Option<&str>;
    pub const fn commit_count(&self) -> usize;
    pub const fn can_go_previous(&self) -> bool;
    pub const fn can_go_next(&self) -> bool;
    pub fn next_commit_sha(&self) -> Option<&CommitSha>;
    pub fn previous_commit_sha(&self) -> Option<&CommitSha>;

    // Mutation (pub, was #[doc(hidden)])
    pub fn update_snapshot(
        &mut self,
        snapshot: CommitSnapshot,
        line_mapping: Option<LineMappingVerification>,
        new_index: usize,
    );

    // Crate-internal (not promoted)
    pub(crate) fn loading(...) -> Self;
    pub(crate) fn error(...) -> Self;  // + #[cfg(test)]
    pub(crate) const fn set_loading(&mut self, loading: bool);
    pub(crate) fn set_error(&mut self, message: String);
}
```

In `src/time_travel/mod.rs` (additions):

```rust
mod state;

pub use state::{TimeTravelInitParams, TimeTravelState};
```

## Approval gate

This plan is in DRAFT status and awaits explicit approval before implementation
begins.
