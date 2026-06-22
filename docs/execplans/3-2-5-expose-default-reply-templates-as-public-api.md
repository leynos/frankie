# Expose default reply templates as a public library API (3.2.5)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

## Purpose / big picture

Frankie ships a small set of built-in reply templates that seed the interactive
reply-drafting experience (press `a`, then a digit `1`-`9` to insert a starter
reply). Today those defaults are crate-private: the function
`default_reply_templates()` lives in `src/config/mod.rs` and is only reachable
from inside the `frankie` crate.

Roadmap item 3.2.5 requires these defaults to become part of the **public
library API** so that embedding hosts and alternative front ends (the
"library-first" delivery contract from ADR-005) can read the same canonical
defaults Frankie uses, without copying string literals or depending on the TUI.

After this change, a library consumer can write:

```rust
use frankie::{DEFAULT_REPLY_TEMPLATES, default_reply_templates};

// Borrow the canonical defaults with zero allocation.
assert!(!DEFAULT_REPLY_TEMPLATES.is_empty());

// Or take an owned copy ready to drop into configuration.
let owned: Vec<String> = default_reply_templates();
assert_eq!(owned.len(), DEFAULT_REPLY_TEMPLATES.len());
```

Success is observable when:

1. `frankie::DEFAULT_REPLY_TEMPLATES` and `frankie::default_reply_templates()`
   compile and are reachable from an integration test in `tests/`.
2. The defaults are non-empty and identical on every call (deterministic).
3. The defaults that ship today remain present, in their established order, and
   a test fails if any is removed or reordered.
4. `FrankieConfig::default().reply_templates` equals
   `frankie::default_reply_templates()`, proving the configuration default and
   the public API share one source of truth.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

- The three default template strings that ship today must remain present and in
  their current order. This is the roadmap acceptance criterion ("configured
  defaults remain present") and the order is load-bearing: the TUI binds the
  list positionally to keyboard slots `1`-`9`.
- The rendered output and user-facing error messages of reply drafting must not
  change. Existing behavioural tests in `tests/template_reply_drafting_bdd.rs`
  and unit tests in `src/reply_template/tests.rs` must continue to pass
  unchanged.
- No new external dependency may be added.
- `default_reply_templates()` must continue to return `Vec<String>` (the type
  of the `FrankieConfig::reply_templates` field) so both existing call sites
  keep compiling without signature churn.
- The reply-template module must remain free of I/O and framework imports. It
  may keep its existing `minijinja` and `crate::github::models::ReviewComment`
  imports, but this change must not add infrastructure dependencies to it.
- Out of scope (do not change in this slice): the residual lateral dependency
  where `src/tui/reply_draft_config.rs` imports `DEFAULT_REPLY_MAX_LENGTH` from
  `crate::config`. This plan relocates only the default *templates*; the
  max-length constant relocation, if desired, belongs to a separate step. Note
  it in `Surprises & Discoveries` but do not act on it.
- No live GitHub submission, no AI rewording, and no new CLI subcommand are in
  scope. This is a narrow library-surface change.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached.

- Scope: if implementation requires changes to more than 12 files or more than
  250 net lines of code (excluding the ExecPlan itself), stop and escalate.
- Interface: if any existing public API signature must change (as opposed to
  new additive items), stop and escalate. This change is purely additive plus
  an internal relocation.
- Dependencies: if a new external crate is required, stop and escalate.
- Iterations: if `make test` still fails after 3 focused attempts on the same
  milestone, stop and escalate.
- Ambiguity: if the "configured defaults remain present" criterion appears to
  conflict with a requested edit to the default strings, stop and present the
  trade-off (assertion strictness vs. ability to revise defaults).

## Risks

- Risk: a future maintainer adds a fourth default and a downstream consumer had
  hard-coded `DEFAULT_REPLY_TEMPLATES.len() == 3` or indexed by position.
  Severity: low. Likelihood: medium. Mitigation: document the stability
  contract on the constant — ordering is stable, entries may be **appended** in
  future minor versions, and positional indices are not stable identity. Assert
  exact ordered contents in tests so an intentional change is a deliberate,
  reviewed edit rather than a silent one.

- Risk: the crate has no public-API guard (`cargo-public-api` /
  `cargo-semver-checks` are absent), so future accidental breakage of this new
  surface would not be caught mechanically. Severity: low. Likelihood: low.
  Mitigation: out of scope to introduce such tooling here (new dependency +
  CI), but record it as a recommended follow-up. The integration parity test
  guards the one contract that matters operationally (config default == public
  defaults).

- Risk: relocating the function changes which module "owns" the defaults and
  could leave a stale re-export or a dangling `crate::config` reference.
  Severity: medium. Likelihood: low. Mitigation: delete the crate-private
  definition outright (no stub), update both call sites, and rely on
  `cargo clippy -D warnings` plus the compiler to surface any missed reference.
  `leta refs default_reply_templates` confirms there are exactly two call sites
  today.

## Progress

- [x] (2026-06-22 15:54 Europe/Berlin) Stage A: confirmed orientation and the
      two call sites with `leta refs default_reply_templates`.
- [ ] (pending) Stage B: red tests (unit + integration) that reference the not-
      yet-public API and fail to compile/assert.
- [ ] (pending) Stage C: implement `src/reply_template/defaults.rs`, re-export,
      refactor consumers, delete crate-private copy.
- [ ] (pending) Stage D: documentation, ADR-004 update, design-doc note,
      gates, CodeRabbit, roadmap tick.

## Surprises & discoveries

- 2026-06-22: The local branch was already named
  `3-2-5-expose-default-reply-templates-as-public-api`, but it was not tracking
  a remote branch. The remote branch
  `origin/3-2-5-expose-default-reply-templates-as-public-api` exists, so the
  local branch was configured to track it before implementation.
- 2026-06-22: `leta refs default_reply_templates` matched the plan exactly:
  the crate-private definition in `src/config/mod.rs`, the
  `FrankieConfig::default` call, and the `ReplyDraftConfig::default` call.

## Decision log

- Decision: Treat the repository owner's 2026-06-22 request to "proceed with
  implementation" as the approval gate for this previously drafted ExecPlan.
  Rationale: the ExecPlan skill requires explicit approval before execution;
  the request names the plan file and directs implementation, so the plan is no
  longer in draft. Date/Author: 2026-06-22, implementation agent.

- Decision: Relocate the canonical defaults into the `reply_template` domain
  module rather than simply making the existing `config` function `pub`.
  Rationale: the defaults are domain policy (they contain the reply-template
  grammar `{{ file }}`, `{{ reviewer }}`, … and are validated by the module's
  own `render_reply_template`). `reply_template` already owns the public
  reply-template contract (ADR-005) and is re-exported at the crate root.
  `config`'s job is layered resolution (CLI/env/file precedence), not
  authorship of fallback content. The relocation also removes a real lateral
  adapter-to-adapter dependency: `src/tui/reply_draft_config.rs` currently
  reaches into `crate::config` for the defaults; afterwards both `config` and
  `tui` depend *inward* on the domain module. Date/Author: 2026-06-18, planning
  panel (Pandalump, Wafflecat).

- Decision: Expose both `pub const DEFAULT_REPLY_TEMPLATES: &[&str]` (canonical
  source of truth) and `pub fn default_reply_templates() -> Vec<String>`, with
  the function deriving from the constant so the two forms cannot drift.
  Rationale: the constant gives compile-time determinism and non-emptiness for
  free and a zero-allocation borrow path; the function preserves both existing
  `Vec<String>` call sites without churn. Deriving one from the other keeps a
  single source of truth and makes the near-identical names truthful ("same
  data, different ownership"). Date/Author: 2026-06-18, planning panel
  (Telefono, Pandalump, Wafflecat).

- Decision: Place the defaults in a dedicated submodule
  `src/reply_template/defaults.rs`, not inline in `mod.rs`. Rationale: `mod.rs`
  already holds three concerns (error type, context type, renderer) at ~173
  lines against a 400-line repository file cap; the repo already uses sibling
  files (`tests.rs`, `test_support.rs`, `config/summarize_mode.rs`). A named
  submodule is self-documenting and gives the unit tests an obvious home.
  Date/Author: 2026-06-18, planning panel (Pandalump, Dinolump).

- Decision: Keep the test surface proportionate — rstest unit tests plus one
  extension of the existing integration test. Do **not** add an insta snapshot,
  a proptest, or a new rstest-bdd feature for this change. Rationale: the
  artefact is three constant strings. The acceptance criteria (public,
  deterministic, non-empty, defaults remain present) are fully encoded by
  explicit ordered assertions, which are strictly more precise than a 3-element
  snapshot and impose less maintenance friction. The only genuine invariant
  ("each default is valid MiniJinja using only supported variables") is proven
  by rendering each default once against a fixed context; the templates are
  fixed strings, so a proptest over arbitrary contexts would exercise the
  renderer (already covered in `src/reply_template/tests.rs`) rather than the
  defaults, adding noise without coverage. The user-observable drafting
  workflow that consumes these defaults already has BDD coverage in
  `tests/template_reply_drafting_bdd.rs`; a Gherkin feature for "read a public
  constant" would be disproportionate. This applies the user's "where
  applicable / use your best judgement" guidance for snapshot, property, and
  BDD tiers. If the reviewer prefers maximal-rigour coverage, this is the point
  to say so at the approval gate. Date/Author: 2026-06-18, planning panel
  (Dinolump), endorsed by the panel. **Confirmed by the repository owner on
  2026-06-18**: the proportionate test surface (rstest unit tests plus one
  integration parity assertion; no insta/proptest/rstest-bdd) is accepted.

- Decision: Update ADR-004's consequences and the design doc rather than
  writing a new ADR. Rationale: "built-in defaults are part of the public API"
  is a *consequence* of ADR-004 (templates exist and are configurable) and
  ADR-005 (library-first delivery), not a new hard-to-reverse architectural
  choice. A standalone ADR per visibility bump would devalue the record.
  Date/Author: 2026-06-18, planning panel (Dinolump).

## Outcomes & retrospective

- (to be completed at delivery)

## Context and orientation

The `frankie` crate is both a library (`src/lib.rs`) and a binary
(`src/main.rs`). Treat the reader as new to the repository.

Key files for this change:

- `src/config/mod.rs` — application configuration loaded via `ortho-config`
  from CLI, environment, and file layers. Defines `FrankieConfig`, whose field
  `reply_templates: Vec<String>` holds the reply templates. The crate-private
  function `default_reply_templates()` (around line 389) returns the three
  built-in defaults and is called by `FrankieConfig::default` (around line
  376). The constant `DEFAULT_REPLY_MAX_LENGTH` also lives here and is out of
  scope.

- `src/reply_template/mod.rs` — the shared reply-template **domain module**
  introduced by roadmap items 3.2.3 and 3.2.4. It owns `ReplyTemplateContext`
  (the render-input data transfer object), `render_reply_template`, and
  `ReplyTemplateError`, and is re-exported at the crate root. It declares
  `#[cfg(test)] mod test_support;` and `#[cfg(test)] mod tests;`, whose bodies
  live in sibling files `src/reply_template/test_support.rs` and
  `src/reply_template/tests.rs`.

- `src/tui/reply_draft_config.rs` — a TUI adapter. Its `ReplyDraftConfig`
  `Default` impl currently seeds `templates` from
  `crate::config::default_reply_templates()` (line 52).

- `src/lib.rs` — declares the public modules and the crate-root `pub use`
  re-exports. Line 33 re-exports the reply-template contract, namely
  `ReplyTemplateContext`, `ReplyTemplateError`, and `render_reply_template`.

- `tests/reply_template_public_api.rs` — integration tests proving the reply-
  template API is reachable from outside the crate. It uses shared fixtures from
  `tests/support/reply_template.rs` (helpers `sample_review_comment` and
  `review_comment_with_body`).

- `src/config/tests/reply_drafting.rs` and
  `src/config/tests/field_resolution.rs`
  — existing config tests asserting the default `reply_templates` list is
  non-empty. These must continue to pass.

Terms used:

- **Domain policy**: business rules and canonical values that exist
  independently of how data is loaded or displayed. The default templates are
  domain policy.
- **Adapter**: code that connects the domain to the outside world (the TUI, the
  config loader). Adapters depend inward on the domain, never laterally on each
  other.
- **DTO (data transfer object)**: a plain data struct used to carry values
  across a boundary. `ReplyTemplateContext` is one.

The current defaults (preserve verbatim and in this order):

```rust
"Thanks for the review on {{ file }}:{{ line }}. I will update this."
"Good catch, {{ reviewer }}. I will address this in the next commit."
"I have addressed this feedback and pushed an update."
```

## Plan of work

The work follows Red-Green-Refactor. Stages end with validation; do not proceed
past a failing stage.

### Stage A: understand and propose (no code changes)

Confirm the two and only two consumers of the crate-private function with
`leta refs default_reply_templates`. Expected: a definition in
`src/config/mod.rs`, the call in `FrankieConfig::default`
(`src/config/mod.rs`), and the call in `src/tui/reply_draft_config.rs`. If a
third consumer appears, update `Risks` and re-scope before continuing.

### Stage B: red tests

Add the new tests first so they fail for the expected reason (the public items
do not yet exist, so the crate fails to compile, which is the red signal).

1. Create `src/reply_template/defaults_tests.rs` with rstest unit tests
   (described under "Concrete steps"). Declare it from the defaults module with
   `#[cfg(test)] mod defaults_tests;` once that module exists; in this red step
   the reference will not compile, which is expected.

2. Extend `tests/reply_template_public_api.rs` with two integration tests:
   - `crate_root_exposes_default_reply_templates` — asserts
     `frankie::DEFAULT_REPLY_TEMPLATES` is non-empty, that
     `frankie::default_reply_templates()` equals the constant mapped to owned
     strings, and that the three canonical strings are present in order.
   - `config_default_matches_public_default_reply_templates` — asserts
     `frankie::FrankieConfig::default().reply_templates ==
     frankie::default_reply_templates()`.

Run the focused build/test and observe failure (red):

```bash
cargo test -p frankie --test reply_template_public_api 2>&1 | tee \
  /tmp/redtest-frankie-$(git branch --show-current | tr '/' '-').out
```

Expected: compilation error referencing `DEFAULT_REPLY_TEMPLATES` /
`default_reply_templates` not found in `frankie`. This proves the tests
exercise the new surface.

### Stage C: implementation (minimal change to satisfy tests)

1. Create `src/reply_template/defaults.rs` (module-level `//!` doc required).
   Define the canonical constant and the derived function. Document the
   stability contract on the constant (order stable and load-bearing for TUI
   slots `1`-`9`; entries may be appended in future minor versions; positional
   indices are not stable identity). Add a rustdoc example (doctest) on the
   function showing both forms; keep it minimal per the doctest DRY guide.

2. In `src/reply_template/mod.rs`, declare `pub mod defaults;` and re-export the
   two items:
   `pub use defaults::{DEFAULT_REPLY_TEMPLATES, default_reply_templates};`.
   Wire the unit tests by adding `#[cfg(test)] mod defaults_tests;` to
   `defaults.rs`.

3. In `src/lib.rs`, extend the crate-root re-export on line 33 to add
   `DEFAULT_REPLY_TEMPLATES` and `default_reply_templates` alongside the
   existing reply-template items. The final form is shown verbatim under
   *Artifacts and notes* below.

4. In `src/config/mod.rs`, delete the crate-private `default_reply_templates()`
   function and change `FrankieConfig::default` to call
   `crate::reply_template::default_reply_templates()`.

5. In `src/tui/reply_draft_config.rs`, change the `Default` impl to call
   `crate::reply_template::default_reply_templates()` instead of
   `crate::config::default_reply_templates()`.

Run the focused tests and observe green:

```bash
cargo test -p frankie --test reply_template_public_api 2>&1 | tee \
  /tmp/greentest-frankie-$(git branch --show-current | tr '/' '-').out
cargo test -p frankie reply_template 2>&1 | tee -a \
  /tmp/greentest-frankie-$(git branch --show-current | tr '/' '-').out
```

Expected: the new integration tests pass; existing reply-template and config
default tests still pass.

### Stage D: refactor, documentation, and cleanup

1. `docs/users-guide.md`: in the existing reply-templates configuration area
   (around the `reply_templates` config and CLI/env entries), add a short note
   that Frankie ships built-in defaults and that they are available
   programmatically via `frankie::default_reply_templates()` /
   `frankie::DEFAULT_REPLY_TEMPLATES`. Keep it to a few lines; do not add a
   large standalone Library API section for three strings.

2. `docs/developers-guide.md`: add a short subsection recording the internal
   convention: the canonical defaults live in `frankie::reply_template`
   (`DEFAULT_REPLY_TEMPLATES` is the source of truth, the function derives from
   it), and `config`/`tui` consume the public API rather than defining their
   own copies.

3. `docs/adr-004-inline-template-based-reply-drafting.md`: add a "Public
   defaults" line to the Consequences section noting that the built-in defaults
   are now part of the public library API, owned by `frankie::reply_template`,
   and reference the relevant test files.

4. `docs/frankie-design.md`: in the ADR-005 reply-template paragraph (the one
   describing the shared `frankie::reply_template` module), add a sentence that
   the built-in defaults are now exposed publicly from that module.

5. Mark roadmap item 3.2.5 as done in `docs/roadmap.md` (`- [ ]` to `- [x]`)
   only after all gates pass.

## Concrete steps

Work from the repository root. The exact unit tests to add to
`src/reply_template/defaults_tests.rs`:

```rust
//! Unit tests for the built-in default reply templates.

use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{DEFAULT_REPLY_TEMPLATES, default_reply_templates};
use crate::reply_template::{ReplyTemplateContext, render_reply_template};

#[rstest]
fn default_reply_templates_constant_is_non_empty() {
    assert!(
        !DEFAULT_REPLY_TEMPLATES.is_empty(),
        "the built-in default reply templates must not be empty"
    );
}

#[rstest]
fn default_reply_templates_function_derives_from_constant() {
    let owned = default_reply_templates();
    let expected: Vec<String> = DEFAULT_REPLY_TEMPLATES
        .iter()
        .map(|template| (*template).to_owned())
        .collect();
    assert_eq!(owned, expected);
}

#[rstest]
fn default_reply_templates_are_deterministic() {
    assert_eq!(default_reply_templates(), default_reply_templates());
}

#[rstest]
fn default_reply_templates_preserve_configured_defaults_in_order() {
    assert_eq!(
        DEFAULT_REPLY_TEMPLATES,
        [
            "Thanks for the review on {{ file }}:{{ line }}. I will update this.",
            "Good catch, {{ reviewer }}. I will address this in the next commit.",
            "I have addressed this feedback and pushed an update.",
        ]
    );
}

#[rstest]
fn each_default_template_renders_against_a_representative_context() {
    let context = ReplyTemplateContext {
        comment_id: 7,
        reviewer: "alice".to_owned(),
        file: "src/lib.rs".to_owned(),
        line: "12".to_owned(),
        body: "Please tidy this up.".to_owned(),
    };

    for template in DEFAULT_REPLY_TEMPLATES {
        let rendered = render_reply_template(template, &context);
        expect_that!(rendered, ok(anything()));
    }
}
```

The defaults module body (`src/reply_template/defaults.rs`):

```rust
//! Built-in default reply templates shared across Frankie surfaces.
//!
//! These defaults seed reply drafting when no templates are configured. They
//! are domain policy: the canonical content lives here, and configuration and
//! TUI adapters consume this module rather than defining their own copies.

/// The built-in default reply templates, in their stable presentation order.
///
/// This constant is the single source of truth for Frankie's default reply
/// templates. The ordering is part of the public contract: the interactive
/// TUI binds entries positionally to keyboard slots `1`-`9`. Future minor
/// versions may **append** entries; positional indices are not a stable
/// identity, so do not rely on a fixed length or index for a specific
/// template.
pub const DEFAULT_REPLY_TEMPLATES: &[&str] = &[
    "Thanks for the review on {{ file }}:{{ line }}. I will update this.",
    "Good catch, {{ reviewer }}. I will address this in the next commit.",
    "I have addressed this feedback and pushed an update.",
];

/// Returns an owned copy of [`DEFAULT_REPLY_TEMPLATES`].
///
/// This is the convenient form for seeding owned configuration such as
/// [`crate::config::FrankieConfig`]'s `reply_templates` field. It always
/// derives from [`DEFAULT_REPLY_TEMPLATES`], so the two forms cannot drift.
///
/// # Examples
///
/// ```
/// use frankie::{DEFAULT_REPLY_TEMPLATES, default_reply_templates};
///
/// let owned = default_reply_templates();
/// assert_eq!(owned.len(), DEFAULT_REPLY_TEMPLATES.len());
/// assert!(!owned.is_empty());
/// ```
#[must_use]
pub fn default_reply_templates() -> Vec<String> {
    DEFAULT_REPLY_TEMPLATES
        .iter()
        .map(|template| (*template).to_owned())
        .collect()
}

#[cfg(test)]
mod defaults_tests;
```

Run the full gate sequence (sequentially, never in parallel) before each
CodeRabbit review:

```bash
make check-fmt 2>&1 | tee /tmp/checkfmt-frankie-$(git branch --show-current | \
  tr '/' '-').out
make lint      2>&1 | tee /tmp/lint-frankie-$(git branch --show-current | \
  tr '/' '-').out
make test      2>&1 | tee /tmp/test-frankie-$(git branch --show-current | \
  tr '/' '-').out
make markdownlint 2>&1 | tee /tmp/mdlint-frankie-$(git branch --show-current | \
  tr '/' '-').out
```

Then, only once the deterministic gates are green:

```bash
coderabbit review --agent
```

## Validation and acceptance

Red-Green-Refactor evidence to record in `Progress` and
`Outcomes & retrospective`:

- Red: `cargo test -p frankie --test reply_template_public_api` fails to compile
  because `frankie::DEFAULT_REPLY_TEMPLATES` /
  `frankie::default_reply_templates` do not exist yet.
- Green: after Stage C, the same command passes, and
  `cargo test -p frankie reply_template` passes (new unit tests plus existing
  renderer tests).
- Refactor: `make test` passes for the whole workspace with no behavioural test
  changes.

Behavioural acceptance (maps to the roadmap criterion):

1. Publicly available: an out-of-crate integration test in
   `tests/reply_template_public_api.rs` references
   `frankie::DEFAULT_REPLY_TEMPLATES` and `frankie::default_reply_templates()`
   and compiles.
2. Deterministic: `default_reply_templates()` equals itself across two calls and
   equals the constant mapped to owned strings.
3. Non-empty: `DEFAULT_REPLY_TEMPLATES` is non-empty.
4. Configured defaults remain present:
   `default_reply_templates_preserve_configured_defaults_in_order` asserts
   exact ordered contents, and
   `config_default_matches_public_default_reply_templates` proves the config
   default and public API agree.

Quality criteria (what "done" means):

- Tests: `make test` passes for the workspace; the new unit and integration
  tests pass; pre-existing reply-template, config, and BDD tests pass unchanged.
- Lint/format: `make check-fmt` and `make lint` (clippy with `-D warnings`)
  pass; `make markdownlint` passes for the edited docs.
- Review: `coderabbit review --agent` reports no outstanding concerns after the
  deterministic gates pass.

Quality method (how we check): run the gate sequence above sequentially, then
the CodeRabbit pass, after each major milestone (Stage C implementation and
Stage D documentation).

## Idempotence and recovery

All steps are re-runnable. The implementation is additive plus a single
deletion (the crate-private function); if a step fails midway, `git diff` shows
the partial state and the compiler/clippy identify any dangling reference to
the removed function. Reverting is `git restore` on the affected files. No
data, no migrations, and no external services are involved.

## Artifacts and notes

The final public surface added at the crate root:

```rust
// src/lib.rs
pub use reply_template::{
    DEFAULT_REPLY_TEMPLATES, ReplyTemplateContext, ReplyTemplateError,
    default_reply_templates, render_reply_template,
};
```

## Interfaces and dependencies

No new dependencies. The end-state public items, by stable path:

- `frankie::reply_template::DEFAULT_REPLY_TEMPLATES: &[&str]` (also re-exported
  as `frankie::DEFAULT_REPLY_TEMPLATES`).
- `frankie::reply_template::default_reply_templates() -> Vec<String>` (also
  re-exported as `frankie::default_reply_templates`).

Internal callers after the change:

- `frankie::config::FrankieConfig::default` calls
  `crate::reply_template::default_reply_templates()`.
- `frankie::tui::reply_draft_config::ReplyDraftConfig::default` calls
  `crate::reply_template::default_reply_templates()`.

## Signposted documentation and skills

- Skills: `hexagonal-architecture` (domain owns policy; adapters depend
  inward), `rust-router` then `arch-crate-design` (public surface, re-export
  layering) and `rust-types-and-apis` (const vs function contract),
  `rust-unit-testing` (rstest, googletest, pretty_assertions),
  `arch-decision-records` (the ADR-004 update), `leta` (navigation and `refs`/
  `rename`), `execplans` (this document).
- Repository docs: `docs/adr-004-inline-template-based-reply-drafting.md`
  (reply-drafting decision and templates),
  `docs/adr-005-cross-surface-library-first-delivery.md` (library-first
  delivery), `docs/frankie-design.md` (ADR index and the reply-template module
  description), `docs/developers-guide.md`, `docs/users-guide.md`,
  `docs/rust-doctest-dry-guide.md` (keep doctests minimal),
  `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/documentation-style-guide.md` (en-GB Oxford spelling, 80-column prose
  wrapping).
