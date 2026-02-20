# Implement template-based reply drafting with keyboard insertion

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Enable users to draft pull-request review replies directly in the TUI by
inserting configured templates with keyboard shortcuts, editing the draft
before sending, and seeing immediate length-limit feedback. After this change,
a user in review-list mode can open a reply composer for the selected comment,
insert a template with a keypress, edit the text inline, and be blocked from
sending if the configured maximum length is exceeded.

Success is observable when:

- A reply draft appears inline in the detail pane for the selected comment.
- Template insertion is keyboard driven (no mouse, no modal form required).
- The inserted text remains editable before send.
- Length limits from configuration are enforced during insertion and editing.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.5.0) cover
  happy paths, unhappy paths, and relevant edge cases.
- Documentation and roadmap updates are completed.
- `make check-fmt`, `make lint`, and `make test` all pass.

## Constraints

- Preserve MVU separation already used by the TUI:
  - input mapping in `src/tui/input.rs`
  - message definitions in `src/tui/messages.rs`
  - state transitions in `src/tui/app/`
  - rendering queries in `src/tui/components/` and `src/tui/app/rendering.rs`
- Keep Codex execution/session-resume behaviour unchanged while adding reply
  drafting.
- Do not add external dependencies; use existing crates (`serde`,
  `ortho-config`, `rstest`, `rstest-bdd`, `mockall`, `cap_std`, `camino`).
- Every new Rust module must start with a `//!` module-level comment.
- Keep files under 400 lines; split handlers/components/tests into focused
  modules when needed.
- Use capability-oriented filesystem and path APIs (`cap_std::fs_utf8`,
  `camino`) for any new file interactions.
- New unit tests must use `rstest` fixtures/parameterization.
- New behavioural tests must use `rstest-bdd` v0.5.0 with feature files under
  `tests/features/`.
- Record architectural decisions in `docs/frankie-design.md`.
- Document user-visible behaviour in `docs/users-guide.md`.
- Mark the relevant roadmap checklist item in `docs/roadmap.md` as done only
  after full validation passes.
- Required validation gates before completion:
  `make check-fmt`, `make lint`, `make test`.

## Tolerances (exception triggers)

- Scope: if implementation needs changes in more than 22 files or more than
  1,400 net new lines, stop and escalate.
- Interface: if existing public CLI flags must change incompatibly, stop and
  escalate.
- Dependencies: if any new dependency is required, stop and escalate.
- Protocol/API: if meeting acceptance requires introducing live GitHub comment
  submission APIs not already present, stop and confirm scope before adding the
  gateway surface.
- Iterations: if any stage fails validation more than three fix cycles, stop
  and escalate with logs.

## Risks

- Risk: keyboard mappings for reply drafting can conflict with existing review,
  diff-context, time-travel, and resume-prompt contexts. Severity: high.
  Likelihood: medium. Mitigation: add explicit `InputContext` routing for reply
  drafting and unit tests for each context to ensure key isolation.

- Risk: unclear length semantics (bytes vs Unicode scalar values) can cause
  inconsistent enforcement and user confusion. Severity: medium. Likelihood:
  medium. Mitigation: define one counting rule in code/docs (Unicode scalar
  count) and test with multi-byte content.

- Risk: `ReviewApp` and existing handler modules are already near size limits.
  Severity: medium. Likelihood: high. Mitigation: add focused reply modules
  (state/handlers/tests) instead of enlarging current files.

- Risk: behavioural tests can become timing-sensitive if they depend on async
  polling paths unrelated to reply drafting. Severity: medium. Likelihood: low.
  Mitigation: keep reply-drafting BDD scenarios synchronous and state-driven;
  inject deterministic fixtures.

## Progress

- [x] (2026-02-20 00:00Z) Drafted ExecPlan with constraints, tolerances,
      staged implementation, and validation criteria.
- [x] (2026-02-20 00:35Z) Stage A: finalised reply-drafting domain model and
      configuration surface (`ReplyDraftState`, template rendering helpers,
      and config keys `reply_max_length` / `reply_templates` with tests).
- [x] (2026-02-20 00:50Z) Stage B: implemented input/message plumbing and
      reply draft state transitions (`InputContext::ReplyDraft`, new `AppMsg`
      variants, routing/handlers).
- [x] (2026-02-20 01:05Z) Stage C: rendered inline drafts and wired
      edit-before-send interactions in the detail pane and status/help text.
- [x] (2026-02-20 02:10Z) Stage D: added unit and behavioural coverage for
      happy/unhappy/edge cases (`rstest` unit coverage plus
      `tests/template_reply_drafting_bdd.rs` scenarios).
- [x] (2026-02-20 02:30Z) Stage E: updated design/user docs, marked roadmap
      entry done, refreshed snapshots, and passed gates (`make check-fmt`,
      `make lint`, `make test`).

## Surprises & discoveries

- Discovery: `rstest-bdd` is already at `0.5.0` in `Cargo.toml`, so no
  dependency upgrade is needed for this milestone. Evidence: `Cargo.toml`
  dev-dependencies list `rstest-bdd = "0.5.0"` and
  `rstest-bdd-macros = "0.5.0"`. Impact: effort can focus on new scenarios and
  state harnesses.

- Discovery: there is currently no GitHub review-reply submission gateway in
  the codebase; current TUI support ends at comment viewing/filtering and Codex
  execution. Evidence: no `create`/`reply` review-comment gateway methods under
  `src/github/gateway/`. Impact: this plan scopes to
  drafting/editing/validation UX, with send treated as local draft readiness
  unless scope is explicitly expanded.

- Discovery: review-list status hints can overflow narrow widths and hide
  critical controls (`q:quit`) when additional shortcuts are appended.
  Evidence: failing test
  `tui::app::tests::tiny_terminal_skips_detail_pane_and_keeps_status_bar_visible`
   after adding `a:reply`. Impact: status hints now need a width-aware compact
  variant for narrow terminals.

## Decision log

- Decision: implement reply drafting as a dedicated TUI state slice with
  keyboard-only interactions, rather than embedding transient logic inside
  existing Codex handlers. Rationale: keeps responsibilities separated and
  testable under MVU. Date/Author: 2026-02-20 / plan author.

- Decision: length limits are enforced as Unicode scalar counts, not byte
  length or display width. Rationale: this rule is deterministic,
  language-agnostic, and aligns with existing text helpers that reason about
  character content. Date/Author: 2026-02-20 / plan author.

- Decision: this step covers draft insertion/editing/readiness in the TUI and
  does not introduce live GitHub reply submission unless explicitly requested.
  Rationale: roadmap acceptance for this step is drafting-focused (inline
  render, edit-before-send, length limit), while API submission is a larger
  surface with separate failure semantics. Date/Author: 2026-02-20 / plan
  author.

- Decision: keep the reply-template engine aligned with existing export
  templating by using `MiniJinja` and a focused comment-variable context.
  Rationale: this avoids introducing a second templating model and keeps user
  mental load low (`{{ reviewer }}`, `{{ file }}`, etc. behave consistently).
  Date/Author: 2026-02-20 / implementation.

- Decision: add width-aware review-list status hints so `q:quit` and `?:help`
  remain visible on narrow terminals even with new `a:reply` hints. Rationale:
  preserving escape hatches in constrained layouts is more important than
  showing every shortcut simultaneously. Date/Author: 2026-02-20 /
  implementation.

## Outcomes & retrospective

- Outcome: reply drafting is now keyboard-driven and inline in the detail pane,
  with template insertion (`1` to `9`), free-form edits, readiness marking, and
  cancel flow.
- Outcome: configured limits are enforced during typing and template insertion
  using Unicode scalar counts, with explicit user-facing errors for limit
  violations and invalid template slots.
- Outcome: configuration is additive and layered (`reply_max_length`,
  `reply_templates`) across defaults, config file, environment, and CLI.
- Outcome: feature validation now includes both unit and behavioural test
  coverage, and snapshot baselines were updated for status-hint changes.
- Retrospective: adding a new status-bar hint changed narrow-screen snapshots;
  width-aware hint compaction reduced future risk of truncating escape-hatch
  controls.

## Context and orientation

### Existing architecture relevant to this change

- `src/tui/app/mod.rs` holds `ReviewApp` state and currently tracks review
  data, view mode, Codex state, and session-resume prompt state.
- `src/tui/app/model_impl.rs` selects input context and routes mapped messages
  through `handle_message`.
- `src/tui/input.rs` maps key events to `AppMsg` per `InputContext`.
- `src/tui/messages.rs` defines typed messages for navigation, Codex flows,
  refresh/sync, and lifecycle.
- `src/tui/components/comment_detail.rs` renders selected-comment metadata,
  body, and code context; this is the natural place to render inline reply
  drafts.
- `src/config/mod.rs` defines `FrankieConfig` and ortho-config mapping from
  CLI/env/config-file layers.
- `docs/users-guide.md` contains keyboard tables and Codex/TUI behaviour docs.
- `docs/frankie-design.md` tracks architecture decision records (ADR-001..004),
  including the new reply-drafting decision.
- `docs/roadmap.md` contains the unchecked item for this step under:
  `Phase 3 -> Step: Template and reply automation`.

### Terminology used in this plan

- Reply draft: editable in-TUI text tied to the currently selected review
  comment.
- Template insertion: populating or appending draft text from a configured
  template slot using a keypress.
- Edit-before-send: user can modify inserted template text before triggering a
  send-intent action.
- Send-intent action: a local action that marks the draft ready for send in
  this step; remote submission is out of scope unless scope is expanded.

## Plan of work

### Stage A: Reply drafting domain + configuration

Add a focused reply-drafting state model and configuration contract. Keep this
stage strictly additive and test-first where possible.

Create `src/tui/state/reply_draft.rs` (new module) with:

- `ReplyDraftState` containing selected comment ID binding, current text,
  template provenance metadata, max-length setting, and ready-to-send marker.
- pure methods for insertion, append, replace, backspace, clear, and
  send-intent validation.
- explicit error/violation enum for over-limit and invalid template index.

Update `src/tui/state/mod.rs` exports and add unit tests (`rstest`) for:

- empty draft lifecycle
- template insertion success
- insertion/edit under limit
- over-limit rejection
- Unicode content length counting

Extend `FrankieConfig` in `src/config/mod.rs` with additive settings:

- `reply_max_length: usize` (defaulted)
- `reply_templates: Vec<String>` (default template set)

Add/extend config tests under `src/config/tests/` for defaults and layer
resolution behaviour.

Go/no-go for Stage A:

- Go when reply state helpers compile, defaults load, and focused unit tests
  pass.
- No-go if config layering cannot express template lists without brittle
  parsing; escalate with alternatives.

### Stage B: Input + message plumbing

Introduce reply-drafting messages and context-aware key mapping.

Modify `src/tui/messages.rs` with additive `AppMsg` variants for:

- opening draft mode for selected comment
- template insertion by slot
- character insertion/backspace
- send-intent request
- cancel/close draft mode

Extend `InputContext` in `src/tui/input.rs` with `ReplyDraft` mode and map:

- one key to start draft mode from review list (for example `a`)
- number keys (`1`-`9`) to template insertion in draft mode
- printable characters and backspace for editing
- `Enter` for send-intent and `Esc` for cancel

Update input mapping tests (`rstest`) to cover all contexts and prevent key
bleed into unrelated modes.

Go/no-go for Stage B:

- Go when mapping tests prove deterministic context behaviour.
- No-go if key conflicts with existing mandatory bindings cannot be resolved
  without UX regression.

### Stage C: ReviewApp integration + inline rendering + enforcement

Wire reply drafting into app state transitions and render inline draft content
in the comment-detail area.

Add dedicated handler module(s) under `src/tui/app/` (for example
`reply_handlers.rs`) and route from `src/tui/app/routing.rs` to avoid bloating
existing Codex handlers.

Update `ReviewApp` state in `src/tui/app/mod.rs` minimally to hold reply draft
state. If `mod.rs` approaches file-size constraints, move auxiliary structs and
helper methods into new modules.

Extend `CommentDetailViewContext` and `CommentDetailComponent` in
`src/tui/components/comment_detail.rs` to show:

- current draft text inline beneath the selected comment
- length indicator (`current/max`)
- over-limit or invalid-template feedback
- ready-to-send indication after `Enter`

Ensure send-intent path validates limits and blocks when invalid.

Add/extend unit tests in:

- `src/tui/app/*_tests.rs` for message handling and state transitions
- `src/tui/components/comment_detail_tests.rs` for inline render states

Go/no-go for Stage C:

- Go when manual interaction in tests shows inline draft rendering and enforced
  limits.
- No-go if rendering cannot remain deterministic without asynchronous coupling;
  refactor to pure view-state transformation first.

### Stage D: Behavioural coverage with rstest-bdd v0.5.0

Add new feature file `tests/features/template_reply_drafting.feature` and step
bindings in `tests/template_reply_drafting_bdd.rs` with deterministic scenario
state under `tests/template_reply_drafting_bdd/`.

Scenarios must include:

- Happy path: open draft mode, insert template via keyboard, render inline.
- Happy path: edit inserted template and trigger send-intent successfully.
- Unhappy path: insertion blocked when template exceeds configured length.
- Unhappy path: send-intent blocked when edited draft exceeds limit.
- Edge case: Unicode text counting around boundary limit.

Use `rstest` fixtures for shared app setup and deterministic template config.

Go/no-go for Stage D:

- Go when new BDD suite passes and does not destabilize existing features.
- No-go when scenarios are timing-dependent; simplify to synchronous state
  assertions.

### Stage E: Documentation, roadmap, and quality gates

Update `docs/frankie-design.md` with a new ADR entry describing:

- why reply drafting is local state in TUI
- keyboard template insertion model
- length-limit semantics
- send-intent scope for this step

Update `docs/users-guide.md` with:

- new keyboard shortcuts and reply-draft interaction flow
- template insertion usage
- length-limit behaviour and error messages

Update `docs/roadmap.md` by marking this item done after validation:

- `Provide template-based reply drafting with keyboard-driven insertion ...`

Then run full required gates and capture logs.

## Concrete steps

From repository root (`/home/user/project`), execute:

1. Implement Stage A-C code and unit tests in small, reviewable commits.
2. Implement Stage D behavioural tests and fixtures.
3. Update docs and roadmap in Stage E.
4. Run validation commands with log capture:

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/execplan-3-2-1-check-fmt.log
    set -o pipefail; make lint 2>&1 | tee /tmp/execplan-3-2-1-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/execplan-3-2-1-test.log

Recommended documentation gates when docs are touched:

    set -o pipefail; make markdownlint 2>&1 | tee /tmp/execplan-3-2-1-markdownlint.log
    set -o pipefail; make nixie 2>&1 | tee /tmp/execplan-3-2-1-nixie.log

Expected short transcripts:

- `make check-fmt`: exits `0`, no diff-needed formatter output.
- `make lint`: exits `0`, `cargo doc` and clippy complete without warnings.
- `make test`: exits `0`, all workspace tests pass.

## Validation and acceptance

Feature acceptance is satisfied when all of the following are observable:

- Reply draft renders inline in the selected comment detail pane.
- Template insertion works from keyboard shortcuts in reply-draft mode.
- User can edit inserted content before invoking send-intent.
- Length limit from configuration is enforced during insertion/editing and
  blocks send-intent when exceeded.

Testing acceptance:

- Unit tests (`rstest`) cover domain logic, message routing, and rendering
  helpers for both valid and invalid paths.
- Behavioural tests (`rstest-bdd` v0.5.0) cover happy, unhappy, and edge
  scenarios end-to-end at TUI interaction level.

Quality-gate acceptance:

- `make check-fmt` exits `0`.
- `make lint` exits `0`.
- `make test` exits `0`.

Documentation acceptance:

- `docs/frankie-design.md` contains design decisions for this feature.
- `docs/users-guide.md` documents new user-visible behaviour and shortcuts.
- `docs/roadmap.md` marks the relevant Phase 3 step as done.

## Idempotence and recovery

- All implementation/test steps are additive and re-runnable.
- If a stage fails, fix and re-run only the failing stage command set first,
  then full gates.
- If behavioural tests fail intermittently, replace timing waits with
  deterministic state-message sequencing before proceeding.
- Keep temporary logs in `/tmp/execplan-3-2-1-*.log`; they can be overwritten
  safely on retries.

## Artifacts and notes

Capture concise evidence during implementation:

- one unit-test output snippet proving over-limit rejection
- one behavioural-test snippet proving inline template insertion
- one behavioural-test snippet proving blocked send-intent over limit
- final gate logs from `/tmp/execplan-3-2-1-*.log`

## Interfaces and dependencies

Planned internal interfaces (names may vary, responsibilities must not):

- Reply draft state API in `src/tui/state/reply_draft.rs`, e.g.:

    pub struct ReplyDraftState {
        pub comment_id: u64,
        pub text: String,
        pub max_length: usize,
        pub ready_to_send: bool,
    }

    impl ReplyDraftState {
        pub fn insert_template(&mut self, template: &str) -> Result<(), ReplyDraftError>;
        pub fn push_char(&mut self, ch: char) -> Result<(), ReplyDraftError>;
        pub fn backspace(&mut self);
        pub fn request_send(&mut self) -> Result<(), ReplyDraftError>;
        pub fn char_count(&self) -> usize;
    }

- Additive config fields in `FrankieConfig`:

  `pub reply_max_length: usize` `pub reply_templates: Vec<String>`

- Additive message variants in `AppMsg` for reply draft lifecycle.

- `InputContext::ReplyDraft` in `src/tui/input.rs` with deterministic
  key mapping.

No new external dependencies are expected.

## Revision note

Initial draft created for Phase 3, Step "Template and reply automation" item
"Provide template-based reply drafting with keyboard-driven insertion". This
revision fixes scope on inline drafting, keyboard template insertion,
edit-before-send interaction, and configured length enforcement, while
explicitly requiring `rstest` unit coverage, `rstest-bdd` behavioural coverage,
design/user-doc updates, roadmap completion, and final quality gates.
