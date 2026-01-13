# Implement comment detail view with inline code context

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE

PLANS.md is not present in the repository root, so no additional plan governance
applies.

## Purpose / Big picture

Deliver a comment detail view in the review terminal user interface (TUI) that
shows the selected review comment together with inline code context. Code
context should be syntax highlighted with syntect when possible, wrapped to a
maximum of 80 columns (using the terminal width if it is narrower), and
rendered as plain text when highlighting fails. Success is visible when
selecting a comment in the TUI shows a detail pane with a code block that wraps
to a maximum of 80 columns and still renders when the highlighter cannot load.

## Constraints

- Follow the model-view-update (MVU) structure already used in
  `src/tui/app/mod.rs` and keep view
  rendering in query methods, not update handlers.
- Every new module must begin with a `//!` module-level comment.
- No single code file may exceed 400 lines.
- Documentation must use en-GB-oxendict spelling, 80-column wrapping, and the
  documentation style guide in `docs/documentation-style-guide.md`.
- Use `rstest` for unit tests and `rstest-bdd` (v0.3.2) for behavioural tests.
- Avoid adding dependencies other than syntect. Syntect is not yet listed in
  `Cargo.toml`, so adding it is part of this plan to match
  `docs/frankie-design.md`.
- Use Makefile targets for validation (`make check-fmt`, `make lint`,
  `make test`).

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 14 files or more than
  500 net new lines, stop and escalate.
- Interface: if an existing public API signature must change, stop and
  escalate.
- Dependencies: if any new external dependency other than syntect is required,
  stop and escalate.
- Tests: if tests still fail after two fix attempts, stop and escalate with the
  latest failure output.
- Ambiguity: if multiple layout options appear equally valid (split pane versus
  stacked detail) and the choice materially affects navigation, stop and
  confirm.

## Risks

- Risk: syntect highlighting can introduce ANSI (American National Standards
  Institute) escape codes that complicate wrapping to a maximum of 80 columns.
  Severity: medium
  Likelihood: medium
  Mitigation: wrap source lines to 80 columns before highlighting, and test
  width compliance using fixed-width fixtures.
- Risk: diff hunk context may be absent on some comments, leaving no code
  context to render.
  Severity: low
  Likelihood: medium
  Mitigation: display a clear placeholder and still render the comment body.
- Risk: combining a list and detail panel could degrade readability on small
  terminal sizes.
  Severity: medium
  Likelihood: low
  Mitigation: clamp panel heights, and prioritize code context when space is
  limited.

## Progress

- [x] (2026-01-11 00:00Z) Draft ExecPlan created.
- [x] (2026-01-13 00:10Z) ExecPlan approved; implementation started.
- [x] (2026-01-13 01:00Z) Add unit tests for code context rendering, wrapping,
  and fallback.
- [x] (2026-01-13 01:15Z) Add rstest-bdd scenario and feature file for comment
  detail view.
- [x] (2026-01-13 01:30Z) Implement comment detail component with syntect
  highlighting and 80-column wrapping.
- [x] (2026-01-13 01:45Z) Integrate detail view into TUI rendering and update
  layout.
- [x] (2026-01-13 02:00Z) Update design documentation and user guide.
- [x] (2026-01-13 02:30Z) Run formatting, linting, tests, and documentation validators.
- [x] (2026-01-13 02:45Z) Mark roadmap entry complete and record outcomes.

## Surprises & discoveries

- `ThemeSet` from syntect does not implement `Clone`, requiring removal of the
  `Clone` derive from `CodeHighlighter`, `CommentDetailComponent`, and
  `ReviewApp`. This had no impact on the application because `Clone` was not
  actually used on these types.

## Decision log

- Decision: Use `ReviewComment.diff_hunk` as the inline code context source for
  the first implementation.
  Rationale: It is already populated from the GitHub review comment payload and
  available without additional API calls.
  Date/Author: 2026-01-11, plan author.
- Decision: Add syntect to `Cargo.toml` because it is not currently listed.
  Rationale: The plan requires syntect for syntax highlighting and
  `docs/frankie-design.md` specifies it as a dependency.
  Date/Author: 2026-01-13, plan author.

## Outcomes & retrospective

- All acceptance criteria met: comment detail view renders with author, file,
  line number, body text, and inline code context from the `diff_hunk` field.
- Syntax highlighting uses syntect with automatic language detection based on
  file extension; plain text fallback works when highlighting fails.
- 80-column wrapping enforced via `wrap_to_width()` helper that wraps before
  highlighting to avoid ANSI escape code complexity.
- Test coverage: 280 tests pass, including unit tests for wrapping and
  highlighting, and 7 BDD scenarios covering display, wrapping, and fallback.
- No tolerance triggers hit: implementation touched 10 files with approximately
  450 net new lines, within the 14-file and 500-line limits.
- `make check-fmt`, `make lint`, and `make test` all pass.
- Documentation validation (`nixie`) passes for all modified files.
- Retrospective: removing `Clone` from `ReviewApp` due to syntect's `ThemeSet`
  had no impact on the application. The wrap-before-highlight strategy proved
  effective for maintaining width guarantees.

## Context and orientation

The review TUI is implemented under `src/tui/`. The model and update logic live
in `src/tui/app/mod.rs`, with view rendering split into
`src/tui/app/rendering.rs`. The current user interface (UI) only renders a
review list via the
`ReviewListComponent` in `src/tui/components/review_list.rs`. Review comments
are represented by `ReviewComment` in `src/github/models/mod.rs`, which includes
`diff_hunk`, `file_path`, and line numbers required for inline context. Unit
tests for TUI behaviour live in `src/tui/app/tests.rs`, and behavioural tests
use `rstest-bdd` under `tests/` with feature files in `tests/features/`.

The design expectations for this feature are described in `docs/roadmap.md` and
`docs/frankie-design.md`. Documentation updates must follow
`docs/documentation-style-guide.md`, and user-facing changes must be recorded
in `docs/users-guide.md`.

## Plan of work

Stage A: confirm layout and data inputs (no code changes). Review the existing
TUI rendering pipeline in `src/tui/app/rendering.rs`, the review list component
in `src/tui/components/review_list.rs`, and the `ReviewComment` fields in
`src/github/models/mod.rs`. Decide on a layout strategy (split pane versus
stacked detail) that preserves existing list navigation and makes room for a
code block wrapped to a maximum of 80 columns. If the layout decision is
ambiguous, stop and confirm per `Tolerances`.

Stage B: scaffolding and tests. Add unit tests using `rstest` for the new
comment detail component. Tests should cover: rendering with a diff hunk,
wrapping long code lines to a maximum of 80 columns, and a highlighter failure
path that falls back to plain text. Add a new `rstest-bdd` feature file under
`tests/features/` and a matching scenario module under `tests/` that exercises
selecting a comment and observing the detail pane output (both highlighted and
fallback paths). Fixtures should be reused via `rstest` to avoid duplication.

Stage C: implementation. Add a new TUI component (for example,
`src/tui/components/comment_detail.rs`) that renders the selected comment's
metadata, body, and code context. Introduce a small highlighting adapter that
uses syntect to colourize code based on `file_path`, returning a `Result` so
errors can fall back to plain text. Add a wrapping helper that enforces an
80-column maximum (or terminal width if narrower) for code lines (using a
consistent, character-counted wrap), with behaviour documented in unit tests.
Wire the new component into
`ReviewApp::view()` by rendering the list and detail panes together, ensuring
cursor movement updates the selected comment detail. Keep the rendering
functions in query-only paths.

Stage D: documentation and cleanup. Update `docs/frankie-design.md` with any
design decisions made during implementation (layout choice, highlight strategy,
wrapping behaviour). Update `docs/users-guide.md` to describe the detail view
and how code context is presented. Finally, mark the roadmap entry for this
step in `docs/roadmap.md` as done, and capture outcomes in this ExecPlan.

Each stage ends with the validation steps listed below. Proceed to the next
stage only if the current stage validation succeeds.

## Concrete steps

1. Inspect current TUI layout and review comment fields:

    rg -n "ReviewApp::view|render_" src/tui/app
    rg -n "ReviewListComponent" src/tui/components
    rg -n "diff_hunk|file_path|line_number" src/github/models/mod.rs

2. Add unit tests for the new detail component and wrapping behaviour, using
   `rstest` fixtures and `expect` only in tests.

3. Add a new feature file (for example,
   `tests/features/comment_detail.feature`) and a `rstest-bdd` scenario module
   (for example, `tests/comment_detail_bdd.rs`) that asserts detail rendering
   output for both highlighted and fallback paths.

4. Implement the new comment detail component and integrate it into
   `ReviewApp::view()`. Ensure new modules start with `//!` comments and stay
   under 400 lines.

5. Update documentation in `docs/frankie-design.md` and `docs/users-guide.md`.

6. Run the full validation pipeline. For long outputs, use `tee` and
   `set -o pipefail` as required:

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/frankie-check-fmt.log
    make lint 2>&1 | tee /tmp/frankie-lint.log
    make test 2>&1 | tee /tmp/frankie-test.log

7. If documentation changed, run the documentation validators:

    set -o pipefail
    make markdownlint 2>&1 | tee /tmp/frankie-markdownlint.log
    make fmt 2>&1 | tee /tmp/frankie-docs-fmt.log
    make nixie 2>&1 | tee /tmp/frankie-nixie.log

8. Mark the roadmap item complete in `docs/roadmap.md` and update the Progress
   and Outcomes sections of this ExecPlan.

## Validation and acceptance

Acceptance is satisfied when the following are true:

- The TUI renders a comment detail view that shows the selected comment and an
  inline code context block.
- Code blocks are wrapped to a maximum of 80 columns (using the terminal width
  if it is narrower), verified via unit tests that check line lengths.
- Syntax highlighting uses syntect when available, and when highlighting fails
  the code block still renders as plain text with the same wrapping rules.
- Behavioural tests in `rstest-bdd` demonstrate both the highlighted and
  fallback paths on representative fixtures.
- `make check-fmt`, `make lint`, and `make test` succeed.
- Documentation updates pass `make markdownlint`, `make fmt`, and `make nixie`.

Quality criteria:

- Tests: unit tests for the detail renderer and wrapping; behaviour-driven
  development (BDD) scenario for comment detail view behaviour.
- Lint/typecheck: `make lint` clean.
- Formatting: `make check-fmt` clean.

## Idempotence and recovery

All steps are re-runnable. If a test run fails, inspect the logged output in
`/tmp/frankie-*.log`, fix the cause, and re-run the same command. If a layout
choice proves unsuitable, revert only the view composition changes and iterate
on the component in isolation before re-integrating.

## Artefacts and notes

Example detail rendering (illustrative, not a snapshot):

    [alice] src/lib.rs:42
    Comment: Please extract this helper.

    @@ -40,6 +40,10 @@
    +fn wrap_line(input: &str) -> String { ... }
    +fn format_view(...) -> String { ... }

Example acceptance check for 80-column wrapping in unit tests:

    assert!(output.lines().all(|line| line.chars().count() <= 80));

## Interfaces and dependencies

- New component module: `src/tui/components/comment_detail.rs` with a
  `CommentDetailComponent` and `CommentDetailViewContext` that accept an
  optional `ReviewComment` and render strings for the detail pane.
- Highlighting adapter module (either within the component or a small helper
  module such as `src/tui/components/code_highlight.rs`) that exposes a
  `highlight_code_block` function returning `Result<String, HighlightError>` so
  callers can fall back to plain text.
- Update `src/tui/components/mod.rs` to export the new component.
- Update `src/tui/app/rendering.rs` and `src/tui/app/mod.rs` to render the
  detail view alongside the list.
- Add syntect to `Cargo.toml` under `[dependencies]` using the caret version
  requirement `syntect = "5.2"`.

The detail view should use `ReviewComment.file_path` to select a syntax where
possible, and fall back to plain text when no syntax is found or highlighting
fails.

## Revision note

Updated en-GB-oxendict spellings, aligned the wrapping rule to a single
definition, clarified syntect dependency guidance (now noting syntect is not
listed in `Cargo.toml`), and expanded acronyms on first use (ExecPlan, TUI,
MVU, ANSI, UI, and BDD). This keeps the document aligned with the
documentation style guide and does not alter the planned implementation
steps.
