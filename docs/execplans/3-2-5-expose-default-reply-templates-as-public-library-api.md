# Expose default reply templates as a public library API

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT (2026-04-16)

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Roadmap step 3.2.5 exists to make Frankie's built-in reply-template defaults
usable by library consumers without reaching into crate-private config code.
After this change, an embedding host should be able to discover the same
default template strings that Frankie itself uses for reply drafting, while the
configuration and TUI fallback paths continue to derive their defaults from
that same canonical source.

Success is observable when:

- `frankie::reply_template` exposes a stable public default-template API, and
  the crate root re-exports it for discoverability;
- the public defaults are deterministic, non-empty, and documented with
  Rustdoc;
- `FrankieConfig::default().reply_templates` and TUI fallback reply-draft
  config derive their templates from the public defaults rather than from a
  separate private list;
- `rstest` unit and integration tests assert the exact default strings and
  prove the configured defaults remain present in Frankie config and TUI
  startup defaults;
- `rstest-bdd` v0.5.0 scenarios prove the user-visible reply-drafting flow
  still works when Frankie relies on its built-in defaults;
- `docs/frankie-design.md` records the canonical-source decision and the
  no-new-CLI rationale for this narrow API-publication step;
- `docs/users-guide.md` is updated if the implementation changes any
  user-visible default-template behaviour or if the guide would otherwise imply
  the wrong built-in defaults; and
- `docs/roadmap.md` marks `3.2.5` done only after all documentation and gates
  pass.

## Relevant documentation and skills

The implementer should keep these documents open while working:

- `docs/roadmap.md` for the exact scope and acceptance language of item
  `3.2.5`.
- `docs/adr-004-inline-template-based-reply-drafting.md` for the original
  reply-drafting behaviour, template-slot UX, and configuration contract.
- `docs/adr-005-cross-surface-library-first-delivery.md` for the requirement
  that shared reply-template behaviour lives in the library surface, with TUI
  and CLI layers acting as adapters.
- `docs/frankie-design.md` for the design-doc ADR index and the place where
  this step's public-default decision must be recorded.
- `docs/execplans/3-2-3-extract-reply-templating-into-library-module.md` and
  `docs/execplans/3-2-4-reply-templating-data-transfer-object.md` for the two
  completed prerequisite slices that established the shared module and DTO.
- `docs/rust-testing-with-rstest-fixtures.md` and
  `docs/rstest-bdd-users-guide.md` for the required `rstest` and `rstest-bdd`
  testing style.
- `docs/rust-doctest-dry-guide.md` because the new public API should carry a
  compilable Rustdoc example.
- `docs/reliable-testing-in-rust-via-dependency-injection.md` because the
  behavioural coverage should prefer existing injected TUI configuration seams
  over ad hoc global mutations.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` to keep the
  change focused on a single canonical default source instead of spreading
  template-copy logic across modules.
- `docs/ortho-config-users-guide.md` for config-layer expectations if any
  loading or defaulting nuance needs clarification.
- `docs/snapshot-testing-bubbletea-terminal-uis-with-insta.md`,
  `docs/building-idiomatic-terminal-uis-with-bubbletea-rs.md`, and
  `docs/two-tier-testing-strategy-for-an-octocrab-github-client.md` are
  background references only. This slice should not need snapshot work or
  GitHub client changes unless a regression forces that scope wider.

The most relevant skills for implementation are:

- `execplans` to keep this document current during implementation.
- `leta` to inspect exact symbols such as `default_reply_templates`,
  `ReplyDraftConfig::default`, and public re-exports without guessing.
- `rust-router` and `rust-types-and-apis` for the public API shape and
  re-export decisions.
- `rust-unused-code` if the old private helper becomes dead code after the
  canonical-source move.
- `en-gb-oxendict-style` for documentation updates.

## Constraints

- Follow `docs/adr-004-inline-template-based-reply-drafting.md` and
  `docs/adr-005-cross-surface-library-first-delivery.md`: the canonical
  built-in reply-template defaults must live in a shared library module, not in
  TUI-only or config-only code.
- Treat roadmap steps 3.2.3 and 3.2.4 as completed prerequisites. Do not
  re-open the renderer extraction or DTO migration except where tiny follow-on
  edits are required to route defaults through the shared module.
- Keep this slice scoped to public default-template exposure. Do not fold in
  roadmap step 3.2.6's broader adapter cleanup or step 3.2.7's reply action API.
- Preserve the current built-in default template copy unless a deliberate
  design decision is recorded in `docs/frankie-design.md`. This roadmap step is
  about publication and determinism, not about rewriting the template text.
- Keep configuration override semantics unchanged. Explicit
  `reply_templates` values from CLI, environment, or config files must still
  replace the defaults as they do today.
- Avoid duplicate canonical sources. After implementation, there must be one
  authoritative built-in default list, and config/TUI defaults must derive from
  it.
- Preserve current user-visible reply-drafting behaviour and current inline
  error messages.
- Keep the public API host-safe and library-first:
  - the canonical defaults must be accessible from `frankie::reply_template`;
  - the crate root should re-export the same public API for parity with
    `ReplyTemplateContext` and `render_reply_template`;
  - no TUI-specific types may leak into the default-template surface.
- Prefer an immutable borrowed public representation, such as
  `pub const DEFAULT_REPLY_TEMPLATES: &[&str]`, so the public contract is
  deterministic and semver-stable even if the number of templates changes.
- Any new or changed public Rust item must have Rustdoc comments with a clear
  example.
- Use `rstest` fixtures for unit and integration tests and `rstest-bdd`
  v0.5.0 for behavioural tests.
- Shared BDD helpers that return `Result` must not use `assert!`; they must
  return explicit `Err(...)` values instead.
- Only mark roadmap step `3.2.5` done after implementation, documentation, and
  all required gates pass.

## Tolerances (exception triggers)

- Scope: if landing this cleanly requires touching more than 14 files or more
  than 700 net new lines, stop and re-scope before implementation.
- Contract: if exposing the defaults publicly appears to require a breaking
  change to `FrankieConfig`, `ReplyDraftConfig`, or template override
  semantics, stop and escalate.
- API shape: if the immutable borrowed representation proves unworkable and a
  mutable owned public API seems necessary, stop and record the trade-offs
  before proceeding.
- Dependencies: if any new runtime or development dependency seems required,
  stop and escalate.
- Behavioural testing: if a new BDD scenario cannot be added by reusing the
  existing reply-drafting harness in `tests/template_reply_drafting_bdd.rs`,
  stop after one prototype and decide whether narrower integration coverage is
  the safer choice.
- Iterations: if any milestone still fails after three fix cycles, stop and
  escalate with the failing logs.

## Risks

- Risk: a new public constant or function is added, but Frankie config and TUI
  startup continue to use a separate private list. Severity: high. Likelihood:
  medium. Mitigation: make one canonical public source and route every default
  consumer through it.
- Risk: tests only assert that defaults are non-empty, so later edits can
  silently reorder or delete built-ins. Severity: high. Likelihood: high.
  Mitigation: add exact-value tests for the public defaults and parity tests
  for config and TUI default consumers.
- Risk: the public API leaks an owned `Vec<String>` and encourages callers to
  treat mutation as part of the contract. Severity: medium. Likelihood: medium.
  Mitigation: expose an immutable borrowed representation and keep any owned
  conversion helper private or crate-private.
- Risk: behavioural coverage keeps exercising only injected custom templates
  and never proves the built-in default path. Severity: medium. Likelihood:
  high. Mitigation: add a BDD scenario that launches the TUI with default
  reply-draft config and inserts slot `1`.
- Risk: `docs/users-guide.md` continues to show example templates that readers
  could misread as the exact built-ins. Severity: low. Likelihood: medium.
  Mitigation: either label the example clearly as illustrative or update it to
  match the canonical defaults if the guide is meant to document the actual
  shipped defaults.
- Risk: TUI default tests interact poorly with process-global `OnceLock`
  storage. Severity: medium. Likelihood: low. Mitigation: use the existing
  storage-test guard patterns if any new test needs to exercise mutable global
  reply-draft configuration.

## Progress

- [x] (2026-04-16 00:00Z) Read `docs/roadmap.md`,
      `docs/adr-004-inline-template-based-reply-drafting.md`, the completed
      3.2.3 and 3.2.4 ExecPlans, the current reply-template/config/TUI code,
      and the referenced testing guidance.
- [x] (2026-04-16 00:00Z) Drafted this ExecPlan for roadmap step 3.2.5.
- [ ] Stage A: add red-phase tests for the public default API and for config
      and TUI parity with that API.
- [ ] Stage B: publish the canonical default-template source from
      `src/reply_template/` and re-export it from `src/lib.rs`.
- [ ] Stage C: rewire config and TUI default construction to derive from the
      public canonical defaults without changing override semantics.
- [ ] Stage D: update `docs/frankie-design.md`, update
      `docs/users-guide.md` if needed, and mark roadmap step `3.2.5` done only
      after implementation and all gates pass.
- [ ] Stage E: run `make fmt`,
      `MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint`,
      `make nixie`, `make check-fmt`, `make lint`, and `make test`.

## Surprises & Discoveries

- The only current built-in default source is
  `src/config/mod.rs:default_reply_templates`, which is crate-private and is
  called from only two places: `FrankieConfig::default()` and
  `ReplyDraftConfig::default()`.
- `src/reply_template/mod.rs` already owns the shared public reply-template
  surface for step 3.2.4, so step 3.2.5 can stay tightly scoped by adding the
  default source there instead of creating another new module.
- Current tests assert only that reply-template defaults are non-empty. No
  test currently proves the exact built-in strings or that the library, config,
  and TUI surfaces all share the same list.
- Existing behavioural coverage in
  `tests/template_reply_drafting_bdd.rs` already exercises template insertion,
  invalid slots, over-limit insertion, invalid syntax, and literal rendering.
  The missing end-to-end evidence is the built-in default path itself.
- `docs/users-guide.md` currently shows `reply_templates` in an illustrative
  config snippet, not obviously as a canonical list of shipped defaults. The
  implementation should decide whether that is acceptable or whether the guide
  needs a clarifying note.

## Decision Log

- Decision (2026-04-16): the canonical built-in default source should move to
  `src/reply_template/mod.rs`. Rationale: the reply-template shared library
  module already owns the public renderer and DTO, so it is the correct home
  for the related public default contract.
- Decision (2026-04-16): prefer an immutable borrowed public API, such as
  `DEFAULT_REPLY_TEMPLATES: &[&str]`, over a public `Vec<String>` factory.
  Rationale: the borrowed form is deterministic, non-empty, allocation-free for
  readers, and avoids baking the current template count into the public type.
- Decision (2026-04-16): keep any owned conversion helper private or
  crate-private and derive config/TUI `Vec<String>` values from the borrowed
  public defaults. Rationale: Frankie still needs owned strings internally, but
  callers do not need a mutable public contract for this roadmap item.
- Decision (2026-04-16): do not add a new standalone CLI mode for this step.
  Rationale: the work refines a shared library contract and the default
  templates are already indirectly observable through the existing TUI reply
  drafting surface and config defaults.
- Decision (2026-04-16): the behavioural test for this slice should reuse the
  existing reply-drafting BDD harness rather than inventing a new adapter test
  rig. Rationale: the current harness already models the user-visible keyboard
  flow and only needs one additional default-path scenario.

## Outcomes & Retrospective

This plan is still in draft. No implementation changes, roadmap updates, or
feature-level outcomes have been recorded yet.

When implementation is complete, update this section with:

- the final public API path and Rustdoc example shape;
- the exact tests added or changed;
- whether `docs/users-guide.md` changed and why;
- the final validation results; and
- any follow-on cleanup deferred to roadmap step `3.2.6`.

## Context and orientation

The current code path for reply-template defaults is narrow and easy to trace:

1. `src/reply_template/mod.rs` already exposes the shared public renderer and
   DTO:
   - `ReplyTemplateContext`
   - `ReplyTemplateError`
   - `render_reply_template`
2. `src/config/mod.rs` still owns a crate-private helper named
   `default_reply_templates()` that returns the current three built-in template
   strings.
3. `FrankieConfig::default()` in `src/config/mod.rs` fills
   `reply_templates: Vec<String>` by calling that private helper.
4. `ReplyDraftConfig::default()` in `src/tui/reply_draft_config.rs` also calls
   `crate::config::default_reply_templates()` to supply TUI fallback templates.
5. `tests/reply_template_public_api.rs` proves the shared renderer and DTO are
   publicly accessible today, but it does not yet exercise the built-in
   defaults.
6. `src/config/tests/reply_drafting.rs`,
   `src/config/tests/field_resolution.rs`, and `src/tui/tests.rs` prove only
   that defaults are non-empty, not that they match a public canonical source.
7. `tests/template_reply_drafting_bdd.rs` and
   `tests/features/template_reply_drafting.feature` already cover the
   user-visible reply-draft flow when a custom template is injected through
   `ReplyDraftConfig::new(...)`.

This roadmap item should tighten that contract rather than inventing a new
subsystem. The work is to publish the built-ins from the existing shared
reply-template module, derive internal defaults from that source, and prove the
three surfaces stay aligned: public library API, config defaults, and TUI
fallback defaults.

## Implementation plan

### Stage A: add red-phase tests before touching implementation

Start by extending tests so the desired public-default contract is executable.
The first targeted test run should fail because the public default API does not
exist yet and the default-path BDD scenario is not yet wired.

1. Extend `tests/reply_template_public_api.rs` so it imports the new public
   default-template API from both the crate root and `frankie::reply_template`.
   Add tests that prove:
   - the defaults are non-empty;
   - the defaults are deterministic and in the current order;
   - the exact current strings are still present.
2. Extend `src/reply_template/tests.rs` with unit tests that convert the public
   defaults into the owned representation used internally and prove the shared
   module keeps the exact built-ins expected by this roadmap step.
3. Tighten `src/config/tests/reply_drafting.rs` and
   `src/config/tests/field_resolution.rs` so
   `FrankieConfig::default().reply_templates` is asserted against the public
   defaults, not merely checked for non-emptiness.
4. Tighten `src/tui/tests.rs` so the TUI fallback config asserts that
   `get_reply_draft_config().templates` matches the public defaults. If this
   requires mutable global setup, use the existing storage guard pattern rather
   than ad hoc mutation.
5. Extend `tests/features/template_reply_drafting.feature` and
   `tests/template_reply_drafting_bdd.rs` with one happy-path scenario that
   launches a review TUI without a custom reply-template override, presses `a`
   then `1`, renders the view, and proves Frankie inserts slot `1` from the
   built-in defaults.
6. Re-run the targeted test commands and confirm the new tests fail for the
   expected reason before changing production code.

### Stage B: publish the canonical default-template source

Once the tests describe the desired contract, publish the built-ins from the
shared reply-template module.

1. Add the canonical public default source to `src/reply_template/mod.rs`.
   Prefer this shape:

   ```rust
   pub const DEFAULT_REPLY_TEMPLATES: &[&str] = &[/* current built-ins */];
   ```

   Keep the current built-in strings unchanged unless the design doc records a
   deliberate copy change.
2. Add Rustdoc comments and a short example showing how an external caller can
   inspect the defaults and render one of them with `ReplyTemplateContext`.
3. Add a small private or crate-private helper near the constant that converts
   the borrowed defaults into `Vec<String>` for internal config/TUI use. Keep
   the canonical source single-sited.
4. Update `src/lib.rs` to re-export the new public default API at the crate
   root alongside the existing reply-template exports.

### Stage C: rewire config and TUI defaults to the canonical source

After the new public API exists, remove the duplicate private ownership of the
defaults.

1. Update `src/config/mod.rs` so `FrankieConfig::default()` derives
   `reply_templates` from the canonical public defaults instead of from a
   separate private literal list.
2. Remove or collapse the old crate-private `default_reply_templates()` helper
   so there is no duplicate source of truth left behind. If a helper remains,
   it should be a thin converter over the public canonical defaults rather than
   a second literal list.
3. Update `src/tui/reply_draft_config.rs` so `ReplyDraftConfig::default()`
   derives its template list from the canonical public defaults.
4. Re-run the targeted tests from Stage A. At this point:
   - public API tests should pass;
   - config default tests should prove the configured defaults remain present;
   - TUI fallback tests should prove the built-in default path uses the same
     list; and
   - the new BDD scenario should prove the built-in slot `1` path works
     end-to-end.

### Stage D: document the decision and user-facing implications

When the code and tests pass, update the documentation while keeping the scope
strict.

1. Update `docs/frankie-design.md` near the ADR-005 reply-template section to
   state that Frankie now exposes canonical built-in reply templates as a
   deterministic public library API, and that config and TUI defaults derive
   from that source.
2. Record the no-new-CLI rationale there as well: this slice refines a shared
   library contract and does not add a distinct non-interactive workflow.
3. Review `docs/users-guide.md`:
   - if the implementation leaves user-visible behaviour unchanged and the
     guide's `reply_templates` snippet is clearly illustrative, note in the
     ExecPlan that no guide change was required;
   - if the guide would mislead users about the shipped defaults, update the
     relevant section so the guide is accurate.
4. Mark roadmap step `3.2.5` done in `docs/roadmap.md` only after all code,
   docs, and validation gates pass.

### Stage E: run full validation and capture evidence

Use Makefile targets and the repository's logging convention. Run each command
with `set -o pipefail` and `tee` so truncated terminal output does not hide the
actual failure.

Targeted red/green checks during implementation:

```plaintext
set -o pipefail; cargo test --test reply_template_public_api 2>&1 | tee /tmp/3-2-5-reply-template-public-api.log
set -o pipefail; cargo test --lib reply_template 2>&1 | tee /tmp/3-2-5-reply-template-lib.log
set -o pipefail; cargo test --test template_reply_drafting_bdd 2>&1 | tee /tmp/3-2-5-template-reply-bdd.log
```

Required repository gates before marking the roadmap item done:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/3-2-5-fmt.log
set -o pipefail; MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint 2>&1 | tee /tmp/3-2-5-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/3-2-5-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/3-2-5-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/3-2-5-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/3-2-5-test.log
```

Success criteria for the final run:

1. `tests/reply_template_public_api.rs` proves the default-template API is
   reachable from both public paths and exposes the expected built-ins.
2. Unit tests prove the canonical public defaults are non-empty, deterministic,
   and unchanged.
3. Config and TUI tests prove Frankie still carries those defaults into runtime
   configuration when no override is supplied.
4. Behavioural tests prove built-in slot insertion still works through the TUI
   adapter.
5. Markdown and Rust quality gates all pass.
