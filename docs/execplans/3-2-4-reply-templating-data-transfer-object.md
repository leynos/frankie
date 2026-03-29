# Make reply templating consume a library data transfer object

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IMPLEMENTED (2026-03-28)

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Roadmap step 3.2.4 exists to stop the public reply-template renderer from
depending on `frankie::ReviewComment` directly. After this change, library
consumers will render templates from a stable public DTO such as
`ReplyTemplateContext`, while TUI reply drafting will remain an adapter that
maps the selected `ReviewComment` into that DTO before rendering. The
user-visible behaviour must stay the same: the same template variables render
the same text, the same defaults are applied for missing comment fields, and
the same inline TUI error prefix is shown when rendering fails.

Success is observable when:

- `src/reply_template/` exposes a public DTO that owns the renderer input
  contract;
- `render_reply_template` renders from that DTO rather than `&ReviewComment`;
- the TUI reply-draft adapter maps `ReviewComment` into the DTO without
  changing rendered output;
- `rstest` unit and integration tests prove both direct DTO rendering and the
  `ReviewComment` to DTO mapping contract;
- `rstest-bdd` v0.5.0 scenarios prove the TUI adapter still behaves the same
  for happy, unhappy, and edge cases;
- `docs/frankie-design.md` records the DTO decision and the no-new-CLI
  rationale for this narrow library-contract step;
- `docs/users-guide.md` is updated only if the observable reply-drafting
  behaviour or wording changes; and
- `docs/roadmap.md` marks `3.2.4` done only after all documentation and gates
  pass.

## Constraints

- Follow `docs/adr-005-cross-surface-library-first-delivery.md`: the reusable
  reply-template contract must stay owned by a shared library module, with TUI
  logic acting as an adapter.
- Treat `docs/execplans/3-2-3-extract-reply-templating-into-library-module.md`
  as a completed prerequisite. Do not re-open step 3.2.3 work except where a
  tiny follow-on adjustment is required for the DTO migration.
- Keep this step scoped to the renderer input contract. Do not fold in roadmap
  step 3.2.5 (public default-template export) or roadmap step 3.2.6 (broader
  surface cleanup).
- Preserve the reply-template variable names already documented and exercised
  by tests: `comment_id`, `reviewer`, `file`, `line`, and `body`.
- Preserve current rendered output for equivalent inputs, including the
  existing fallback defaults:
  - `reviewer` defaults to `"reviewer"`;
  - `file` defaults to `"(unknown file)"`;
  - `line` defaults to the empty string;
  - `body` defaults to the empty string.
- Preserve current TUI-visible error wording, especially the inline prefix
  `Reply template rendering failed:`.
- Keep TUI MVU boundaries intact:
  - `src/tui/input.rs` remains input mapping only.
  - `src/tui/messages/` remains message definitions only.
  - `src/tui/app/` remains orchestration and state transitions only.
  - `src/tui/components/` remains render-only.
- Every new Rust module must begin with a `//!` module comment and stay under
  400 lines.
- Public types and public functions must have Rustdoc comments, and any updated
  doctests must compile from the perspective of an external crate.
- Use `rstest` fixtures for unit and integration coverage and `rstest-bdd`
  v0.5.0 for behavioural coverage.
- Shared BDD helpers returning `Result` must not use `assert!`; return
  explicit `Err(...)` values instead.
- No new runtime or development dependency is allowed for this step.
- Only mark roadmap step `3.2.4` done after implementation, documentation,
  and all required gates pass.

## Tolerances (exception triggers)

- Scope: if the change requires touching more than 16 files or more than 850
  net new lines, stop and re-scope before implementation.
- Contract: introducing `ReplyTemplateContext` and changing the renderer to use
  it is in scope; changing template variable names, fallback defaults, or TUI
  error text is not. If any of those behaviours must change, stop and escalate.
- Surface area: if satisfying ADR-005 for this step appears to require a new
  standalone CLI mode, stop and document why. This roadmap item is a library
  DTO migration, not a new CLI feature.
- Testing: if the existing reply-template BDD harness cannot cover the adapter
  mapping work with additive steps and scenarios, stop after one prototype and
  decide whether a narrower integration test gives better coverage.
- Iterations: if any milestone still fails after three fix cycles, stop and
  escalate with the failing logs.

## Risks

- Risk: the change becomes a breaking public API migration without enough
  documentation or test evidence. Severity: high. Likelihood: medium.
  Mitigation: update Rustdoc examples, crate-root integration tests, and the
  ADR-005 design note in the same change as the signature migration.
- Risk: DTO design simply mirrors `ReviewComment` too literally, leaving the
  renderer responsible for fallback/default logic again. Severity: medium.
  Likelihood: medium. Mitigation: make the DTO carry the normalized render
  values the template engine actually needs, and keep translation from
  `ReviewComment` in one explicit adapter path.
- Risk: TUI parity regresses even though direct DTO tests pass. Severity: high.
  Likelihood: medium. Mitigation: keep and extend
  `tests/template_reply_drafting_bdd.rs` so the adapter path still proves the
  same observable output and error text.
- Risk: the implementation quietly drifts into step 3.2.5 by exposing or
  reshaping default template lists while touching `reply_template` files.
  Severity: medium. Likelihood: low. Mitigation: treat only renderer input data
  as in scope; leave default template publication unchanged.
- Risk: missing-field defaults are covered in unit tests but not through the
  `ReviewComment` adapter path. Severity: medium. Likelihood: medium.
  Mitigation: add at least one adapter-focused regression case that starts from
  a sparse `ReviewComment` and proves the DTO mapping preserves current
  defaults.

## Progress

- [x] (2026-03-22 00:00Z) Read `docs/roadmap.md`,
      `docs/adr-005-cross-surface-library-first-delivery.md`, the completed
      3.2.3 ExecPlan, the current `src/reply_template/` implementation, the
      TUI reply-draft adapter, and the referenced testing guidance.
- [x] (2026-03-22 00:00Z) Drafted this ExecPlan for roadmap step 3.2.4.
- [x] Stage A: add red-phase tests that define the DTO contract, the
      `ReviewComment` to DTO mapping expectations, and TUI adapter-regression
      coverage.
- [x] Stage B: introduce the public DTO and convert the renderer to consume it.
- [x] Stage C: rewire TUI and other in-repo call sites to construct the DTO
      before rendering, without changing output or error text.
- [x] Stage D: update design documentation, update the user guide if behaviour
      changed, and mark roadmap step `3.2.4` done only after all gates pass.
- [x] Stage E: run `make fmt`,
      `MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint`,
      `make nixie`, `make check-fmt`, `make lint`, and `make test`.

## Surprises & Discoveries

- `src/reply_template/mod.rs` already lives in the right top-level library
  location from step 3.2.3, but `render_reply_template` still constructs its
  MiniJinja context directly from `&ReviewComment`.
- The current render contract is small and stable: only five template
  variables are exposed, and all non-`comment_id` values are already rendered
  as strings.
- `src/tui/app/reply_draft_handlers.rs` is already a thin adapter over the
  shared renderer. The DTO migration should change only the pre-render mapping
  seam there, not the broader MVU flow.
- Existing tests already cover direct public API access
  (`tests/reply_template_public_api.rs`) and adapter-level behaviour
  (`tests/template_reply_drafting_bdd.rs`), which means the missing evidence is
  specifically the DTO contract and mapping, not a whole new test harness.
- `docs/users-guide.md` describes keyboard-driven reply drafting and template
  insertion, but not the library API. That means this step should update the
  user guide only if observable TUI behaviour or wording changes.
- The existing BDD scenarios already covered the required happy, unhappy, and
  edge adapter paths, so no new Gherkin steps were needed for this DTO-only
  migration.

## Decision Log

- Decision (2026-03-22): introduce a dedicated public DTO named
  `ReplyTemplateContext`. Rationale: the roadmap explicitly asks for a library
  DTO, and a named type gives external callers a stable contract that is
  independent of GitHub review-comment transport details.
- Decision (2026-03-22): keep `ReplyTemplateContext` in `src/reply_template/`
  rather than under `src/github/`. Rationale: the DTO belongs to the rendering
  contract, not to GitHub intake models, and ADR-005 wants the shared library
  behaviour to own its own surface.
- Decision (2026-03-22): normalize optional `ReviewComment` fields while
  building the DTO, not inside the renderer. Rationale: after step 3.2.4 the
  renderer should consume a ready-to-render library contract, while adapters
  own translation from transport models into that contract.
- Decision (2026-03-22): keep `comment_id` numeric, but store `reviewer`,
  `file`, `line`, and `body` in the DTO exactly as the renderer needs them.
  Rationale: this preserves existing template behaviour and avoids duplicating
  fallback formatting logic inside the renderer.
- Decision (2026-03-22): provide an explicit public mapping from
  `&ReviewComment` to `ReplyTemplateContext`, preferably via
  `impl From<&ReviewComment> for ReplyTemplateContext`. Rationale: the roadmap
  acceptance calls for adapter tests that cover this mapping, and a public,
  testable conversion makes that adapter contract unambiguous.
- Decision (2026-03-22): do not add a new CLI mode for this step. Rationale:
  the renderer DTO migration is a library-contract refinement with no distinct
  non-interactive workflow of its own; the design doc should record that this
  narrow item has no standalone CLI surface beyond the existing adapters.

## Outcomes & Retrospective

- `ReplyTemplateContext` now lives in `src/reply_template/mod.rs` and is
  re-exported at the crate root as `frankie::ReplyTemplateContext`.
- `render_reply_template` now renders from `&ReplyTemplateContext`, and
  `impl From<&ReviewComment> for ReplyTemplateContext` keeps the
  `ReviewComment` adapter path explicit and testable.
- TUI reply drafting still owns the transport-model adaptation and preserves
  the same inline error prefix, so `docs/users-guide.md` did not need an update
  for this internal contract change.
- Targeted red/green checks passed during implementation:
  - `cargo test --lib reply_template_context_from_review_comment_normalizes_fields`
  - `cargo test --test reply_template_public_api`
  - `cargo test --test template_reply_drafting_bdd`
- Final validation completed successfully:
  - `make fmt`
  - `MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint`
  - `make nixie`
  - `make check-fmt`
  - `make lint`
  - `make test` (`816` passed, `1` skipped)

## Context and orientation

The current reply-template feature is already split into a library renderer and
TUI adapter, but the library boundary still leaks the GitHub transport model:

1. `src/reply_template/mod.rs` owns `ReplyTemplateError` and
   `render_reply_template`, but the render function takes `&ReviewComment`.
2. `src/tui/app/reply_draft_handlers.rs` calls that renderer directly when the
   user presses a reply-template slot key.
3. `tests/reply_template_public_api.rs` proves the current public API is
   reachable from `frankie`.
4. `tests/template_reply_drafting_bdd.rs` and
   `tests/features/template_reply_drafting.feature` prove the TUI adapter still
   renders inline reply templates and surfaces inline errors.
5. `docs/frankie-design.md` currently documents ADR-005 by saying the shared
   renderer lives under `frankie::reply_template`, but it does not yet say the
   renderer contract is DTO-based.

This step should tighten that contract rather than inventing a new subsystem.
The main seam is to create a render-focused DTO, switch the renderer to that
DTO, then adapt existing call sites and tests to prove behaviour has not
changed.

## Implementation plan

### Stage A: define the DTO contract in tests first

Start by updating tests before changing implementation so the desired contract
is executable and reviewable.

1. Extend `src/reply_template/tests.rs` with `rstest` coverage that renders
   from a `ReplyTemplateContext` directly.
   - Cover the happy path for all current variables.
   - Cover missing-field defaults through DTO construction from a sparse
     `ReviewComment`.
   - Keep the existing unhappy-path coverage for invalid syntax and render-time
     failure, but move it to the DTO-based API.
   - Keep the edge coverage for escaped braces and non-recursive rendering of
     template-like body text.
2. Extend `tests/reply_template_public_api.rs` so an external integration test
   imports the public DTO from `frankie` and proves the renderer works from the
   new API surface.
3. Add mapping-focused regression coverage that starts with `ReviewComment`,
   converts it into the DTO, and proves the normalized DTO fields or rendered
   output match current expectations.
   - Use `rstest` parameterization for at least one fully populated comment and
     one sparse comment.
4. Extend `tests/template_reply_drafting_bdd.rs` and
   `tests/features/template_reply_drafting.feature` only where needed to prove
   the TUI adapter still maps `ReviewComment` into the DTO with unchanged
   behaviour.
   - Keep a happy path.
   - Keep an unhappy path that surfaces inline template errors.
   - Add or retain an edge case that proves missing fields and/or template-like
     literal body text still render identically.

The red phase is complete when these tests clearly express the target DTO
contract and at least one of them fails against the pre-migration code because
`render_reply_template` still requires `&ReviewComment`.

### Stage B: introduce the DTO and migrate the renderer

Once the tests express the target contract, change the shared library API.

1. Add `ReplyTemplateContext` under `src/reply_template/`.
   - Prefer a small dedicated file such as `src/reply_template/context.rs` if
     that keeps `mod.rs` focused and under the file-size limit.
   - Add a `//!` module comment if a new module file is created.
   - Document the DTO fields with Rustdoc, including a compact example showing
     direct library usage.
2. Provide explicit conversion from `&ReviewComment` to
   `ReplyTemplateContext`.
   - Keep the existing defaulting rules in this conversion path.
   - Avoid fallible conversion unless a real failure mode exists; today there
     is no adapter failure because missing fields default cleanly.
3. Change `render_reply_template` to accept `&ReplyTemplateContext`.
   - Keep the function name so the primary renderer API stays easy to discover.
   - Update Rustdoc examples and doctests to use the DTO-based signature.
   - Keep the `MiniJinja` environment stateless per call, as it is today.
4. Re-export the DTO from `src/lib.rs` if that keeps parity with the existing
   crate-root renderer and error re-exports. If crate-root re-export is not
   added, record the rationale in the `Decision Log` and ensure the public
   module-path tests cover discoverability.

Stage B is complete when the library renderer no longer needs
`crate::github::models::ReviewComment` as its input type.

### Stage C: adapt in-repo consumers without changing behaviour

After the shared API changes, update the in-repo adapters.

1. Update `src/tui/app/reply_draft_handlers.rs` so
   `insert_reply_template` converts the selected `ReviewComment` into
   `ReplyTemplateContext` before calling the renderer.
2. Update any in-repo tests, fixtures, or doctests that still call
   `render_reply_template` with `&ReviewComment`.
3. Keep the TUI-visible error mapping unchanged:
   - `ReplyTemplateError::InvalidSyntax` and
     `ReplyTemplateError::RenderFailed` must still surface through the same
     `Reply template rendering failed: {message}` prefix.
4. Do not expand scope into default-template publication or broader adapter
   cleanup; this step ends once the DTO migration is complete and proven.

Stage C is complete when every in-repo caller either constructs the DTO
directly or explicitly converts from `ReviewComment`, and all observable output
remains unchanged.

### Stage D: update documentation and roadmap state

Finish by aligning repository documentation with the new contract.

1. Update `docs/frankie-design.md` in the ADR-005 section so it states that
   reply templating now renders from `ReplyTemplateContext`, with TUI reply
   drafting acting as the `ReviewComment` adapter.
2. Record the no-new-CLI rationale for this roadmap step in the design notes,
   because ADR-005 expects either a CLI surface or an explicit rationale when
   CLI is not applicable.
3. Review `docs/users-guide.md`.
   - If no user-visible reply-drafting behaviour, wording, or configuration has
     changed, leave the file untouched and record that rationale in the final
     implementation notes.
   - If any user-visible wording or behaviour changed, update the inline
     reply-drafting section accordingly.
4. Mark roadmap step `3.2.4` as done in `docs/roadmap.md` only after the code,
   tests, and documentation all pass the validation commands below.

### Stage E: validate end-to-end

Run targeted tests during the red/green cycle, then run the full project gates.
Because command output is long in this environment, use `tee` and
`set -o pipefail` so logs are preserved and failures are not hidden.

Suggested targeted commands:

```bash
set -o pipefail; cargo test --lib reply_template 2>&1 | tee /tmp/3-2-4-reply-template-lib.log
set -o pipefail; cargo test --test reply_template_public_api 2>&1 | tee /tmp/3-2-4-reply-template-public-api.log
set -o pipefail; cargo test --test template_reply_drafting_bdd 2>&1 | tee /tmp/3-2-4-template-reply-bdd.log
```

Required repository gates:

```bash
set -o pipefail; make fmt 2>&1 | tee /tmp/3-2-4-make-fmt.log
set -o pipefail; MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint 2>&1 | tee /tmp/3-2-4-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/3-2-4-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/3-2-4-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/3-2-4-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/3-2-4-test.log
```

Evidence to capture in the implementation turn:

- the public DTO definition and its Rustdoc example;
- the integration test importing the DTO from `frankie`;
- the adapter regression test proving `ReviewComment` still produces the same
  rendered output through TUI;
- the updated ADR-005 design note; and
- the roadmap entry showing `3.2.4` marked done only after all gates pass.
