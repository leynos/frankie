# Make `TimeTravelState` a stable public type

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Roadmap item 2.2.5 is the second extraction step that turns time-travel state
management from a TUI-only implementation detail into a reusable library
capability. After this change, an external caller (for example an embedded
agent host or an alternative CLI surface) will be able to inspect a
`TimeTravelState` value using stable, documented read accessors without
importing `crate::tui`. The TUI continues to use the same type; the only
change is that the struct and its read accessors become part of the public
library contract.

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
  library-first intent of `docs/adr-005-cross-surface-library-first-delivery.md`.
- Do not widen the scope to orchestration extraction. This item moves the state
  container and its read accessors; it does not move handler logic, `Cmd`
  wrappers, or `spawn_blocking` calls.
- Preserve current TUI behaviour. The change must swap the source location of
  `TimeTravelState`, not redesign the TUI interaction. All existing TUI and BDD
  tests must continue to pass without semantic changes.
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
- Mutation methods (`set_loading`, `set_error`, `set` on `loading` and
  `error_message` fields) and TUI-only factory methods (`loading`,
  `error`) must remain `pub(crate)` because they are internal orchestration
  concerns that belong to the TUI adapter layer and will be extracted in
  2.2.7. Publishing them now would create a premature public contract.
- `TimeTravelInitParams` must also move to `src/time_travel/` alongside
  `TimeTravelState`, because it is the sole constructor input and cannot
  remain in `src/tui/state/` once `TimeTravelState::new` is public.

## Tolerances (exception triggers)

- Scope: if the implementation needs more than 15 files or more than 700 net
  new lines, stop and escalate with a narrower staging proposal.
- Interface: if making `TimeTravelState` public requires changing the public
  shape of `CommitSnapshot`, `LineMappingVerification`, `CommitSha`, or
  `RepoFilePath`, stop and escalate.
- Surface area: if the extraction appears to require making orchestration
  functions (e.g. `load_time_travel_state`, `spawn_time_travel_load`) public
  in the same change, stop and escalate. That work belongs to 2.2.7.
- Validation: if `make check-fmt`, `make lint`, or `make test` fail after
  three fix cycles, stop and escalate with the logs and current diff.

## Risks

- Risk: the move introduces a circular dependency between `src/time_travel/`
  and `src/tui/state/` because `TimeTravelState` was originally defined in
  the TUI module and used by TUI state.
  Severity: medium. Likelihood: low.
  Mitigation: `TimeTravelState` depends only on types from `crate::local`
  (`CommitSnapshot`, `CommitSha`, `RepoFilePath`, `LineMappingVerification`),
  not on any TUI type. The dependency direction is clean:
  `src/tui/state/` will import from `src/time_travel/`, not vice versa.

- Risk: existing TUI BDD tests in `tests/time_travel_bdd.rs` import
  `frankie::tui::state::{TimeTravelInitParams, TimeTravelState}`. After the
  move, these import paths break.
  Severity: medium. Likelihood: high.
  Mitigation: update the TUI state module (`src/tui/state/mod.rs`) to
  re-export both types from `crate::time_travel`, preserving the existing
  import path for downstream TUI test code. New public-API tests will import
  directly from `frankie::time_travel`.

- Risk: the `#[cfg(test)]`-only `error` factory on `TimeTravelState` uses
  `pub(crate)` visibility. Moving the type to `src/time_travel/` could make
  this factory unreachable from TUI test code that currently calls it.
  Severity: low. Likelihood: medium.
  Mitigation: keep the factory as `pub(crate)` with `#[cfg(test)]`; it
  remains reachable from within the crate. The TUI view tests at
  `src/tui/components/time_travel_view/tests.rs` already construct error
  states via `TimeTravelState::error(...)` and will still compile because
  `pub(crate)` covers all in-crate code.

- Risk: making getters `pub` but leaving mutation methods `pub(crate)` creates
  a lopsided API that could confuse library consumers.
  Severity: low. Likelihood: low.
  Mitigation: document in Rustdoc that mutation is intentionally restricted to
  in-crate orchestration and will be extracted in a future roadmap item.

- Risk: design documentation drifts by implying this item delivered more than
  it actually did.
  Severity: low. Likelihood: medium.
  Mitigation: document the exact boundary in `docs/frankie-design.md`:
  public read accessors now, public orchestration later.

## Progress

- [ ] Read and internalise current codebase state, roadmap, and prior ExecPlan
      for 2.2.4.
- [ ] Draft this ExecPlan.
- [ ] Stage A: move `TimeTravelState` and `TimeTravelInitParams` to
      `src/time_travel/`, remove `#[doc(hidden)]`, promote read accessors to
      `pub`.
- [ ] Stage B: update TUI state module to re-export from `crate::time_travel`
      and adapt all TUI handler and component imports.
- [ ] Stage C: add unit tests alongside the new module and integration-level
      BDD tests under `tests/` proving the public API is usable without
      `frankie::tui`.
- [ ] Stage D: update `docs/frankie-design.md`, `docs/users-guide.md`,
      `docs/roadmap.md`, and run all validation gates.

## Surprises & discoveries

(No surprises yet; this section will be updated during implementation.)

## Decision log

(Decisions will be recorded here as implementation proceeds.)

## Outcomes & retrospective

(To be completed after implementation.)

## Context and orientation

The time-travel feature allows users to view the exact code state when a review
comment was made. It was originally implemented entirely under `src/tui/` as
part of roadmap item 2.2.3. Roadmap item 2.2.4 extracted `TimeTravelParams`
(the parameter object for initiating time travel from a review comment) into a
public `src/time_travel/mod.rs` module. This plan continues that extraction by
moving `TimeTravelState` (the runtime state container) into the same public
module.

### Current file layout

- `src/time_travel/mod.rs` — public module containing `TimeTravelParams` and
  `TimeTravelParamsError`, exported via `pub mod time_travel` in `src/lib.rs`.
  Also contains unit tests in `src/time_travel/tests.rs`.
- `src/tui/state/time_travel.rs` — defines `TimeTravelInitParams` (public
  struct with public fields) and `TimeTravelState` (public struct with
  `#[doc(hidden)]`, private fields, `pub(crate)` getters). Contains unit tests
  in `src/tui/state/time_travel/tests.rs`.
- `src/tui/state/mod.rs` — re-exports `TimeTravelInitParams` and
  `TimeTravelState` as public types from the `tui::state` namespace.
- `src/tui/app/time_travel_handlers/mod.rs` — TUI message handlers that
  create and mutate `TimeTravelState`. Imports `TimeTravelParams` from
  `crate::time_travel` and `TimeTravelInitParams`/`TimeTravelState` from
  `crate::tui::state`. Contains unit tests in
  `src/tui/app/time_travel_handlers/tests.rs`.
- `src/tui/components/time_travel_view.rs` — rendering component that reads
  `TimeTravelState` via its `pub(crate)` getters. Contains unit tests in
  `src/tui/components/time_travel_view/tests.rs`.
- `src/tui/app/mod.rs` — `ReviewApp` struct holds
  `time_travel_state: Option<TimeTravelState>` as a field.
- `src/tui/messages/mod.rs` — `AppMsg` enum has variants
  `TimeTravelLoaded(Box<TimeTravelState>)` and
  `CommitNavigated(Box<TimeTravelState>)`.
- `tests/time_travel_bdd.rs` — TUI-level BDD tests that import
  `frankie::tui::state::{TimeTravelInitParams, TimeTravelState}`.
- `tests/time_travel_params_bdd.rs` — public API BDD tests for
  `TimeTravelParams` (imports `frankie::time_travel`, not `frankie::tui`).
- `src/local/commit.rs` — defines `CommitSnapshot`, `CommitMetadata`,
  `LineMappingVerification`, `LineMappingStatus`, and `LineMappingRequest`,
  all of which are already public.
- `src/local/types.rs` — defines `CommitSha` and `RepoFilePath`, both public.

### Types involved

`TimeTravelState` has:

- A constructor `new(TimeTravelInitParams) -> Self` — currently
  `pub #[doc(hidden)]`.
- A factory `loading(RepoFilePath, Option<u32>) -> Self` — `pub(crate)`, used
  only by TUI handlers.
- A factory `error(String, RepoFilePath) -> Self` — `pub(crate)` +
  `#[cfg(test)]`, used only by TUI view tests.
- Read accessors (all `pub(crate)`): `snapshot`, `file_path`,
  `original_line`, `line_mapping`, `commit_history`, `current_index`,
  `is_loading`, `error_message`, `commit_count`, `can_go_previous`,
  `can_go_next`, `next_commit_sha`, `previous_commit_sha`.
- Mutation methods (all `pub(crate)` or `pub #[doc(hidden)]`):
  `update_snapshot`, `set_loading`, `set_error`.

The read accessors are the ones required by renderers and are the target for
promotion to `pub`. The mutation methods and TUI-only factories remain
`pub(crate)` because they are orchestration concerns belonging to roadmap item
2.2.7.

`TimeTravelInitParams` has all public fields and is used only as the input to
`TimeTravelState::new`. It must move alongside `TimeTravelState` because it is
part of the constructor's public signature.

### Getter inventory for promotion to `pub`

The following getters are called by the TUI rendering component
(`time_travel_view.rs`) and are therefore required by any renderer:

- `snapshot() -> &CommitSnapshot`
- `file_path() -> &RepoFilePath`
- `original_line() -> Option<u32>`
- `line_mapping() -> Option<&LineMappingVerification>`
- `current_index() -> usize`
- `commit_count() -> usize`
- `can_go_previous() -> bool`
- `can_go_next() -> bool`
- `is_loading() -> bool`
- `error_message() -> Option<&str>`

The following getters are called only by TUI handlers (not renderers) but are
still valuable for library consumers building alternative navigation:

- `commit_history() -> &[CommitSha]`
- `next_commit_sha() -> Option<&CommitSha>`
- `previous_commit_sha() -> Option<&CommitSha>`

All thirteen read accessors will be promoted to `pub` for completeness, since
they all return borrowed or `Copy` data and present no soundness or
backwards-compatibility risk.

## Plan of work

### Stage A: move types to `src/time_travel/` and promote visibility

Move `TimeTravelState` and `TimeTravelInitParams` from
`src/tui/state/time_travel.rs` into `src/time_travel/`. This stage also
removes `#[doc(hidden)]` attributes and promotes read accessor visibility.

The current `src/time_travel/mod.rs` already contains `TimeTravelParams` and
`TimeTravelParamsError`. There are two sensible layouts: adding state types to
the existing `mod.rs`, or splitting into submodules. Because `mod.rs` is
currently 134 lines and adding `TimeTravelState` plus `TimeTravelInitParams`
plus the `clamp_index` helper would add approximately 220 lines, the combined
file would be approximately 350 lines — within the 400-line limit. However,
the params extraction and state management are distinct concerns. For clarity
and to leave headroom for future additions (2.2.6 configurable limits, 2.2.7
orchestration), split the state into a new submodule
`src/time_travel/state.rs`.

Concrete edits:

1. Create `src/time_travel/state.rs` containing:
   - The `//!` module comment explaining it provides the runtime state
     container for time-travel navigation.
   - `TimeTravelInitParams` (moved verbatim from `src/tui/state/time_travel.rs`
     lines 11-26, keeping all public fields and derive attributes).
   - `TimeTravelState` (moved from `src/tui/state/time_travel.rs` lines 28-231)
     with these visibility changes:
     - Remove `#[doc(hidden)]` from the struct, from `new`, and from
       `update_snapshot`.
     - Change all thirteen read accessors from `pub(crate)` to `pub`.
     - Keep `loading`, `error`, `set_loading`, `set_error` as `pub(crate)`.
       Keep `error`'s `#[cfg(test)]` gate.
     - Move the private `clamp_index` function alongside.
   - Add Rustdoc to `TimeTravelState` with a short usage example showing
     construction via `TimeTravelInitParams` and read accessor usage.
   - The `#[cfg(test)]` test submodule path directive pointing to
     `state/tests.rs` (see Stage A, step 3).

2. Update `src/time_travel/mod.rs`:
   - Add `mod state;` and re-export the public types:
     `pub use state::{TimeTravelInitParams, TimeTravelState};`
   - This makes them available as `frankie::time_travel::TimeTravelInitParams`
     and `frankie::time_travel::TimeTravelState`.

3. Move the existing unit tests from `src/tui/state/time_travel/tests.rs` to
   `src/time_travel/state/tests.rs`. Update `use super::*` and any imports
   that referenced `crate::tui::state` to reference `crate::time_travel`
   or use `super::*` as appropriate.

4. Remove `TimeTravelState`, `TimeTravelInitParams`, and `clamp_index` from
   `src/tui/state/time_travel.rs`. The file should be deleted entirely since
   its only remaining content would be the now-moved types. The test file
   `src/tui/state/time_travel/tests.rs` should also be deleted since the tests
   have moved.

5. No changes to `src/lib.rs` are needed because `pub mod time_travel` already
   exists.

### Stage B: update TUI modules to import from `crate::time_travel`

All TUI code that previously imported `TimeTravelState` or
`TimeTravelInitParams` from `crate::tui::state` must now import from
`crate::time_travel`.

Concrete edits:

1. `src/tui/state/mod.rs` — remove the `mod time_travel;` line and its
   `pub use time_travel::{TimeTravelInitParams, TimeTravelState};` re-export.
   Replace with re-exports from the new location:
   `pub use crate::time_travel::{TimeTravelInitParams, TimeTravelState};`
   This preserves the `frankie::tui::state::TimeTravelState` import path for
   existing TUI BDD tests and any external code that was using the
   `#[doc(hidden)]` type through the TUI namespace.

2. `src/tui/app/time_travel_handlers/mod.rs` — change the import from
   `use crate::tui::state::{TimeTravelInitParams, TimeTravelState};` to
   `use crate::time_travel::{TimeTravelInitParams, TimeTravelState};`.

3. `src/tui/components/time_travel_view.rs` — change the import from
   `use crate::tui::state::TimeTravelState;` to
   `use crate::time_travel::TimeTravelState;`.

4. `src/tui/app/mod.rs` — if it imports `TimeTravelState` from
   `super::state`, update to `use crate::time_travel::TimeTravelState;`.

5. `src/tui/messages/mod.rs` — if it imports `TimeTravelState` from
   `super::state`, update to `use crate::time_travel::TimeTravelState;`.

6. `src/tui/app/time_travel_handlers/tests.rs` — update any direct imports.
   The existing test code uses `use super::*` which will pick up the handler's
   updated import, so likely no change needed. Verify by compiling.

7. `src/tui/components/time_travel_view/tests.rs` — update the import of
   `TimeTravelInitParams` from `crate::tui::state::TimeTravelInitParams` to
   `crate::time_travel::TimeTravelInitParams`.

8. `tests/time_travel_bdd.rs` — this file currently imports
   `frankie::tui::state::{TimeTravelInitParams, TimeTravelState}`. The
   re-export added in step 1 preserves this path, so no change is strictly
   required. However, for clarity, consider updating the import to
   `frankie::time_travel::{TimeTravelInitParams, TimeTravelState}` so the
   test demonstrates the canonical public path.

At the end of Stage B, run `make check-fmt`, `make lint`, and `make test` to
verify that the move is complete and all existing tests still pass.

### Stage C: add unit and behavioural tests

Add tests that prove the public API is usable outside `crate::tui`.

#### Unit tests

The existing unit tests moved in Stage A (now at
`src/time_travel/state/tests.rs`) already cover:

- Construction via `TimeTravelInitParams` and `TimeTravelState::new`.
- Loading state factory.
- Error state factory.
- Navigation availability (`can_go_previous`, `can_go_next`).
- Navigation at boundary indices.
- Loading blocks navigation.
- `update_snapshot` clamps index.
- Line mapping storage and retrieval.
- `clamp_index` edge cases.

These tests are sufficient for the state logic. No additional unit tests are
required unless the move introduces new behaviour (it should not).

#### Behavioural tests (BDD)

Add a new integration-level behavioural suite using `rstest-bdd` v0.5.0 under
`tests/`, with a dedicated feature file at
`tests/features/time_travel_state.feature`. The step definitions should import
`frankie::time_travel::{TimeTravelInitParams, TimeTravelState}` and
`frankie::local::{CommitMetadata, CommitSha, CommitSnapshot, RepoFilePath,
LineMappingVerification}` — not `frankie::tui` — to prove the public library
surface is usable by an external caller.

Required behavioural scenarios:

1. **Construct a time-travel state from init params and read accessors** —
   Given valid `TimeTravelInitParams` with a snapshot, file path, original
   line, line mapping, and a three-commit history at index 0, when
   `TimeTravelState::new` is called, then `snapshot().short_sha()` matches the
   expected short SHA, `file_path().as_str()` matches the expected path,
   `original_line()` returns the expected value, `line_mapping()` is present,
   `commit_count()` is 3, `current_index()` is 0, `is_loading()` is false, and
   `error_message()` is `None`.

2. **Navigate to the next commit and verify state update** — Given a state at
   index 1 of a three-commit history, when `update_snapshot` is called with a
   new snapshot and index 0, then `current_index()` is 0, `can_go_next()` is
   false, and `can_go_previous()` is true.

3. **Navigation blocked at the oldest commit** — Given a state at the last
   index of a three-commit history, then `can_go_previous()` is false and
   `can_go_next()` is true.

4. **Index clamping on out-of-bounds update** — Given a state with a
   three-commit history, when `update_snapshot` is called with index 100, then
   `current_index()` is clamped to 2 (the last valid index).

5. **State with no line mapping** — Given `TimeTravelInitParams` where
   `line_mapping` is `None` and `original_line` is `None`, when
   `TimeTravelState::new` is called, then `original_line()` returns `None` and
   `line_mapping()` returns `None`.

The BDD state struct should hold `Slot<TimeTravelState>` and use `Slot::set`
and `Slot::with_ref` following the pattern established in
`tests/time_travel_params_bdd.rs`.

### Stage D: update design and user documentation

Update `docs/frankie-design.md` to record the design decisions taken here. Add
a paragraph under F-007 (after the existing "Library API (roadmap 2.2.4)"
subsection) with a heading "##### Library API (roadmap 2.2.5)":

- `TimeTravelState` and `TimeTravelInitParams` now live in the public
  `frankie::time_travel` module alongside `TimeTravelParams`.
- All read accessors are public and stable; mutation methods remain
  crate-internal pending orchestration extraction in 2.2.7.
- No CLI surface is added for this slice because the work is a type visibility
  promotion underpinning an existing interactive feature.

Update `docs/users-guide.md` only if users need to understand behaviour that
is observable from the tool. For this slice, no user-facing behaviour changes.
The time-travel mode works exactly as before. However, if the user guide
currently references `TimeTravelState` or its accessibility, update that
reference. Based on the current guide content, no update is expected to be
necessary — verify during implementation.

After implementation and validation are complete, update `docs/roadmap.md` to
mark item 2.2.5 as done (change `- [ ]` to `- [x]`). Do not change the
roadmap checkbox during the draft phase.

## Concrete file-by-file changes

Expected code and doc touch points:

- `src/time_travel/state.rs` — new file containing `TimeTravelState`,
  `TimeTravelInitParams`, `clamp_index`, and Rustdoc.
- `src/time_travel/state/tests.rs` — moved unit tests (previously at
  `src/tui/state/time_travel/tests.rs`).
- `src/time_travel/mod.rs` — add `mod state;` and `pub use` re-exports.
- `src/tui/state/time_travel.rs` — delete (contents moved).
- `src/tui/state/time_travel/tests.rs` — delete (contents moved).
- `src/tui/state/mod.rs` — remove local `mod time_travel` and its `pub use`;
  add re-exports from `crate::time_travel`.
- `src/tui/app/time_travel_handlers/mod.rs` — update import path.
- `src/tui/components/time_travel_view.rs` — update import path.
- `src/tui/app/mod.rs` — update import path if needed.
- `src/tui/messages/mod.rs` — update import path if needed.
- `src/tui/components/time_travel_view/tests.rs` — update import path.
- `tests/time_travel_bdd.rs` — optionally update import path to canonical
  public path.
- `tests/time_travel_state_bdd.rs` — new BDD test file.
- `tests/features/time_travel_state.feature` — new feature file.
- `docs/frankie-design.md` — add F-007 library API subsection for 2.2.5.
- `docs/users-guide.md` — update only if needed (expected: no change).
- `docs/roadmap.md` — mark 2.2.5 as done.

## Validation

Run each gate through `tee` so the logs survive truncated terminal output.

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
  accessors, navigation boundaries, index clamping, and absent optional
  fields.
- `docs/frankie-design.md` reflects the final behaviour under F-007.
- `docs/roadmap.md` marks item 2.2.5 done only after the full validation
  suite passes.
- All of `make check-fmt`, `make lint`, and `make test` pass.

## Interfaces and dependencies

No new external dependencies are required. All types referenced by
`TimeTravelState` are already public in `crate::local`:

- `CommitSnapshot` (from `crate::local::commit`)
- `CommitSha` (from `crate::local::types`)
- `RepoFilePath` (from `crate::local::types`)
- `LineMappingVerification` (from `crate::local::commit`)

The `chrono` crate is used transitively via `CommitMetadata` but does not
appear in the `TimeTravelState` public surface.

### Public interface after this change

In `src/time_travel/state.rs`:

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

## Idempotence and recovery

Every stage can be re-run safely. The move is purely a relocation of existing
code with visibility adjustments. If a stage fails midway, reverting the
changed files to their pre-stage state and retrying is safe because no
persistent state (database, config files, user data) is affected.

## Approval gate

This plan is in DRAFT status and awaits explicit approval before
implementation begins.
