# Extract `TimeTravelParams` into the public library application programming interface (API)

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DONE (2026-03-16)

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Roadmap item 2.2.4 is the first extraction step that turns time-travel from a
text user interface (TUI)-only implementation detail into a reusable library
capability. After this change, an external caller will be able to derive
time-travel inputs directly from `frankie::ReviewComment` without importing
`crate::tui`, while the TUI continues to use the same shared extraction logic
when the user presses `t`.

Success is observable in three ways. First, a library consumer can import a
public `TimeTravelParams` type from a non-TUI module and call
`TimeTravelParams::from_comment(&comment)` or the equivalent public conversion
without depending on `frankie::tui`. Second, missing metadata no longer
collapses into an uninformative `None`; tests prove the failure mode for each
required field. Third, the TUI time-travel flow still works and still refuses
comments that cannot supply the required metadata.

This slice is intentionally narrow. It does not make `TimeTravelState` public,
it does not extract TUI orchestration, and it does not add a new command-line
interface (CLI) command. Those concerns belong to roadmap items 2.2.5, 2.2.6,
and 2.2.7.

## Constraints

- Keep the extraction target outside `src/tui/`. A public re-export of a type
  still defined under `src/tui/` does not satisfy the library-first intent of
  `docs/adr-005-cross-surface-library-first-delivery.md`.
- Do not widen the scope to full time-travel orchestration. This item only
  extracts the parameter object and the metadata-to-parameter conversion path.
- Preserve current TUI behaviour for successful time-travel entry. The change
  should swap the source of `TimeTravelParams`, not redesign the key flow.
- Public APIs added for this slice must have Rustdoc comments and at least one
  usage example when the example materially improves discoverability.
- Every new Rust module must begin with a `//!` module comment.
- No single source file may exceed 400 lines.
- Unit tests must use `rstest`.
- Behavioural tests must use `rstest-bdd` v0.5.0 and `#[scenario(path = ...)]`.
- Behavioural helpers returning `Result` must not use `assert!`; return explicit
  step errors to satisfy the repo’s strict Clippy configuration.
- Documentation updates must use en-GB spelling and follow the docs style
  guide.
- Do not mark roadmap item 2.2.4 as done until code, tests, design docs, user
  docs, and validation gates have all passed.

## Tolerances (exception triggers)

- Scope: if the implementation needs more than 12 files or more than 600 net
  new lines, stop and escalate with a narrower staging proposal.
- Interface: if making `TimeTravelParams` public requires changing the public
  shape of `ReviewComment`, stop and escalate. This item must adapt to the
  current comment model.
- Surface area: if the extraction appears to require making `TimeTravelState`
  public in the same change, stop and escalate. That work belongs to 2.2.5.
- Error contract: if the existing TUI path depends on `Option` semantics in a
  way that cannot be preserved through a typed error adapter, stop and escalate
  with the competing options.
- Validation: if `make check-fmt`, `make lint`, or `make test` fail after three
  fix cycles, stop and escalate with the logs and current diff.

## Risks

- Risk: the implementation exposes `TimeTravelParams` publicly but leaves it in
  a TUI-named namespace, forcing a second churn-heavy move in 2.2.7. Severity:
  medium. Likelihood: medium. Mitigation: create a dedicated top-level
  `src/time_travel/` module now and make the TUI import from it.
- Risk: the public API keeps `Option`-based extraction, which technically
  exposes the type but fails the acceptance requirement around missing metadata
  failures. Severity: high. Likelihood: medium. Mitigation: introduce a typed
  extraction error and keep any `Option`-like convenience wrapper, if desired,
  as a thin adapter over the typed result.
- Risk: tests remain TUI-internal, so the crate appears public only because the
  implementation and tests compile within the same module tree. Severity: high.
  Likelihood: medium. Mitigation: add integration tests under `tests/` that
  import only the public library surface.
- Risk: the TUI error message path regresses because handler code currently
  expects a silent `None` from `from_comment`. Severity: medium. Likelihood:
  medium. Mitigation: update the handler to map typed extraction failures into
  the existing user-facing message path and add an unhappy-path behavioural
  test.
- Risk: design documentation drifts by implying this item delivered more than
  it actually did. Severity: low. Likelihood: medium. Mitigation: document the
  exact boundary: public params extraction now, public state and pure
  orchestration later.

## Progress

- [x] (2026-03-13 00:00Z) Read `docs/roadmap.md`,
      `docs/adr-005-cross-surface-library-first-delivery.md`, the time-travel
      implementation under `src/tui/`, and the referenced testing and user
      documentation.
- [x] (2026-03-13 00:00Z) Confirmed current baseline: `TimeTravelParams` is
      `pub(crate)` in `src/tui/state/time_travel.rs`, and extraction tests are
      duplicated between TUI state and handler modules.
- [x] (2026-03-13 00:00Z) Drafted this ExecPlan for roadmap item 2.2.4.
- [x] (2026-03-16) Stage A: introduce a public non-TUI `time_travel` module
      with `TimeTravelParams` and typed extraction failures.
- [x] (2026-03-16) Stage B: switch TUI state and handlers to consume the
      shared public type without changing user-visible happy-path behaviour.
- [x] (2026-03-16) Stage C: add unit and behavioural coverage for success,
      missing-metadata failures, and line-number fallback.
- [x] (2026-03-16) Stage D: update design docs, user guide, and roadmap
      status after all validation gates pass.

## Surprises & Discoveries

- The current implementation already contains the extraction logic the roadmap
  wants, but it is trapped inside `src/tui/state/time_travel.rs` as a
  crate-private type.
- There are already two clusters of extraction tests: one in
  `src/tui/state/time_travel/tests.rs` and another in
  `src/tui/app/time_travel_handlers/tests.rs`. This is a useful warning that
  the extraction should centralize tests around the new public module instead
  of copying them again.
- `ReviewComment` already derives `Default`, so Rustdoc examples for a public
  `from_comment` constructor can be written without relying on test-only helper
  fixtures.
- No standalone CLI surface is warranted for this slice. The feature is a
  library contract extracted from an existing interactive workflow, so the
  design documentation must record that CLI is not applicable here rather than
  pretending a command is needed.

## Decision Log

- Decision (2026-03-13): implement this slice as a new top-level
  `src/time_travel/` module rather than a re-export from `src/tui/state/`.
  Rationale: roadmap item 2.2.4 explicitly requires availability outside
  `crate::tui`, and later time-travel extraction work should build on a stable,
  non-TUI namespace instead of moving the type twice.
- Decision (2026-03-13): replace the current `Option<Self>` extraction contract
  with a typed `Result<Self, TimeTravelParamsError>` public API, with an
  optional convenience wrapper only if a call site benefits from it. Rationale:
  the acceptance criteria require tests for missing metadata failures, which is
  stronger and more useful than an undifferentiated `None`.
- Decision (2026-03-13): keep the TUI as an adapter over the public extraction
  API and preserve the current `t` key interaction. Rationale: this roadmap
  item is about public API shape, not user-interface redesign.
- Decision (2026-03-13): satisfy the behavioural-test requirement with
  integration tests outside `crate::tui` that exercise the new public API, and
  add a TUI unhappy-path scenario only where it proves adapter behaviour.
  Rationale: the most important regression to prevent is a fake-public API that
  cannot be used by external callers.

## Outcomes & Retrospective

Implementation completed 2026-03-16.

Outcomes:

- `frankie::time_travel::TimeTravelParams` is publicly importable outside
  `crate::tui`, with `from_comment` returning
  `Result<Self, TimeTravelParamsError>`.
- `TimeTravelParamsError` has `MissingCommitSha` and `MissingFilePath`
  variants, both tested.
- Five `rstest` unit tests in `src/time_travel/tests.rs` cover success,
  fallback, both failure modes, and the both-lines-absent edge case.
- Four `rstest-bdd` behavioural scenarios in
  `tests/time_travel_params_bdd.rs` prove the public API is usable without
  importing `frankie::tui`.
- The TUI handler maps typed extraction errors into the existing
  user-facing error path, preserving interactive behaviour.
- Duplicated extraction tests were removed from TUI state and handler
  modules; the shared module and integration tests now own extraction coverage.
- `docs/frankie-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
  were updated to reflect the final behaviour.

Retrospective:

- The extraction was straightforward because the existing code had a clean
  `from_comment` method that only needed to be moved and given typed errors.
- Making the struct fields private with accessors was the right call for a
  public API; it prevented handler tests from constructing the type via struct
  literal, which would have been fragile.
- No tolerance triggers were hit. The change touched 10 files and added
  approximately 300 net new lines, well within the 12-file / 600-line budget.

## Context and orientation

The current time-travel feature lives entirely under `src/tui/`.

- `src/tui/state/time_travel.rs` defines `TimeTravelState`,
  `TimeTravelInitParams`, and a crate-private `TimeTravelParams`.
- `src/tui/app/time_travel_handlers/mod.rs` calls
  `TimeTravelParams::from_comment(comment)` when the user presses `t`.
- `src/tui/state/mod.rs` re-exports `TimeTravelState` publicly but keeps
  `TimeTravelParams` crate-private.
- `src/lib.rs` currently exposes no top-level `time_travel` module.
- `tests/time_travel_bdd.rs` covers the TUI interaction flow, but it currently
  imports time-travel types from `frankie::tui::state`.

The implementation for this roadmap item should introduce a small, shared
library surface without prematurely extracting the rest of time-travel.

Recommended target layout:

1. Add `src/time_travel/mod.rs` with the public data transfer object (DTO) and
   its extraction error.
2. Export the module from `src/lib.rs` via `pub mod time_travel;`.
3. Update TUI state and handler code to depend on `crate::time_travel`.
4. Move extraction-focused tests to the new module and to integration tests in
   `tests/`.

Recommended public contract:

- `pub struct TimeTravelParams { commit_sha, file_path, line_number }`
- `pub enum TimeTravelParamsError { MissingCommitSha, MissingFilePath }`
- `impl TimeTravelParams { pub fn from_comment(comment: &ReviewComment)
  -> Result<Self, TimeTravelParamsError> }`

The line-number rule should remain unchanged from today’s TUI-only behaviour:
use `line_number` when present, otherwise fall back to `original_line_number`.
Missing line information is not itself a failure, because time-travel can still
load the file snapshot even when line mapping is unavailable.

## Plan of work

### Stage A: create the shared public API

Create `src/time_travel/mod.rs` and move the parameter object there. Give the
module a `//!` comment that explains it contains library-facing types for
deriving time-travel inputs from review comments.

Implement `TimeTravelParams` as a public struct using the existing domain types
already used by the TUI: `CommitSha`, `RepoFilePath`, and `Option<u32>` for the
line number. Add a public `TimeTravelParamsError` enum using
`thiserror::Error`, with variants for each missing required field. Implement
`from_comment` to return `Result<Self, TimeTravelParamsError>`, and keep the
current precedence of `line_number` over `original_line_number`.

Add Rustdoc for both public types. The `from_comment` docs should include a
small example showing success and mention that a missing commit SHA or file
path returns a typed error.

### Stage B: adapt the TUI to the shared API

Remove the local `TimeTravelParams` definition from
`src/tui/state/time_travel.rs`. Import the shared type from
`crate::time_travel::TimeTravelParams` in the TUI state module and the time
travel handlers.

Update `handle_enter_time_travel` in `src/tui/app/time_travel_handlers/mod.rs`
to call the new public extraction method. Map `TimeTravelParamsError` into the
existing user-facing error path so that comments without sufficient metadata
still fail gracefully inside the TUI. Do not change key bindings, view modes,
or navigation behaviour in this step.

Clean up duplicated extraction tests from TUI-only modules once equivalent
coverage exists in the new shared module or integration tests.

### Stage C: add unit and behavioural tests

Add `rstest` unit tests alongside `src/time_travel/mod.rs` for the public
extraction contract.

Required unit cases:

1. Successful extraction when commit SHA, file path, and current line are
   present.
2. Successful extraction when `line_number` is absent but
   `original_line_number` is present.
3. Failure with `MissingCommitSha`.
4. Failure with `MissingFilePath`.
5. Successful extraction when both line fields are absent, proving line number
   remains optional.

Add an integration-style behavioural suite using `rstest-bdd` v0.5.0 under
`tests/`, with a dedicated feature file such as
`tests/features/time_travel_params.feature`. The step definitions should import
`frankie::time_travel::TimeTravelParams` and `frankie::ReviewComment`, not
`frankie::tui`, so the test proves the public library surface is usable by an
external caller.

Required behavioural scenarios:

1. Deriving parameters from a complete review comment succeeds.
2. Deriving parameters falls back to the original line when the current line is
   missing.
3. Deriving parameters fails when the commit SHA is missing.
4. Deriving parameters fails when the file path is missing.

Add or extend one TUI-focused regression test only if needed to prove the
handler maps typed extraction failures into the expected interactive error
path. Keep that adapter test narrow; the public contract belongs in the new
library tests.

### Stage D: update design and user documentation

Update `docs/frankie-design.md` to record the design decisions taken here. Keep
the wording precise:

- `TimeTravelParams` now lives in a non-TUI public module.
- Missing metadata surfaces as typed extraction failures.
- No CLI surface is added for this slice because the work is an API extraction
  underpinning an existing interactive feature.
- `TimeTravelState` and orchestration remain pending roadmap items 2.2.5 to
  2.2.7.

Update `docs/users-guide.md` only where users need to understand behaviour that
is observable from the tool. The likely update is the time-travel requirements
section, clarifying that the selected review comment must carry both a commit
SHA and file path for time travel to start.

After implementation and validation are complete, update `docs/roadmap.md` to
mark item 2.2.4 as done. Do not change the roadmap checkbox during the draft
phase.

## Concrete file-by-file changes

Expected code and doc touch points:

- `src/time_travel/mod.rs` for the new public DTO, error type, docs, and unit
  tests.
- `src/lib.rs` to export the new top-level module.
- `src/tui/state/time_travel.rs` to remove the local DTO definition and import
  the shared type.
- `src/tui/state/mod.rs` to stop re-exporting the crate-private params type.
- `src/tui/app/time_travel_handlers/mod.rs` for the adapter from typed
  extraction errors into the existing flow.
- `src/tui/app/time_travel_handlers/tests.rs` only if a TUI adapter regression
  test remains necessary after centralizing extraction coverage.
- `tests/time_travel_params_bdd.rs` and
  `tests/features/time_travel_params.feature` for public API behavioural tests.
- `docs/frankie-design.md`, `docs/users-guide.md`, and `docs/roadmap.md` for
  design rationale, user-facing behaviour notes, and roadmap completion.

## Validation

Run each gate through `tee` so the logs survive truncated terminal output.

- Format docs and source:

```bash
set -o pipefail && make fmt 2>&1 | tee /tmp/2-2-4-time-travel-params-fmt.log
```

- Validate Markdown:

```bash
set -o pipefail && MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint 2>&1 | tee /tmp/2-2-4-time-travel-params-markdownlint.log
```

- Validate Mermaid diagrams:

```bash
set -o pipefail && make nixie 2>&1 | tee /tmp/2-2-4-time-travel-params-nixie.log
```

- Verify formatting:

```bash
set -o pipefail && make check-fmt 2>&1 | tee /tmp/2-2-4-time-travel-params-check-fmt.log
```

- Run Clippy and Rustdoc checks:

```bash
set -o pipefail && make lint 2>&1 | tee /tmp/2-2-4-time-travel-params-lint.log
```

- Run the full test suite:

```bash
set -o pipefail && make test 2>&1 | tee /tmp/2-2-4-time-travel-params-test.log
```

- Confirm the new behavioural suite is part of the green run. A focused spot
   check is useful during development, but it does not replace the full gate:

```bash
set -o pipefail && cargo test --test time_travel_params_bdd 2>&1 | tee /tmp/2-2-4-time-travel-params-bdd.log
```

Success criteria for close-out:

- The crate exposes `frankie::time_travel::TimeTravelParams` outside
  `crate::tui`.
- `TimeTravelParams::from_comment` or its documented equivalent returns a typed
  failure for missing metadata.
- The TUI still enters time travel for valid comments and fails gracefully for
  invalid ones.
- Unit tests and `rstest-bdd` behavioural tests cover happy path, unhappy path,
  and the line-number fallback edge case.
- `docs/frankie-design.md` and `docs/users-guide.md` reflect the final
  behaviour.
- `docs/roadmap.md` marks item 2.2.4 done only after the full validation suite
  passes.

## Approval gate

This plan has been reviewed and approved; implementation may proceed and the
plan is considered complete.
