# Provide full-screen diff context with jump navigation

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

PLANS.md is not present in the repository root, so no additional plan
governance applies.

## Purpose / Big picture

Add a full-screen diff context screen to the review text user interface (TUI)
so a reviewer can open complete change context from the review list, jump
between diff hunks with keyboard shortcuts, and return to the list without
losing selection. Success is visible when pressing the context shortcut renders
a full-screen diff view, next/previous hunk shortcuts move between hunks, and a
profiling run confirms rendering stays under 100ms on the reference dataset.

## Constraints

- Keep the model-view-update (MVU) split intact: update logic stays in
  `src/tui/app/mod.rs`, rendering stays in `src/tui/app/rendering.rs`, and
  components remain pure view helpers.
- Every new module begins with a `//!` module-level comment.
- No single file may exceed 400 lines; split into feature-focused modules if
  needed.
- Use `rstest` for unit tests and `rstest-bdd` v0.3.2 for behavioural tests.
- Any filesystem access must use `cap_std`, `cap_std::fs_utf8`, and `camino`.
- Avoid adding new dependencies beyond the existing stack; if unavoidable,
  escalate before proceeding.
- Documentation updates must follow the en-GB style guide, wrap at 80 columns,
  and pass `make markdownlint`, `make fmt`, and `make nixie`.
- Use Makefile targets for validation (`make check-fmt`, `make lint`,
  `make test`).

## Tolerances (exception triggers)

- Scope: if implementation needs more than 18 files or 750 net new lines, stop
  and escalate.
- Interface: if any public API signature must change, stop and escalate.
- Dependencies: if a new external dependency is required, stop and escalate.
- Tests: if tests still fail after two fix attempts, stop and escalate with the
  latest failure output.
- Ambiguity: if the source of “full-screen diff context” is unclear (comment
  hunks vs full file diffs) and the choice materially affects user experience
  (UX), stop and ask for confirmation.

## Risks

- Risk: the reference dataset for performance profiling is not defined in the
  repo, making the 100ms target hard to validate. Severity: high. Likelihood:
  medium. Mitigation: locate or define the reference dataset in
  `tests/fixtures/` (or agreed location) and document the profiling command in
  this plan and `docs/frankie-design.md`.
- Risk: syntect highlighting across multiple hunks increases render time.
  Severity: medium. Likelihood: medium. Mitigation: pre-wrap and cache rendered
  hunks when entering the full-screen view so `view()` is mostly string
  assembly.
- Risk: keyboard shortcuts collide with existing bindings or future planned
  shortcuts (AI actions). Severity: medium. Likelihood: low. Mitigation: use
  non-conflicting keys (for example `[` and `]`), update help text, and cover
  in behavioural tests.
- Risk: comments without `diff_hunk` data lead to empty screens. Severity: low.
  Likelihood: medium. Mitigation: render a clear placeholder and allow
  returning to the list.

## Progress

- [x] (2026-01-17) Draft ExecPlan created.
- [x] (2026-01-17) Inspected the TUI view and confirmed diff context sources.
- [x] (2026-01-17) Defined hunk model + navigation helpers with unit tests.
- [x] (2026-01-17) Implemented full-screen diff context view and navigation.
- [x] (2026-01-17) Updated design docs, user guide, and roadmap entry.
- [x] (2026-01-17) Ran formatting, linting, testing, and documentation
  validation.
- [x] (2026-01-17) Recorded profiling results in outcomes.

## Surprises & discoveries

- `make test` requires `cargo-nextest` in the local environment.
- `make markdownlint` expects `markdownlint-cli2` on `PATH`; use
  `MDLINT=/root/.bun/bin/markdownlint-cli2` if it is not available.
- The performance check is best captured as an ignored test to keep the normal
  test suite fast while still validating the 100ms requirement on demand.

## Decision log

- Decision: Treat each `ReviewComment.diff_hunk` as a diff hunk for navigation
  in the full-screen context view, ordered by file path and line number, with
  duplicates removed by `(file_path, diff_hunk)` identity. Rationale: this uses
  existing data without additional Git access and aligns with Phase 2 scope.
  Date/Author: 2026-01-17, plan author.
- Decision: Use `c` to enter full-screen diff context and `[`/`]` to move to
  previous/next hunk, with `Esc` returning to the list. Rationale: `c` matches
  the design doc; bracket keys avoid conflicts with AI shortcuts. Date/Author:
  2026-01-17, plan author.
- Decision: Capture the performance requirement with an ignored test at
  `tests/diff_context_render_perf.rs` using the reference dataset in
  `tests/fixtures/diff_context_reference.json`. Rationale: keep the default
  test suite fast while enabling repeatable local profiling. Date/Author:
  2026-01-17, implementation.

## Outcomes & retrospective

- Delivered full-screen diff context view with hunk navigation and cached
  rendering for fast redraws.
- Added rstest unit coverage for hunk extraction/navigation and rstest-bdd
  scenarios in `tests/full_screen_diff_context_bdd.rs` with
  `tests/features/full_screen_diff_context.feature`.
- Profiling: ran cargo test -p frankie diff_context_render_perf -- --ignored
  --nocapture; passed (under 100ms) on 2026-01-17. Dataset:
  tests/fixtures/diff_context_reference.json.
- Validation: `make fmt`, `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, and `make test` (logs in `/tmp/frankie-*.log`).

## Context and orientation

The review TUI lives under `src/tui/`. `ReviewApp` in `src/tui/app/mod.rs`
contains model-view-update (MVU) state and update logic, while
`src/tui/app/rendering.rs` builds strings for the terminal. Keyboard inputs are
mapped in `src/tui/input.rs` to `AppMsg` variants in `src/tui/messages.rs`. The
current UI renders a review list (`ReviewListComponent`) and comment detail
pane (`CommentDetailComponent`) with syntax highlighting via `CodeHighlighter`.
Review comments carry a `diff_hunk` string in `src/github/models/mod.rs`, which
is already used for inline code context. Behavioural tests live under `tests/`
with Gherkin feature files in `tests/features/`.

## Plan of work

Stage A: confirm data and UX decisions. Review `docs/roadmap.md` and
`docs/frankie-design.md` to confirm the intent for full-screen context and
keyboard shortcuts. Inspect `ReviewComment` to verify `diff_hunk` availability
and determine how to order and deduplicate hunks. If the reference dataset for
profiling is not present, define a fixture dataset and document its location
and intended size; escalate if this changes scope.

Stage B: scaffolding and tests. Add a hunk model (for example `DiffHunk`) and
index helpers, plus unit tests using `rstest` for hunk extraction,
deduplication, ordering, and navigation boundaries. Add behavioural tests using
`rstest-bdd` v0.3.2 with a new feature file (for example
`tests/features/full_screen_diff_context.feature`) and scenario module (for
example `tests/full_screen_diff_context_bdd.rs`) that drive the TUI through
opening the full-screen view, moving to next/previous hunks, and exiting back
to the list.

Stage C: implementation. Introduce a full-screen diff context component (for
example `src/tui/components/diff_context.rs`) that renders a header (file path,
hunk index) and the current hunk body, using `CodeHighlighter` and
`wrap_code_block` for consistent wrapping. Add new app state to track view
mode, current hunk index, and pre-rendered hunk strings to keep render time
bounded. Update `AppMsg`, `map_key_to_message`, and `ReviewApp` navigation
handlers to support entering/exiting the full-screen view and moving between
hunks while preserving list selection. Update `render_status_bar` and
`render_help_overlay` to include the new shortcuts.

Stage D: documentation and close-out. Record design decisions in
`docs/frankie-design.md`, update `docs/users-guide.md` with the new full-screen
context behaviour and shortcuts, and mark the Phase 2 roadmap entry as done in
`docs/roadmap.md`. Capture profiling results in this ExecPlan’s outcomes.

Each stage ends with the validation steps listed below. Do not proceed to the
next stage if the current stage validation fails.

## Concrete steps

1. Review existing UI state, messages, and view rendering:

   rg -n "ReviewApp|render\_|AppMsg|map_key_to_message" src/tui
   rg -n "diff_hunk" src/github/models/mod.rs

1. Define hunk extraction and navigation helpers with unit tests (rstest).

1. Add behavioural tests with `rstest-bdd` v0.3.2 and a Gherkin feature file
   covering:

   - Entering full-screen context from the review list.
   - Moving between hunks with the new shortcuts.
   - Handling comments without diff hunks.
   - Exiting back to the list without losing selection.

1. Implement the full-screen diff context component, view mode state, and
   navigation handlers. Ensure rendering uses cached/pre-wrapped strings to
   keep `view()` fast.

1. Add a local profiling check for rendering time against the reference
   dataset (for example, an ignored test or a small profiling helper binary
   that logs elapsed milliseconds). Document the dataset path and expected
   output in this ExecPlan and the design document.

1. Update documentation (`docs/frankie-design.md`, `docs/users-guide.md`) and
   mark the roadmap entry as done.

1. Run validation. For long outputs, use `tee` and `set -o pipefail`:

   set -o pipefail
   make check-fmt 2>&1 | tee /tmp/frankie-check-fmt.log
   make lint 2>&1 | tee /tmp/frankie-lint.log
   make test 2>&1 | tee /tmp/frankie-test.log

1. If documentation changed, run documentation validators:

   set -o pipefail
   make markdownlint 2>&1 | tee /tmp/frankie-markdownlint.log
   make fmt 2>&1 | tee /tmp/frankie-docs-fmt.log
   make nixie 2>&1 | tee /tmp/frankie-nixie.log

1. Run the profiling command and record results (example, update with actual
   command and dataset path once defined):

   cargo test -p frankie diff_context_render_perf -- --ignored --nocapture

1. Update the Progress and Outcomes sections with the actual results.

## Validation and acceptance

Acceptance is satisfied when the following are true:

- Pressing the context shortcut opens a full-screen diff view showing the
  current hunk with file metadata.
- Next/previous hunk shortcuts move between hunks and update the header to
  reflect the new hunk index.
- Exiting full-screen context returns to the review list with the same
  selection.
- Unit tests cover hunk extraction, ordering, navigation bounds, and empty
  hunk behaviour.
- Behavioural tests cover entering, navigating, and exiting the full-screen
  context view.
- Profiling confirms full-screen diff rendering stays under 100ms on the
  reference dataset.
- `make check-fmt`, `make lint`, and `make test` succeed.
- Documentation updates pass `make markdownlint`, `make fmt`, and `make nixie`.

Quality criteria:

- Tests: rstest unit tests and rstest-bdd scenarios for the new behaviour.
- Lint/typecheck: `make lint` clean.
- Formatting: `make check-fmt` clean.
- Performance: profiling output shows \<100ms render time on the reference
  dataset.

## Idempotence and recovery

All steps are re-runnable. If tests fail, inspect the log files under `/tmp/`,
apply fixes, and rerun the same commands. If performance regresses, re-check
hunk caching, reduce per-render allocations, and rerun profiling. If the
reference dataset definition changes, update the ExecPlan and design document
before proceeding.

## Artefacts and notes

Example full-screen context header (illustrative):

```
File: src/main.rs  Hunk 2/5  (press [ / ] to jump)
```

Example placeholder when no hunks are available:

```
(No diff context available for this comment)
```

Example navigation assertion in unit tests:

```
assert_eq!(state.current_index(), 1);
```

## Interfaces and dependencies

- New component module (suggested): `src/tui/components/diff_context.rs` with a
  `DiffContextComponent` and a `DiffContextViewContext` carrying the hunk list,
  current index, max width, and max height.
- New state types (suggested): `DiffHunk` plus index helpers (plain `usize`)
  in a feature-local module to avoid integer soup.
- New `AppMsg` variants (suggested): `ShowDiffContext`, `HideDiffContext`,
  `NextHunk`, `PreviousHunk`.
- Update `src/tui/input.rs` to map `c`, `[`, `]`, and `Esc` appropriately.
- Update `src/tui/app/mod.rs` and `src/tui/app/rendering.rs` to handle the new
  view mode and render the full-screen diff context.
- Update `src/tui/components/mod.rs` to export the new component.
- If profiling uses fixture files, store them under `tests/fixtures/` and load
  using `cap_std::fs_utf8` and `camino::Utf8PathBuf`.

## Revision note

Initial draft created to cover full-screen diff context, hunk navigation,
profiling, tests, and documentation updates.
