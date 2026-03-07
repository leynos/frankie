# Implement PR-level discussion summary generation

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT (2026-03-06)

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Add a PR-level discussion summary workflow that turns the full review comment
set for a pull request into a concise, actionable summary. After this change, a
user can request a summary and receive output grouped by file and severity,
with each summary item carrying a stable link back to the TUI comment-detail
view that anchors the underlying discussion.

This roadmap step must ship library-first. The shared library produces a
structured summary model. The CLI exposes a non-interactive summary mode. The
TUI exposes an interactive summary view that can jump back to the review list
and comment detail for the referenced thread.

Success is observable when:

- a library consumer can summarize a pull request discussion without
  importing TUI modules;
- the generated summary groups items under file headings and severity
  buckets with deterministic ordering;
- each summary item includes a stable TUI link target for the underlying
  discussion thread;
- the CLI can print the summary and those link targets non-interactively;
- the TUI can open a summary view, render the same grouped content, and
  navigate from a summary entry back to the referenced comment detail view;
- unit tests written with `rstest` and behavioural tests written with
  `rstest-bdd` v0.5.0 cover happy paths, unhappy paths, and edge cases;
- design decisions are recorded in `docs/frankie-design.md`;
- `docs/users-guide.md` explains the new CLI mode and TUI interaction;
- `docs/roadmap.md` marks step 3.3.2 done only after all acceptance
  gates pass; and
- `make check-fmt`, `make lint`, and `make test` all pass.

## Constraints

- Keep core summary logic in shared library modules under `src/ai/` or
  another non-TUI top-level module. TUI and CLI must remain thin adapters.
- Follow the cross-surface contract in
  `docs/adr-005-cross-surface-library-first-delivery.md`.
- Preserve TUI Model-View-Update separation:
  - key mapping in `src/tui/input.rs`;
  - messages in `src/tui/messages.rs`;
  - state transitions and async orchestration in `src/tui/app/`;
  - render-only components in `src/tui/components/`.
- Do not push summary logic into `src/tui/state/` or `src/cli/`.
- Keep the AI-provider boundary dependency-injected so tests can use
  stubs or deterministic local mocks instead of mutating environment state.
- Use `vidaimock` for end-to-end tests that exercise the real
  OpenAI-compatible generative summary path. Do not depend on live network
  providers in local or CI gates.
- Every new Rust module must begin with a `//!` module comment.
- Public APIs must have Rustdoc comments. Examples should be included on
  public functions where they materially improve discoverability.
- No source file may exceed 400 lines. Split handlers, renderers, and
  tests aggressively when needed.
- Behavioural tests must use `rstest-bdd` v0.5.0 and
  `#[scenario(path = ...)]`.
- Shared BDD helpers that return `Result` must not use `assert!`; return
  explicit errors instead, to satisfy the repo's strict Clippy rules.
- Markdown documentation changes must pass `make fmt`,
  `make markdownlint`, and `make nixie`.
- Do not mark roadmap step 3.3.2 as done until implementation, tests,
  documentation, and required gates are all complete.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 35 files or more
  than 2,500 net new lines, stop and escalate with a staged alternative.
- Interface: if satisfying the CLI access-path requirement would force an
  incompatible change to existing operation-mode precedence, stop and escalate
  with options.
- Provider contract: if OpenAI-compatible structured output cannot be
  validated deterministically with a schema-backed response shape, stop and
  escalate before introducing heuristic parsing.
- TUI navigation: if "links back to TUI views" cannot be implemented
  without adding a general deep-link router well outside this feature's scope,
  stop and escalate with a narrower link contract.
- Iterations: if any milestone fails validation after three fix cycles,
  stop and escalate with logs and the current diff.

## Risks

- Risk: the model invents summaries or severity levels that are not
  traceable to the source comments. Severity: high. Likelihood: medium.
  Mitigation: require structured JSON output, validate against a strict schema,
  and keep grouping/order/link construction deterministic in the library.
- Risk: there is no existing severity field on `ReviewComment`, so the
  meaning of severity can drift between adapters. Severity: high. Likelihood:
  medium. Mitigation: introduce a shared `DiscussionSeverity` enum and record
  the allowed values and their intended meaning in a new ADR.
- Risk: summary links become CLI-only strings and the TUI implements
  separate ad hoc navigation. Severity: medium. Likelihood: medium. Mitigation:
  represent links as structured data in the shared model and let adapters
  choose how to render or execute them.
- Risk: review threads with replies lose context or are summarized twice.
  Severity: medium. Likelihood: medium. Mitigation: summarize at the
  thread-root level, using `in_reply_to_id` to collapse replies before grouping
  by file.
- Risk: adding a new TUI view regresses narrow-terminal rendering and
  shortcut hints. Severity: medium. Likelihood: medium. Mitigation: add
  component tests and, if the layout changes materially, snapshot tests for
  small and large terminal sizes.
- Risk: the summary adapter works with stubs but fails against a real
  OpenAI-compatible HTTP surface. Severity: high. Likelihood: medium.
  Mitigation: add `vidaimock`-backed end-to-end coverage for success, malformed
  JSON, and forced-failure scenarios using explicit per-request chaos headers.

## Progress

- [x] (2026-03-06 00:00Z) Read roadmap, adjacent execplans, current
      verification and AI flows, and the referenced testing/design docs.
- [x] (2026-03-06 00:00Z) Drafted this ExecPlan for roadmap step 3.3.2.
- [ ] Stage A: lock the summary domain model, severity taxonomy, and TUI
      link contract.
- [ ] Stage B: add failing unit and behavioural tests covering library,
      CLI, and TUI paths.
- [ ] Stage C: implement shared summary orchestration and provider
      adapter.
- [ ] Stage D: wire the CLI access path and output formatter.
- [ ] Stage E: wire the TUI summary view and jump-back navigation.
- [ ] Stage F: update design and user documentation, mark roadmap done,
      and pass all required gates.

## Surprises & Discoveries

- `ReviewComment` currently carries no severity field, so severity must
  be introduced by this feature's summary domain rather than recovered from
  existing GitHub payloads.
- The current TUI has only three view modes
  (`ReviewList`, `DiffContext`, `TimeTravel`), so an interactive summary
  surface will need either a new view mode or a full-screen overlay with its
  own input context.
- The CLI already uses mutually exclusive top-level operation modes for
  AI rewrite and verification. A PR-summary CLI path should follow the same
  pattern rather than being hidden inside export mode.
- `rstest-bdd` v0.5.0 is already in use in this repository, and the
  existing verification/AI flows provide workable patterns for splitting CLI
  and TUI behavioural tests.
- `vidaimock` is available in this environment at `/root/.local/bin/vidaimock`,
  so the plan can require a local OpenAI-compatible mock server for generative
  end-to-end scenarios instead of treating it as optional.

## Decision Log

- Decision (2026-03-06): summarize review threads, not individual comment
  rows. Rationale: PR-level discussions are easier to reason about when a reply
  chain is represented once, and this prevents duplicate summary items for root
  comments plus replies.
- Decision (2026-03-06): keep file grouping and severity buckets
  deterministic in the shared library, while reserving the AI model for the
  natural-language condensation and severity assignment of each thread.
  Rationale: this minimizes output drift and keeps adapters aligned.
- Decision (2026-03-06): define "links back to TUI views" as structured
  targets that identify the comment-detail view for a specific root comment,
  with adapters free to render them as a URI-like string or use them directly
  for in-app navigation. Rationale: the acceptance needs a reusable library
  API, not TUI-only ad hoc jump logic.
- Decision (2026-03-06): on provider/configuration/schema failure, return
  a clear error instead of fabricating a heuristic prose fallback. Rationale:
  summaries drive prioritization; a misleading fallback is worse than an
  explicit failure.
- Decision (2026-03-06): record the final contract in a new ADR-008 in
  `docs/` and add it to the ADR index in `docs/frankie-design.md`. Rationale:
  the feature introduces a new public library model, a new severity taxonomy,
  and a new TUI-link contract.
- Decision (2026-03-07): use `vidaimock` for end-to-end tests that cover the
  real generative summary adapter, while keeping pure unit tests and TUI-only
  behavioural tests on injected stubs. Rationale: this gives deterministic
  provider-level coverage without making UI tests depend on an external process.

## Outcomes & Retrospective

This feature is not implemented yet. When complete, this section should
describe:

- the shared summary library API and its final module layout;
- the CLI and TUI surfaces that consume it;
- the tests that prove happy, unhappy, and edge-case behaviour; and
- any follow-up refactors or deferred work discovered during delivery.

## Context and orientation

The existing repository already contains the pieces this feature must compose
rather than replace.

- `src/github/models/mod.rs` defines `ReviewComment`, including
  `file_path`, `body`, `in_reply_to_id`, and timestamps needed to build
  discussion threads.
- `src/ai/comment_rewrite/` shows the current repository pattern for an
  AI-backed, library-first feature with a provider adapter, deterministic
  tests, and both CLI and TUI consumers.
- `tests/openai_vidaimock.rs` shows that the repository already accepts
  `vidaimock` as the local OpenAI-compatible integration harness for AI-backed
  behaviour.
- `src/verification/` and `src/cli/verify_resolutions.rs` show how a
  shared service is exposed as a standalone CLI mode.
- `src/tui/app/verification_handlers.rs`,
  `src/tui/app/view_mode.rs`, and `src/tui/components/review_list.rs` show the
  existing async-message, view-mode, and rendering conventions for non-trivial
  TUI features.
- `src/config/mod.rs` and `src/main.rs` show how new top-level CLI
  operation modes are added, validated, and dispatched.
- `docs/users-guide.md` already documents AI rewrite and verification
  workflows, so this feature should extend those sections rather than invent a
  parallel document structure.

Terminology used in this plan:

- Thread root: the top-level review comment in a reply chain. A comment
  with `in_reply_to_id = None` is its own root.
- Discussion thread: one root review comment plus any replies attached to
  it.
- Summary item: one structured summary record for a discussion thread,
  including severity, short headline, rationale, referenced comment IDs, and a
  TUI link target.
- File group: the set of summary items whose thread roots point at the
  same file path. Threads without a file path must be grouped under a stable
  fallback label such as `(general discussion)`.
- TUI link target: structured data that can open the comment-detail view
  for the thread root comment inside the review TUI.

## Proposed behaviour and contract

The feature should expose one shared entry point, for example
`PrDiscussionSummaryService::summarize(&PrDiscussionSummaryRequest)`, that
returns a structured `PrDiscussionSummary`. The request should carry:

- pull request identity and title for prompt context;
- the review comments to summarize;
- optional cached verification results keyed by comment ID so the prompt
  can mention comments already verified as fixed;
- a link-construction helper or plain pull-request metadata needed to
  build TUI targets deterministically.

The response model should be entirely library-owned and serializable in tests.
A workable shape is:

- `PrDiscussionSummary { files: Vec<FileDiscussionSummary> }`
- `FileDiscussionSummary { file_path, severities: Vec<SeverityBucket> }`
- `SeverityBucket { severity, items: Vec<DiscussionSummaryItem> }`
- `DiscussionSummaryItem {`
  `root_comment_id, related_comment_ids, headline, rationale,`
  `severity, tui_link` `}`

Use a shared `DiscussionSeverity` enum with explicit, documented values:
`High`, `Medium`, and `Low`. If the team later wants a fourth value (`Unknown`
or `Informational`), that change must be reflected in the ADR and tests.

Stable ordering matters. The library should sort:

1. file groups by file path, with the fallback "general" bucket last;
2. severity buckets by `High`, then `Medium`, then `Low`;
3. items within a bucket by root comment ID or earliest comment
   timestamp, whichever is easier to make deterministic from the current model.

The TUI link contract should be structured first and string-rendered second. A
concrete model such as
`TuiViewLink { comment_id: GithubCommentId, view: TuiView::CommentDetail }` is
sufficient. The CLI can render it as a URI-like token such as
`frankie://review-comment/<id>?view=detail`; the TUI should use the same model
directly to jump to the linked comment.

## Plan of work

### Stage A: define the shared summary domain and failing tests first

Create a dedicated shared module, likely `src/ai/pr_discussion_summary/`, with
submodules such as:

- `mod.rs` for public exports;
- `model.rs` for `DiscussionSeverity`, `TuiViewLink`, and summary DTOs;
- `service.rs` for the main service trait and orchestration;
- `openai.rs` for the OpenAI-compatible provider adapter;
- `threads.rs` for deterministic thread/root grouping helpers.

Before implementing the provider, add failing unit tests that lock the non-AI
contract:

- thread grouping collapses replies into one root summary input;
- missing `file_path` groups to the fallback file bucket;
- severity/file ordering is deterministic;
- the library produces stable TUI links for root comments;
- malformed provider output produces a typed error rather than partial
  output.

Use `rstest` fixtures to keep repeated review-comment setup readable.
Thread-building helpers must stay small and test-friendly.

Go/no-go for Stage A:

- Go when the DTOs, severity levels, and link contract are precise enough
  to document and test without the TUI present.
- No-go if link semantics require a repo-wide deep-link architecture not
  already implied by the acceptance wording.

### Stage B: implement provider contract and orchestration

Implement the shared orchestration so that only one step is AI-driven:
condensing each discussion thread into a headline, rationale, and severity.
Everything else stays deterministic.

Recommended flow:

1. Build thread roots and reply sets from `ReviewComment` values.
2. Convert each thread into a prompt-ready input record that includes:
   file path, root comment text, reply texts, author names, timestamps, and
   optional verification hints.
3. Call a provider trait such as
   `DiscussionSummaryProvider::summarize_threads(...)`.
4. Validate the provider response against the allowed severity enum and
   thread IDs returned.
5. Join the validated provider output with deterministic file grouping and
   TUI link generation.

Do not let the provider choose file buckets or link strings. Those are
shared-library responsibilities.

Unit tests in this stage should cover:

- provider success with multiple files and severities;
- provider returns an unknown thread ID;
- provider omits a required field;
- provider returns an invalid severity string;
- empty comment lists produce a clear, user-readable error.

If useful, mirror the AI rewrite feature by adding a small test-support stub
provider under `cfg(any(test, feature = "test-support"))`.

In addition to the pure unit tests, reserve a separate end-to-end test file for
the generative path, for example `tests/pr_discussion_summary_vidaimock.rs`.
That test should boot `vidaimock`, point the production adapter at its local
base URL, and verify at least:

- successful structured-summary generation against an OpenAI-compatible
  endpoint;
- malformed JSON handling using a deterministic chaos header such as
  `X-Vidai-Chaos-Malformed`;
- forced provider failure using a deterministic request-level chaos header
  rather than random flakiness.

### Stage C: add the CLI access path

Introduce a dedicated CLI mode for PR discussion summarization. A likely shape
is:

- config field: `summarize_discussions: bool`;
- operation mode: `OperationMode::SummarizeDiscussions`;
- handler: `src/cli/summarize_discussions.rs`.

The CLI mode should:

1. resolve the pull request locator exactly as other PR-centric modes do;
2. load review comments from GitHub;
3. optionally load cached verification results when a database is
   configured, but not require the database if the summary contract does not
   depend on it;
4. invoke the shared summary service;
5. print grouped output in a stable, plain-text format that includes the
   TUI link token for each summary item.

Add CLI validation rules in `src/config/mod.rs` to prevent ambiguous mode
selection, following the patterns already used for AI rewrite and verification.

Behavioural coverage for the CLI should use `rstest-bdd` with a dedicated
feature file, for example `tests/features/pr_discussion_summary.feature`.
Scenarios should include:

- successful summary with multiple files and severities;
- provider/configuration failure;
- no review comments returned for the PR;
- comments with replies collapsing into one summary item;
- comments with no file path grouping into the fallback section.

At least the generative happy path and one unhappy path in the CLI end-to-end
suite should run against `vidaimock` rather than a stubbed provider so the
actual HTTP adapter, request payload, and structured-response parsing are all
covered.

### Stage D: add the TUI access path and navigation

Add a dedicated TUI summary surface rather than trying to squeeze the summary
into the status bar or comment-detail footer.

Recommended implementation outline:

- extend `ViewMode` with a summary mode such as
  `PrDiscussionSummary`;
- add `AppMsg` variants for:
  - requesting summary generation;
  - receiving summary results;
  - failing summary generation;
  - moving through summary entries;
  - opening a summary link;
  - closing the summary view;
- add input mappings in `src/tui/input.rs`, choosing a key that does not
  conflict with existing review-list bindings;
- add a render component such as
  `src/tui/components/pr_discussion_summary.rs`;
- add handler/orchestration code under `src/tui/app/`.

The TUI flow should be:

1. user presses the summary key from the review list;
2. TUI requests the shared library summary using the currently loaded
   comments;
3. summary view renders file headings and severity sections;
4. user moves through summary items;
5. activating an item uses the embedded TUI link target to jump back to
   the linked comment in the review list and comment-detail pane.

The summary view does not need a general-purpose link parser. It only needs
enough navigation logic to focus the referenced root comment.

Testing for this stage should include:

- `rstest` component tests for rendering and clamping at narrow widths;
- behavioural tests for entering the summary view, showing grouped data,
  surfacing failures, and jumping back to the linked comment;
- snapshot updates only if the new shortcut or footer text changes the
  default TUI frame. Rebuild any snapshot fixture binary before updating
  snapshots if the test harness requires it.

### Stage E: provider integration, documentation, and design capture

Once the shared model and both adapters exist, finish the provider and
documentation work.

Implementation tasks:

- add an OpenAI-compatible provider implementation reusing the repo's
  existing AI client/configuration patterns where possible;
- add `vidaimock`-backed end-to-end tests for the real summary adapter,
  following the same local-mock pattern already used for AI rewrite;
- record the final contract in a new
  `docs/adr-008-pr-discussion-summary-contract.md`;
- add ADR-008 to the index in `docs/frankie-design.md`;
- update `docs/users-guide.md` with:
  - the new CLI mode and example invocation;
  - TUI keybinding and navigation behaviour;
  - explanation of severity buckets and TUI link targets;
- mark roadmap step 3.3.2 done in `docs/roadmap.md` only after tests and
  gates pass.

If the provider requires new CLI/config fields, document them in
`docs/users-guide.md` and in the config Rustdoc in `src/config/mod.rs`.

## Validation plan

Work red-green-refactor. Add failing tests before implementation for each
stage, then make them pass, then refactor with the suite green.

Targeted validation during development:

```plaintext
cargo test pr_discussion_summary
cargo test tui_pr_discussion_summary
cargo test --test pr_discussion_summary_bdd
cargo test --test tui_pr_discussion_summary_bdd
cargo test --test pr_discussion_summary_vidaimock
```

Before running the `vidaimock`-backed tests, confirm the binary is available:

```plaintext
command -v vidaimock
```

The `vidaimock` scenarios should use explicit per-request controls instead of
global randomness so failures are reproducible. Prefer request-local malformed
JSON, forced-error, and latency headers over ambient server-wide chaos.

Repository completion gates for the final implementation turn:

```bash
set -o pipefail
make fmt | tee /tmp/frankie-make-fmt-3-3-2.log
```

```bash
set -o pipefail
MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint | tee /tmp/frankie-markdownlint-3-3-2.log
```

```bash
set -o pipefail
make nixie | tee /tmp/frankie-nixie-3-3-2.log
```

```bash
set -o pipefail
make check-fmt | tee /tmp/frankie-check-fmt-3-3-2.log
```

```bash
set -o pipefail
make lint | tee /tmp/frankie-lint-3-3-2.log
```

```bash
set -o pipefail
make test | tee /tmp/frankie-test-3-3-2.log
```

Expected observable outcomes at completion:

- the CLI prints grouped summary output with file headings, severity
  sections, and TUI link tokens;
- the TUI opens a summary view and can jump from a summary item back to
  the linked comment detail;
- the design doc and user guide describe the contract the code now
  implements;
- `docs/roadmap.md` shows step 3.3.2 as done; and
- all logs above end in success.
