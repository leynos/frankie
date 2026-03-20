# Extract reply templating into a shared library module

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETED (2026-03-14)

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Move reply-template rendering out of `src/tui/state/` into a top-level shared
library module so external hosts can render reply templates without depending
on text user interface (TUI) internals. After this change, a library consumer
can call a documented public API from `frankie`, while the TUI continues to
insert reply templates with unchanged behaviour and unchanged user-facing error
messages.

Success is observable when:

- reply-template rendering no longer lives under `src/tui/state/`;
- `src/lib.rs` re-exports a public reply-templating API from a non-TUI module;
- the TUI reply-draft flow still renders the same template output via the
  shared module;
- `rstest` coverage proves substitution, escaping, and error reporting through
  the shared API;
- `rstest-bdd` coverage proves the adapter behaviour still works for happy,
  unhappy, and edge paths;
- design notes in `docs/frankie-design.md` document the extraction and public
  contract;
- `docs/users-guide.md` is updated if template behaviour or error semantics
  become user-visible; and
- `make check-fmt`, `make lint`, and `make test` pass before the roadmap item
  is marked done.

## Constraints

- Follow `docs/adr-005-cross-surface-library-first-delivery.md`: the template
  renderer must live in a shared library module first, with the TUI reduced to
  an adapter.
- Keep this step scoped to extraction and publication of the renderer. Do not
  fold step 3.2.4's data transfer object (DTO) migration or step 3.2.5's
  default-template export into the same change unless a small additive seam is
  required to avoid duplicate work.
- Preserve current rendered output and current TUI error strings unless a
  change is explicitly documented in `docs/users-guide.md`.
- Preserve current TUI model-view-update (MVU) boundaries:
  - `src/tui/input.rs` remains input mapping only.
  - `src/tui/messages/` remains message definitions only.
  - `src/tui/app/` remains state transition and orchestration only.
  - `src/tui/components/` remains render-only.
- Do not leave the public API under `frankie::tui::...`; the new surface must
  be top-level and library-oriented.
- Keep every new Rust module under 400 lines and begin each new module with a
  `//!` module comment.
- Public functions and public error types must have Rustdoc comments, with a
  compilable example when it materially improves discoverability.
- Use `rstest` for unit and integration coverage and `rstest-bdd` v0.5.0 for
  behaviour-driven development (BDD) coverage.
- Shared BDD helpers returning `Result` must not use `assert!`; return
  explicit `Err(...)` values instead.
- Documentation changes must pass `make fmt`,
  `MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint`, and `make nixie`.
- Only mark roadmap step 3.2.3 done after implementation, documentation, and
  all required gates pass.

## Tolerances (exception triggers)

- Scope: if extracting the renderer cleanly requires touching more than 18
  files or more than 900 net new lines, stop and escalate with a narrower
  staged option.
- Interface: if the new public API cannot remain compatible with the current
  `render_reply_template(template_source, &ReviewComment)` shape for this step,
  stop and escalate rather than silently folding in the DTO redesign from step
  3.2.4.
- Dependencies: if any new runtime or development dependency is required, stop
  and escalate.
- Behaviour: if the TUI adapter cannot preserve current reply-template output
  or current user-visible error strings, stop and document the proposed delta
  before proceeding.
- Iterations: if any milestone still fails after three fix cycles, stop and
  escalate with logs.

## Risks

- Risk: the extraction accidentally widens scope into the DTO/default-template
  roadmap steps. Severity: medium. Likelihood: medium. Mitigation: keep the
  initial shared API on `ReviewComment` and move only the renderer and its
  error type in this step.
- Risk: the TUI continues compiling against stale re-exports in
  `crate::tui::state`, which would satisfy tests while failing the
  library-first intent. Severity: high. Likelihood: medium. Mitigation: update
  the TUI handler to import from the new top-level module directly and remove
  the old renderer symbols from `src/tui/state/mod.rs`.
- Risk: "escaping" is interpreted too narrowly and tests miss the real edge
  case. Severity: medium. Likelihood: medium. Mitigation: cover both literal
  brace escaping in templates and template-like text coming from comment data,
  proving there is no second rendering pass.
- Risk: public API docs drift from the actual exported path. Severity: medium.
  Likelihood: medium. Mitigation: add an external integration test that imports
  the renderer from `frankie`, not from an internal module path.
- Risk: a render-time failure case is not exercised because all current fields
  are present or defaulted. Severity: medium. Likelihood: medium. Mitigation:
  add a deliberate parse-success/render-failure template such as calling a
  string value like a function.

## Progress

- [x] (2026-03-13 00:00Z) Read the roadmap entry, ADR-005, the existing
      reply-drafting ExecPlan, the current `src/tui/state/reply_draft.rs`
      implementation, and the referenced testing guidance.
- [x] (2026-03-13 00:00Z) Drafted this ExecPlan for roadmap step 3.2.3.
- [x] (2026-03-14 00:00Z) Stage A: added shared-library tests, a public API
      integration test, and BDD regression scenarios covering substitution,
      escaping, and error reporting.
- [x] (2026-03-14 00:00Z) Stage B: extracted `ReplyTemplateError` and
      `render_reply_template` into `src/reply_template/` and re-exported them
      from `src/lib.rs`.
- [x] (2026-03-14 00:00Z) Stage C: switched the TUI reply-draft handler to the
      shared module and removed the old renderer export from
      `src/tui/state/mod.rs`.
- [x] (2026-03-14 00:00Z) Stage D: updated `docs/frankie-design.md`, marked
      roadmap step 3.2.3 done, and confirmed no `docs/users-guide.md` change
      was needed because user-visible behaviour and error strings stayed the
      same.
- [x] (2026-03-14 00:00Z) Stage E: passed `make fmt`,
      `MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint`,
      `make nixie`, `make check-fmt`, `make lint`, and `make test`.

## Surprises & Discoveries

- The current renderer already has the exact feature shape needed for this
  step, but it lives in `src/tui/state/reply_draft.rs` alongside
  `ReplyDraftState`, which makes the TUI module the accidental owner of shared
  behaviour.
- `src/lib.rs` currently re-exports export-format helpers from top-level
  modules, so a new top-level reply-template module aligns with an existing
  library-publication pattern.
- The current unit tests cover substitution and invalid syntax, but they do not
  yet prove render-time failure handling or escaping semantics.
- Existing behavioural coverage in
  `tests/template_reply_drafting_bdd.rs` already exercises TUI template
  insertion, so extending that suite is lower-risk than inventing a new TUI
  harness.
- `docs/users-guide.md` already documents inline reply drafting, including
  configured templates and AI rewrite. This step should update that guide only
  if extraction changes observable behaviour, terminology, or documented
  template semantics.

## Decision Log

- Decision (2026-03-13): introduce a dedicated top-level module named
  `src/reply_template/` and export it from `src/lib.rs`. Rationale: the module
  name is explicit, keeps the API independent of `src/tui/`, and gives later
  roadmap steps a stable home for `ReplyTemplateContext` and public defaults.
- Decision (2026-03-13): keep the public render function on
  `&ReviewComment` for this step. Rationale: step 3.2.4 exists specifically to
  replace `ReviewComment` with a DTO, so folding that change into 3.2.3 would
  blur acceptance boundaries and make regressions harder to isolate.
- Decision (2026-03-13): prove the public API through both module-local tests
  and an external integration test. Rationale: unit tests alone do not prove
  that the symbol is reachable from the library surface intended by ADR-005.
- Decision (2026-03-13): treat "escaping" as two separate guarantees:
  template authors can emit literal braces intentionally, and comment data that
  contains template syntax is rendered as data rather than recursively
  re-evaluated. Rationale: that is the narrowest interpretation that covers
  real user risk with MiniJinja-based templates.
- Decision (2026-03-13): keep roadmap step 3.2.3 focused on extraction and
  parity, not on introducing a new CLI surface. Rationale: ADR-005 allows a
  library-first step to land before the later adapter-consolidation step 3.2.6,
  and this roadmap item's acceptance only requires a public library API plus
  tests.

## Outcomes & Retrospective

Completed on 2026-03-14.

- Final public API path:
  - `frankie::reply_template::render_reply_template`
  - `frankie::render_reply_template`
  - `frankie::ReplyTemplateError`
- Implementation notes:
  - Moved the shared renderer out of `src/tui/state/reply_draft.rs` into the
    new top-level `src/reply_template/` module.
  - Kept `ReplyDraftState` and reply-draft mutation logic in TUI state.
  - Rewired `src/tui/app/reply_draft_handlers.rs` to import the shared module
    directly, preserving existing inline error strings.
- Tests added or extended:
  - `src/reply_template/tests.rs` for substitution, fallback defaults, literal
    brace escaping, non-recursive rendering of template-like comment data,
    parse-time failures, and render-time failures.
  - `tests/reply_template_public_api.rs` proving the API is reachable from both
    the crate-root re-export and the public module path.
  - `tests/template_reply_drafting_bdd.rs` and
    `tests/features/template_reply_drafting.feature` for adapter regression on
    invalid syntax and escaped/template-like literal output.
- User-visible changes:
  - None intended. Inline reply drafting continues to render templates and
    report template failures with the same TUI-visible prefix.
- Gate results:
  - `make fmt`
  - `MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint`
  - `make nixie`
  - `make check-fmt`
  - `make lint`
  - `make test`
  - `cargo test --test reply_template_public_api`
  - `cargo test --test template_reply_drafting_bdd`
  - `cargo test reply_template --lib`
  - Final `nextest` summary: `812 passed, 1 skipped`.

## Context and orientation

The existing reply-template implementation is concentrated in
`src/tui/state/reply_draft.rs`. That file currently owns two distinct concerns:

1. `ReplyDraftState`, which is TUI state and should stay TUI-specific.
2. `ReplyTemplateError` and `render_reply_template`, which are shared template
   concerns and are the extraction target for this step.

The main call site is `src/tui/app/reply_draft_handlers.rs`, where template
insertion currently imports `render_reply_template` from `crate::tui::state`.
That handler should become a thin adapter over a top-level library module.

The repository already has a pattern for this kind of split:

- `src/export/template.rs` contains shared MiniJinja-based export rendering.
- `src/lib.rs` re-exports public library helpers from non-TUI modules.
- `tests/template_reply_drafting_bdd.rs` proves end-user TUI behaviour via
  `rstest-bdd`.

The new reply-template module should follow the same pattern:

- shared logic under a top-level non-TUI module;
- TUI imports from that shared module;
- public symbols re-exported from `src/lib.rs`;
- adapter regression tests proving no behavioural drift.

The following reference documents should guide implementation choices:

- `docs/adr-005-cross-surface-library-first-delivery.md` for the library-first
  requirement;
- `docs/execplans/3-2-1-template-based-reply-drafting.md` for the original TUI
  contract;
- `docs/rust-testing-with-rstest-fixtures.md` for fixture and parametrization
  style;
- `docs/rstest-bdd-users-guide.md` for `rstest-bdd` v0.5.0 scenario structure;
- `docs/rust-doctest-dry-guide.md` for public Rustdoc example discipline;
- `docs/reliable-testing-in-rust-via-dependency-injection.md` and
  `docs/two-tier-testing-strategy-for-an-octocrab-github-client.md` for the
  library/adapter test split;
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for avoiding a
  larger mixed-responsibility `reply_draft.rs`; and
- `docs/users-guide.md` plus `docs/frankie-design.md` for documentation updates.

## Plan of work

### Stage A: Define the extraction target with failing tests first

Start by adding tests that express the intended shared-library contract before
moving code. The first red state is acceptable if it fails because the new
module or new public re-export does not exist yet.

Add or update focused tests in these layers:

1. Shared-library tests with `rstest`:
   - add module-local tests next to the extracted renderer, or in a dedicated
     sibling test module under `src/reply_template/`;
   - cover placeholder substitution for `comment_id`, `reviewer`, `file`,
     `line`, and `body`;
   - cover fallback defaults for missing `author`, `file_path`, `line_number`,
     and `body`;
   - cover literal-brace escaping in template source;
   - cover comment body text containing `{{ nested }}` to prove no recursive
     template evaluation occurs; and
   - cover both parse-time failure and render-time failure.
2. External integration coverage with `rstest`:
   - add `tests/reply_template_public_api.rs` that imports the renderer from
     `frankie`, not from `frankie::tui::...`;
   - keep at least one test intentionally small and documentation-oriented so a
     future refactor cannot accidentally hide the public API.
3. Behavioural regression coverage with `rstest-bdd`:
   - extend `tests/template_reply_drafting_bdd.rs` and
     `tests/features/template_reply_drafting.feature`, or add a narrowly scoped
     sibling feature file;
   - include one happy path proving the TUI still inserts rendered shared
     output;
   - include one unhappy path proving invalid template syntax still surfaces a
     user-readable TUI error; and
   - include one edge path proving escaped literal braces or template-like body
     text render correctly in the TUI.

Go/no-go for Stage A:

- Go when the intended module path and public API are unambiguous from the test
  failures.
- No-go if the tests force a DTO migration or default-template export to move
  forward; that would mean the stage is bleeding into later roadmap items.

### Stage B: Extract the shared renderer into a top-level module

Create a new top-level library module, preferably:

```plaintext
src/reply_template/mod.rs
```

Move `ReplyTemplateError` and `render_reply_template` into that module with
their existing behaviour preserved. Keep the implementation intentionally small:

- continue to use `MiniJinja` with auto-escaping disabled to match current
  behaviour;
- keep the same placeholder names so configured user templates do not change;
- keep the same default values for missing comment fields; and
- preserve the distinction between syntax errors and render failures.

Publish the module from `src/lib.rs`:

- add `pub mod reply_template;`;
- add a root re-export such as
  `pub use reply_template::{ReplyTemplateError, render_reply_template};`.

Add Rustdoc comments that show a minimal example using `ReviewComment`. The
example should compile without importing any TUI modules.

Keep `ReplyDraftState` and `ReplyDraftError` in `src/tui/state/reply_draft.rs`.
Do not move TUI state into the shared module.

Go/no-go for Stage B:

- Go when `frankie::render_reply_template(...)` and/or
  `frankie::reply_template::render_reply_template(...)` compile from an
  integration test.
- No-go if the extracted module still depends on `crate::tui::...`; that would
  violate the purpose of the step.

### Stage C: Rewire the TUI adapter and remove the old TUI-local export

Once the shared module exists, update the TUI to depend on it directly.

Expected code changes:

- change `src/tui/app/reply_draft_handlers.rs` to import
  `ReplyTemplateError` and `render_reply_template` from the new shared module;
- remove the renderer and template-error re-export from
  `src/tui/state/mod.rs`;
- delete the extracted symbols from `src/tui/state/reply_draft.rs`;
- keep all TUI error mapping in the handler so the UI remains the adapter layer.

Run the existing reply-drafting TUI tests after this step. The behaviour should
remain identical. If any user-facing error string changes, update both the
tests and `docs/users-guide.md`, and record the reason in `Decision Log`.

### Stage D: Update design and user documentation

Refresh documentation after code and tests are green.

Required documentation changes:

1. `docs/frankie-design.md`
   - add or update a short section describing the new
     `reply_template` shared module;
   - record that TUI reply drafting now consumes a library-first renderer per
     ADR-005;
   - note the public API path and the fact that step 3.2.4 will later replace
     `ReviewComment` with a DTO.
2. `docs/users-guide.md`
   - inspect the inline reply-drafting section;
   - update only if users need to know about changed template semantics,
     escaping guidance, or changed error messages;
   - if there is no user-visible change, leave the behaviour description
     stable rather than inventing churn.
3. `docs/roadmap.md`
   - mark step 3.2.3 as done only after the implementation and all gates below
     have passed.

### Stage E: Validate end to end and capture evidence

Use targeted iteration first, then run the full repository gates. Because this
repository truncates long command output, route commands through `tee` with
`set -o pipefail`.

Suggested targeted checks during implementation:

```bash
set -o pipefail; cargo test --test reply_template_public_api 2>&1 | tee /tmp/3-2-3-public-api.log
set -o pipefail; cargo test --test template_reply_drafting_bdd 2>&1 | tee /tmp/3-2-3-bdd.log
```

Required final documentation and repo gates:

```bash
set -o pipefail; make fmt 2>&1 | tee /tmp/3-2-3-fmt.log
set -o pipefail; MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint 2>&1 | tee /tmp/3-2-3-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/3-2-3-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/3-2-3-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/3-2-3-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/3-2-3-test.log
```

Completion evidence should include:

- the new public API path in `src/lib.rs`;
- passing unit, integration, and BDD coverage for substitution, escaping, and
  error reporting;
- updated design notes, plus `docs/users-guide.md` only if behaviour changed;
- `docs/roadmap.md` showing `3.2.3` marked done; and
- a clean `git status --short` apart from the intended implementation changes.
