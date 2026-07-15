# Replace shared `TuiViewLink` references with host-neutral review-view references (3.3.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETED

## Purpose / big picture

Frankie's shared PR-discussion summary contract currently exposes a type named
`TuiViewLink` (and a companion enum `TuiView`), and the summary item field is
named `tui_link`. The name bakes one delivery surface — the Terminal User
Interface (TUI) — into a data transfer object (DTO) that is meant to be
host-neutral and consumed by the library, the Command Line Interface (CLI), and
embedded hosts alike. ADR-010 identifies this as an architectural gap: "summary
contracts currently expose `TuiViewLink`, which bakes one delivery surface into
the shared model".

After this change, a consumer of the `frankie` library will find a host-neutral
`ReviewViewRef` value object (with an accompanying `ReviewView` enum) on each
summary item, carrying only *which review comment* and *which logical view* the
summary points at. The `frankie://review-comment/<id>?view=detail` deep link —
previously produced by a `Display` implementation on the DTO — is rendered by a
separate, explicitly named presentation helper (`FrankieDeepLink`), shared by
the CLI and the TUI. The DTO itself carries no rendering behaviour and
serializes to host-neutral fields.

You can observe success three ways:

1. `cargo run -- --summarize-discussions …` (or the existing CLI behavioural
   test) still prints `Link: frankie://review-comment/1?view=detail` — the
   user-visible output is unchanged.
2. The TUI summary view still renders the same `frankie://…` token and the
   "jump to linked comment" action still works.
3. A new library test serializes a `PrDiscussionSummary` to JSON and proves the
   wire form contains only host-neutral fields (`view_ref`, `comment_id`,
   `view`) and no TUI-specific type name, deep-link string, or `tui_link` field.

This is a behaviour-preserving refactor: the only *new* behaviour is the
serialization guarantee. Every existing behavioural assertion (the `.feature`
files, the component render tests, the VidaiMock integration test) must keep
passing unchanged, because the rendered output is identical.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

- The rendered deep-link string MUST remain exactly
  `frankie://review-comment/<id>?view=detail`. The URI format is explicitly out
  of scope (see Decision Log: the `//` authority form is mildly non-conformant
  per RFC 7595 §3.2 but is preserved deliberately).
- The CLI's user-visible output and the TUI's rendered rows MUST be
  byte-identical to the current behaviour. The existing `.feature` files and
  render assertions are the regression net and MUST NOT be weakened to pass.
- The host-neutral DTO (`ReviewViewRef`, `ReviewView`) MUST NOT carry any
  rendering behaviour: no `impl Display` producing a URI, and no dependency on
  any `tui`, `bubbletea_rs`, or other UI-framework type. Domain purity per the
  hexagonal `hexagonal-architecture` skill: the shared model is a driven-port
  value object; rendering is an adapter/presentation concern.
- Public-surface change is intentional and sanctioned by ADR-010. This is a
  hard rename with NO deprecated alias (see Decision Log D2). The crate is
  `0.1.0` (pre-1.0) with no `cargo-public-api`/`cargo-semver-checks` gate, so
  no SemVer guard blocks the break.
- Do not modify modules outside the PR-discussion-summary feature and its
  direct consumers, the relevant docs, and the roadmap entry.
- Keep all work on the branch
  `3-3-3-replace-shared-tui-references-with-host-neutral-references`. Commit
  after each milestone, gating every commit.

## Tolerances (exception triggers)

- Scope: if the migration requires touching more than 20 files (the inventory
  predicts ~14 source/test/doc files) or more than ~400 net lines, stop and
  escalate.
- Interface: the rename itself is the sanctioned interface change. If any
  *additional* public API signature must change (e.g. a consumer outside the
  inventory), stop and escalate.
- Dependencies: if any new external crate is required, stop and escalate. None
  is expected.
- Iterations: if `make check-fmt`, `make lint`, or `make test` still fail after
  3 focused attempts on the same milestone, stop and escalate.
- Behaviour drift: if making the code compile would require changing any
  existing behavioural assertion's expected output (not just its construction
  syntax), stop and escalate — that signals an unintended behaviour change.
- Ambiguity: if the serde wire form of `ReviewView` turns out to be depended on
  by a persisted fixture or snapshot, stop and escalate before changing it.

## Risks

- Risk: a hidden consumer (insta snapshot, persisted fixture, doctest)
  references the old symbol or the `tui_link` field and is missed. Severity:
  medium. Likelihood: low. Mitigation: the exhaustive inventory in "Context and
  orientation" lists every site; additionally run
  `rg -n 'TuiViewLink|TuiView|tui_link'` after the migration and expect zero
  hits in `src/`, `tests/`, and `benches/`. The inventory confirmed no insta
  snapshots reference these symbols.
- Risk: the shared `FrankieDeepLink` helper re-introduces a coupling that the
  ADR wanted removed. Severity: low. Likelihood: low. Mitigation: per D8 the
  helper is a presentation newtype in a clearly named `deep_link` submodule,
  not part of the DTO; the *canonical* shared format is the serde
  representation of `ReviewViewRef`, and the helper renders one projection.
  This matches ADR-010 ("TUI deep links are rendered from those references as
  an adapter concern") and its non-goal ("Making TUI deep links the canonical
  shared navigation format"). The helper is host-neutral (usable by any Frankie
  surface), not TUI-only. Lock the invariant in rustdoc: `ReviewViewRef`'s doc
  states it deliberately has no `Display`/URI rendering and intra-doc-links to
  `FrankieDeepLink` (Dinolump 🔴), so a future edit cannot silently re-add a URI
  `Display` and resurrect the coupling.
- Risk: changing the `ReviewView` serde representation breaks a wire consumer.
  Severity: low. Likelihood: low. Mitigation: preserve the existing default
  serde derivation (no `rename_all`); `CommentDetail` continues to serialize as
  `"CommentDetail"`. Treat any change here as out of scope.
- Risk: ADR amendment conflicts with the documentation style guide's ADR
  conventions. Severity: low. Likelihood: low. Mitigation: follow
  `docs/documentation-style-guide.md` §"Architectural decision records": keep
  ADR-008 `Accepted`, bump its `Date`, and add a dated amendment note pointing
  to ADR-010 and roadmap 3.3.3.

## Progress

- [x] (Stage A) Plan approved by the user (approval gate). Approved for
  implementation on 2026-06-23 in the Lody session for PR #68.
- [x] (Stage B / Red) Add the host-neutral serialization test and the
  `FrankieDeepLink` rendering test describing the target API; observe them fail
  to compile / fail for the expected reason. On 2026-06-23,
  `cargo nextest run -p frankie review_view_ref_serialization_is_host_neutral
  review_view_ref_renders_as_frankie_deep_link` failed with unresolved imports
  for `ReviewView`, `ReviewViewRef`, `FrankieDeepLink`, and missing field
  `view_ref`, which is the expected red failure.
- [x] (Stage C / Green, milestone M1) Introduce `ReviewView`, `ReviewViewRef`,
  and `FrankieDeepLink`; rename the `tui_link` field to `view_ref`; migrate
  every consumer and re-export; make the workspace compile and all tests pass.
- [x] (Stage C, milestone M2) Update construction sites in tests and confirm
  behavioural assertions (feature files, component/service/VidaiMock tests)
  pass unchanged. On 2026-06-23, `make check-fmt`, `make lint`, and
  `make test` passed; the full test gate ran 882 tests, all passed, with one
  skipped.
- [x] (Stage D, milestone M3) Update documentation: ADR-008 amendment,
  `frankie-design.md`, `users-guide.md`, `developers-guide.md`. On
  2026-06-23, `make markdownlint` and `make nixie` passed after these
  documentation edits.
- [x] (Stage D, milestone M4) Mark roadmap 3.3.3 done; run the full gate suite
  and `coderabbit review --agent`; clear all concerns. On 2026-06-23, final
  gates passed (`make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie`) and CodeRabbit reported zero findings.

## Surprises & discoveries

- Observation: on 2026-07-15 the stale status warning was corrected; Wyvern
  verified `FrankieDeepLink` already has exact helper-level coverage, so only
  the PR-summary component render snapshot was added. Impact: the final review
  follow-up stays narrowly scoped.
- Observation: implementation resumed in worktree
  `/home/leynos/.lody/repos/github---leynos---frankie/worktrees/ab496267-b105-41d3-9de5-e126c3e6c779`,
  not the older planning worktree path shown in "Concrete steps". Impact:
  commands are run from the current worktree root while preserving the same
  branch and plan filename.
- Observation: `make fmt` runs Rust formatting and then all-repository
  Markdown formatting. Rust formatting completed, but the command failed on an
  unrelated pre-existing Markdown line-length violation in
  `docs/execplans/3-1-2-session-resumption-for-interrupted-codex-runs.md:392`.
  The formatter also rewrote unrelated Markdown files before failing; those
  unrelated edits were restored. Impact: code milestone gates use `cargo fmt`
  / `make check-fmt` first, and the all-doc Markdown gate must be revisited in
  the documentation milestone without weakening this plan's scope constraint.
- Observation: after introducing `ReviewViewRef` and `FrankieDeepLink`, the
  focused tests
  `review_view_ref_serialization_is_host_neutral` and
  `review_view_ref_renders_as_frankie_deep_link` both pass, and
  `rg -n 'TuiViewLink|TuiView|tui_link|selected_link' src tests` returns no
  matches. Impact: the code migration appears complete before wider gates.
- Observation: Clippy rejected chained indexing into the JSON value in the new
  serialization test under `clippy::indexing_slicing`. Impact: the test now
  uses `Value::pointer(...).expect(...)`, keeping the assertion explicit while
  respecting the repository lint policy.
- Observation: CodeRabbit reviewed the code milestone three times. It first
  requested repository-style cleanup for `FrankieDeepLink::new`, then requested
  rustdoc examples for the public wrapper and constructor. Both concerns were
  fixed; after rerunning `make check-fmt`, `make lint`, and `make test`,
  CodeRabbit reported zero findings. Impact: M1/M2 review is clear.
- Observation: the TUI "jump to linked comment" handler
  (`open_selected_pr_discussion_summary_link`) only consumes
  `link.comment_id.as_u64()`; it never parses the `frankie://` string. Evidence:
  `src/tui/app/pr_discussion_summary_handlers.rs:140-152`. Impact: the
  host-neutral reference needs only `comment_id` + `view`; removing the
  `Display` impl from the DTO cannot break navigation.
- Observation: the dependency "Requires 2.1.3" (public `ReviewThread`
  aggregate) is nominal for this item. The summary code keys on
  `GithubCommentId`, not on `ReviewThread`, so the rename is implementable
  independently of 2.1.3. Evidence: `src/ai/pr_discussion_summary/service.rs`
  and `model.rs` reference only `GithubCommentId`/`ReviewComment`. Impact: see
  Decision Log D5 — proceed now; the dependency is about conceptual alignment
  (host-neutral contracts), already satisfied by this change.
- Observation: because the CLI keeps emitting the same `frankie://` link, the
  `.feature` files and the VidaiMock string assertion need no change to their
  *expected output* — only Rust construction sites change. Evidence:
  `tests/features/pr_discussion_summary.feature:10`,
  `tests/features/tui_pr_discussion_summary.feature:10`,
  `tests/pr_discussion_summary_vidaimock.rs:87`. Impact: strong, cheap proof of
  behaviour preservation.
- Observation: a community-of-experts panel review (Pandalump, Telefono,
  Doggylump/Buzzy Bee completeness audit, Dinolump) refined this plan before
  approval. Key adopted findings: the single `Display` compile-breaker at
  `pr_discussion_summary_state.rs:203`; an exact-JSON + round-trip acceptance
  assertion (verified shape `{ "comment_id": 42, "view": "CommentDetail" }`,
  `GithubCommentId` serializing transparently as a bare number); missed rustdoc
  prose and the `tui_link_formats_as_uri_like_token` test-fn name; the
  presentation-submodule framing for `FrankieDeepLink` (D8); the
  `FrankieDeepLink::new` shape; and bidirectional intra-doc rustdoc. Evidence:
  panel findings, 2026-06-18. Impact: folded into Constraints, Decision Log,
  the inventory, Plan of work, and Validation.

## Decision log

- Decision (D1): name the host-neutral types `ReviewViewRef` (value object) and
  `ReviewView` (enum), and rename the field `tui_link` to `view_ref`.
  Rationale: roadmap 3.3.3 suggests `ReviewViewRef` "or equivalent"; prior-art
  research confirms `Ref` correctly signals "identifies a target, with no
  embedded presentation/transport semantics", which is exactly the
  responsibility being stripped out of the old `…Link` name (a "link" implies a
  dereferenceable URI). `Ref` is also idiomatic in Rust for lightweight
  identifying value types. Date/Author: 2026-06-18, planning.
- Decision (D2): hard rename with no deprecated alias. Rationale: user
  selection; crate is `0.1.0` and ADR-010 explicitly sanctions the break; an
  alias would keep the TUI-coupled name alive in the public surface, defeating
  the purpose. Date/Author: 2026-06-18, user.
- Decision (D3): the CLI keeps printing
  `Link: frankie://review-comment/<id>?view=detail`,
  rendered through the shared, host-neutral `FrankieDeepLink` presentation
  helper rather than a `Display` on the DTO. Rationale: user selection ("Keep
  frankie:// via shared helper"); preserves user-visible output, keeps the DTO
  rendering-free, and avoids a CLI→TUI dependency because the helper is
  host-neutral. Date/Author: 2026-06-18, user.
- Decision (D4): render the deep link via a newtype `Display` wrapper
  `FrankieDeepLink<'a>(&'a ReviewViewRef)` rather than a free function, with a
  private field and a `pub const fn new(&'a ReviewViewRef) -> Self` constructor
  (call sites use `FrankieDeepLink::new(&item.view_ref)`). Rationale: prior-art
  research and the Rust newtype pattern favour a wrapper for zero-allocation,
  composable formatting (`write!`/`format!`) while keeping `Display` off the
  domain type; it matches the existing call sites that do `.to_string()` /
  `format!("{}", …)`. A private field + `new()` (Dinolump 🟡) avoids committing
  the wrapper's internal representation to the public API at zero call-site
  cost. Date/Author: 2026-06-18, planning + panel.
- Decision (D5): proceed despite roadmap "Requires 2.1.3" being incomplete.
  Rationale: see Surprises — the implementation does not depend on
  `ReviewThread`; this item independently advances the host-neutral-contract
  goal that 2.1.3 also serves. Date/Author: 2026-06-18, planning.
- Decision (D6): preserve the existing serde derivation of `ReviewView`
  (no `rename_all`); `CommentDetail` continues to serialize as
  `"CommentDetail"`. Rationale: avoid an unnecessary wire-format change; the
  host-neutrality requirement is about *type coupling*, not casing. Note the
  resulting casing asymmetry — the sibling `DiscussionSeverity` uses
  `rename_all = "lowercase"` (`"high"`/`"low"`), so one payload mixes `"high"`
  and `"CommentDetail"` (Telefono 💡). This is deliberate (it preserves the old
  `TuiView` wire form); aligning to `snake_case` is explicitly out of scope. A
  code comment on `label()` will flag that `"detail"` is user-visible in the
  deep link so nobody "tidies" it and breaks the `.feature` assertions.
  Date/Author: 2026-06-18, planning + panel.
- Decision (D7): amend ADR-008 in place (keep `Accepted`, bump `Date`, add a
  dated amendment note) rather than superseding it. Rationale: only the
  link-model portion of ADR-008 changes; the thread-root summary contract is
  unaffected. ADR-010 already records the host-neutral direction. Date/Author:
  2026-06-18, planning.
- Decision (D8): keep the shared `FrankieDeepLink` renderer (per the user's
  "shared helper" decision) but house it in a clearly named presentation
  submodule (`src/ai/pr_discussion_summary/deep_link.rs`) whose module-level
  rustdoc states it is an adapter/presentation projection, NOT part of the
  canonical contract — the canonical shared format is the serde representation
  of `ReviewViewRef`. Rationale: Pandalump 🟡 flagged that re-exporting the
  renderer flat beside the DTO on `frankie::ai::` risks readers treating
  `frankie://…` as canonical (ADR-010's named non-goal) and sits awkwardly with
  the "keep TUI deep-link renderings out of shared review contracts" boundary
  (ADR-010 line 83). Trade-off accepted: the renderer remains re-exported
  through `frankie::ai` for ergonomics (both the CLI and TUI import via
  `crate::ai::{…}`/`frankie::ai::{…}`), so the boundary is expressed through
  module placement + rustdoc rather than an unreachable path. Date/Author:
  2026-06-18, planning + panel.

## Outcomes & retrospective

Implemented the host-neutral summary navigation contract. Summary items now
carry `ReviewViewRef`, `ReviewView` names the logical review destination, and
`FrankieDeepLink` renders the preserved
`frankie://review-comment/<id>?view=detail` token for CLI and TUI output. The
DTO no longer implements URI rendering.

Observable behaviour stayed stable: CLI and TUI behavioural tests still assert
the same rendered deep link, and TUI jump-back navigation still uses the
structured comment ID. The new serialization test proves the shared wire shape
contains `view_ref`, `comment_id`, and `view`, with no TUI-specific field or
deep-link string.

Documentation was updated in ADR-008, the design guide, users guide,
developers guide, and roadmap. CodeRabbit raised two code-review concerns
during M1/M2; both were fixed before proceeding. The final review reported zero
findings.

## Context and orientation

This section assumes no prior knowledge of the repository.

### What the feature does today

The PR-discussion summary feature condenses GitHub review-comment threads into
a grouped, severity-ranked summary shared by the CLI and TUI. Its shared model
lives under `src/ai/pr_discussion_summary/`. Each summary item
(`DiscussionSummaryItem`) currently carries a `tui_link: TuiViewLink`. The
`TuiViewLink` struct holds a `comment_id: GithubCommentId` and a
`view: TuiView` (an enum whose only variant is `CommentDetail`). `TuiViewLink`
implements `std::fmt::Display`, producing
`frankie://review-comment/<id>?view=detail`. The CLI prints this via `Display`;
the TUI renders it in each row and, on a "jump" key, navigates by the link's
`comment_id` (it never parses the string).

### Key terms

- **DTO (data transfer object):** a plain data type passed across a boundary.
  Here, the summary types are driven-port value objects in hexagonal terms.
- **Host-neutral:** free of any one delivery surface's types (no TUI/CLI
  coupling), so library and embedded hosts can consume it directly.
- **Deep link:** the `frankie://…` URI token that identifies a navigation
  target within a Frankie surface.

### Exhaustive change inventory (every site to migrate)

Type definition and field — `src/ai/pr_discussion_summary/model.rs`:

- L78–91: `enum TuiView` + `impl TuiView { label() }` → `ReviewView`.
- L94–111: `struct TuiViewLink` + `impl TuiViewLink { comment_detail() }` →
  `ReviewViewRef`.
- L113–122: `impl fmt::Display for TuiViewLink` (the `frankie://…` format) →
  REMOVE from the DTO; relocate the format into `FrankieDeepLink`.
- L251: field `pub tui_link: TuiViewLink` → `pub view_ref: ReviewViewRef`.
- Rustdoc prose to reword host-neutral (completeness audit 🔴): L77
  `/// TUI view targeted by a summary link.`, L93
  `/// Structured link pointing back to a TUI view.`, L96/L98 field docs, L103
  `/// Creates a link to the comment-detail view…`, L250
  `/// Stable TUI link back to the root discussion.`. Replace
  "TUI"/"link" wording with host-neutral language and add the rustdoc described
  in M1 (intra-doc link to `FrankieDeepLink`, host-neutrality +
  `Display`-removal rationale).
- L261, L282, L284, L285, L316, L330: in-module tests constructing/asserting
  the old types and the `Display` output. Also rename the test function L281
  `tui_link_formats_as_uri_like_token` → e.g.
  `review_view_ref_renders_as_frankie_deep_link` (it is itself a `tui_link`
  token the post-migration `rg` gate would otherwise flag).

Re-exports:

- `src/ai/pr_discussion_summary/mod.rs:10-13`: re-export
  `ReviewView, ReviewViewRef, FrankieDeepLink` (drop `TuiView, TuiViewLink`).
- `src/ai/mod.rs:25-29`: same change in the `pub use pr_discussion_summary::{…}`
  list. (`src/lib.rs` does not re-export these directly; the path is
  `frankie::ai::…`.)

Service (orchestration) — `src/ai/pr_discussion_summary/service.rs`:

- L10: import; L177: `tui_link: TuiViewLink::comment_detail(…)` →
  `view_ref: ReviewViewRef::comment_detail(…)`.
- L340–341: test asserting `item.tui_link.to_string() == "frankie://…"` →
  `FrankieDeepLink(&item.view_ref).to_string()`.

CLI — `src/cli/summarize_discussions.rs`:

- L69: `writeln!(writer, "      Link: {}", item.tui_link)` →
  `…, FrankieDeepLink(&item.view_ref)`.
- L86 import; L106, L127 fixtures; L139 assertion (`Link: frankie://…`,
  unchanged expected text).

TUI state — `src/tui/app/pr_discussion_summary_state.rs`:

- L3 import; L44 `pub fn selected_link(&self) -> Option<&TuiViewLink>` →
  `selected_view_ref(&self) -> Option<&ReviewViewRef>` mapping to
  `&item.view_ref` (L47).
- L165 import; L181, L189 fixtures.
- L203 — THE SOLE COMPILE-BREAKER from dropping `Display` (Telefono 🔴): the
  assertion `state.selected_link().map(ToString::to_string)` relies on
  `TuiViewLink: Display` via the blanket `ToString` impl. After `Display` moves
  to `FrankieDeepLink`, rewrite it to
  `state.selected_view_ref().map(|r| FrankieDeepLink::new(r).to_string())`. No
  other site depends on `TuiViewLink: Display`; every other render site is a
  direct `writeln!`/`format!`/`.to_string()` already listed here.

TUI handler — `src/tui/app/pr_discussion_summary_handlers.rs`:

- L140: `state.selected_link()` → `state.selected_view_ref()`. The body still
  uses `…comment_id.as_u64()` unchanged; rename the local binding `link` →
  `view_ref` (L140, L146, L149). L148 error string "referenced by the summary
  link" is already host-neutral wording; reword to "referenced by the summary
  reference" to match the binding rename (completeness audit 🔴, low stakes).

TUI component — `src/tui/components/pr_discussion_summary.rs`:

- L69: render `FrankieDeepLink(&item.view_ref)` in place of `item.tui_link`.
- L94 import; L110 fixture; L129 assertion (`frankie://…`, unchanged).

Integration / behavioural tests:

- `tests/tui_pr_discussion_summary_bdd.rs:10,64`: import + fixture.
- `tests/pr_discussion_summary_vidaimock.rs:87`:
  `item.tui_link.to_string()` → `FrankieDeepLink(&item.view_ref).to_string()`
  (expected string unchanged).
- `tests/features/pr_discussion_summary.feature` and
  `tests/features/tui_pr_discussion_summary.feature`: NO change — they assert
  on the rendered `frankie://…` output, which is preserved.

Test-support: `src/ai/pr_discussion_summary/test_support.rs` constructs whole
`PrDiscussionSummary` values via the stub service and does NOT name the link
type directly; expected to need no change (verify during M2).

Docs:

- `docs/adr-008-pr-discussion-summary-contract.md` (L71, L77–78): amend the
  model list and rendering description; add a dated amendment note.
- `docs/adr-010-close-review-adapter-capability-gap.md` (L30): already frames
  the gap; add nothing beyond a back-reference if helpful (its decision text is
  already correct).
- `docs/frankie-design.md` (around L3499): already describes "host-neutral
  review references"; verify it names `ReviewViewRef` once the type exists, and
  reconcile any `TuiViewLink` mention.
- `docs/users-guide.md` (L193): the sample CLI output is unchanged; verify and,
  if helpful, add a one-line note that the link is a host-neutral reference
  rendered as a Frankie deep link.
- `docs/developers-guide.md`: add an internal-convention note that shared
  summary/navigation DTOs are host-neutral value objects and that deep-link
  rendering is an adapter concern via `FrankieDeepLink`.
- `docs/roadmap.md` (L229–235): tick 3.3.3 to `[x]` at completion (M4).
- `docs/execplans/3-3-2-summary-generation-for-pr-level-discussions.md`
  (L284, L299–302): historical execplan; leave as-is (it records the state at
  the time) unless a reviewer requests a back-reference.

After migration, `rg -n 'TuiViewLink|TuiView|tui_link'` over `src/` and
`tests/` MUST return zero matches.

## Plan of work

### Stage A — understand and propose (no code changes)

Read this plan, the two ADRs, and the inventory above. Confirm with the user
(approval gate per the `execplans` skill). No edits.

### Stage B — red tests (small, failing first)

Add two tests that specify the *new* contract and currently cannot compile/pass:

1. Host-neutral serialization test (library, in `model.rs` tests or a new
   `tests/pr_discussion_summary_contract.rs`). Build a small
   `PrDiscussionSummary` with one item and `serde_json::to_value(&summary)`.
   Assert the EXACT wire shape of the reference, not just key presence
   (Telefono 🟡): the verified shape is a bare numeric `comment_id`
   (`GithubCommentId` is a one-field tuple struct that serializes
   transparently) and a PascalCase `view` string, so assert that
   `item["view_ref"]` equals
   `serde_json::json!({ "comment_id": 42, "view": "CommentDetail" })`. Keep
   negative substring guards on the serialized string as a supplement: it
   contains none of `"tui_link"`, `"frankie://"`, `"Tui"`. Then assert a
   Deserialize round-trip (Telefono 🟡):
   `serde_json::from_value::<PrDiscussionSummary>(v)` equals the original (the
   structs derive `PartialEq, Eq`). Together these are the acceptance proof
   that serialization is host-neutral and bidirectional. `serde_json` is a
   confirmed runtime dependency (`Cargo.toml` `[dependencies]`), so no manifest
   change is needed.
2. `FrankieDeepLink` rendering test (in the new presentation submodule's
   tests):
   `FrankieDeepLink::new(&ReviewViewRef::comment_detail(42_u64.into())).to_string()`
   equals `"frankie://review-comment/42?view=detail"`.

Run the focused tests and observe failure for the expected reason (missing
types / field). Record the red evidence.

### Stage C — implementation (minimal change to go green)

Milestone M1 — introduce types and migrate consumers (one coherent, compiling
change; the compiler enforces completeness):

1. In `src/ai/pr_discussion_summary/model.rs`, rename `TuiView` → `ReviewView`
   and `TuiViewLink` → `ReviewViewRef`; keep `comment_detail()` and `label()`
   (the latter `pub(crate)`); keep serde derives unchanged (D6); REMOVE the
   `impl fmt::Display for …`. Rename the field `tui_link` → `view_ref`. Reword
   all "TUI link"/"TUI view" rustdoc to host-neutral wording. Add
   compiler-checked rustdoc (Dinolump 🔴): on `ReviewViewRef`, state it is a
   host-neutral reference that deliberately has no `Display`/URI rendering —
   render it with the `FrankieDeepLink` renderer — and cite ADR-010; on
   `ReviewView`, note it is forward-looking with a single `CommentDetail`
   variant today; on `label()`, note `"detail"` is user-visible in the deep
   link (do not "tidy").
2. Add `src/ai/pr_discussion_summary/deep_link.rs` with module-level rustdoc
   declaring it a presentation/adapter projection (NOT the canonical contract;
   the canonical shared format is the serde form of `ReviewViewRef`) per D8.
   Define `pub struct FrankieDeepLink<'a>(&'a ReviewViewRef);` with a private
   field, `pub const fn new(view_ref: &'a ReviewViewRef) -> Self`, and
   `impl fmt::Display` rendering `frankie://review-comment/{id}?view={label}`.
   Rustdoc intra-doc-links back to `ReviewViewRef`. Add a brief comment noting
   the `//` authority form is an authority-less deep link kept for
   compatibility (RFC 7595 §3.2). Declare `mod deep_link;` (unconditional — it
   is used by production CLI/TUI code, so no `#[cfg]` gate) in `mod.rs`.
3. Update re-exports: add `ReviewView, ReviewViewRef, FrankieDeepLink` and drop
   `TuiView, TuiViewLink` in BOTH `mod.rs` and `src/ai/mod.rs` (the
   CLI/TUI/test call sites import via `crate::ai::{…}`/`frankie::ai::{…}`, so
   `FrankieDeepLink` must be reachable through both hops; the CLI render site
   doubles as the cross-crate reachability proof — Pandalump 🟡).
4. Migrate `service.rs`, `summarize_discussions.rs`,
   `pr_discussion_summary_state.rs` (rename `selected_link` →
   `selected_view_ref`, fix the `.map(ToString::to_string)` site),
   `pr_discussion_summary_handlers.rs`, and `pr_discussion_summary.rs` per the
   inventory. Each rendering site uses `FrankieDeepLink::new(&item.view_ref)`.

Milestone M2 — migrate test construction sites and confirm behaviour:

1. Update fixtures/imports in the in-module tests, the BDD test, and the
   VidaiMock test. Do NOT change any expected output string.
2. Run `make test`; expect all prior behavioural tests green plus the two new
   tests from Stage B now passing.

### Stage D — refactor, documentation, cleanup

Milestone M3 — docs (ADR-008 amendment, `frankie-design.md`, `users-guide.md`,
`developers-guide.md`). Milestone M4 — roadmap tick, full gate, CodeRabbit.

Each stage ends with the gate suite; do not proceed past a failing gate.

## Concrete steps

Run from the repository root
`/home/leynos/.lody/repos/github---leynos---frankie/worktrees/12393901-512d-415d-a04f-94cc6a38034d`.

Gates (run sequentially — never in parallel; the build cache benefits from
serial runs). Tee output for review per the agent command conventions:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-frankie-$(git branch --show-current).out
make lint      2>&1 | tee /tmp/lint-frankie-$(git branch --show-current).out
make test      2>&1 | tee /tmp/test-frankie-$(git branch --show-current).out
make markdownlint 2>&1 | tee /tmp/mdlint-frankie-$(git branch --show-current).out
```

Red evidence (Stage B), expected to FAIL before M1:

```bash
cargo nextest run -p frankie pr_discussion_summary 2>&1 | tee /tmp/red-frankie.out
# expect: compilation error (unknown ReviewViewRef / FrankieDeepLink) or the
# new serialization test failing because the field is still `tui_link`.
```

Completeness check after M1/M2:

```bash
rg -n 'TuiViewLink|TuiView|tui_link' src tests benches 2>/dev/null
# expect: no matches.
```

CodeRabbit (after each milestone's deterministic gates are green):

```bash
coderabbit review --agent 2>&1 | tee /tmp/coderabbit-frankie.out
```

## Validation and acceptance

Acceptance, phrased as observable behaviour:

- Running `make test` passes. The new test
  `pr_discussion_summary…serialization_is_host_neutral` fails before M1 (field
  is `tui_link`, type is `TuiViewLink`) and passes after; it asserts the exact
  wire shape `view_ref == { "comment_id": 42, "view": "CommentDetail" }`, the
  absence of `tui_link`/`frankie://`/`Tui` substrings, and a Deserialize
  round-trip. The new test `frankie_deep_link_renders_review_view_ref` likewise
  fails before (type absent) and passes after.
- The CLI behavioural scenario in
  `tests/features/pr_discussion_summary.feature` still asserts
  `stdout contains "Link: frankie://review-comment/1?view=detail"` and passes
  unchanged.
- The TUI scenario in `tests/features/tui_pr_discussion_summary.feature` still
  asserts the summary view contains `frankie://review-comment/1?view=detail`
  and that opening the selected link selects comment id 1; both pass unchanged.
- `rg -n 'TuiViewLink|TuiView|tui_link' src tests benches` returns nothing.

Quality criteria for "done":

- Tests: `make test` green, including the two new tests and all pre-existing
  behavioural tests unchanged.
- Rustdoc (M1 acceptance bullet, Dinolump 🟡): the public types `ReviewView`,
  `ReviewViewRef`, and `FrankieDeepLink` carry rustdoc that (a) states
  host-neutrality, (b) intra-doc-links the DTO to its renderer and back, and
  (c) records the `Display`-removal rationale (ADR-010). `cargo doc` builds
  with no broken intra-doc links.
- Lint/format: `make check-fmt` and `make lint` clean (warnings denied).
- Docs: `make markdownlint` and `make nixie` (Mermaid) clean.
- Review: `coderabbit review --agent` reports no outstanding concerns; run it
  only after the deterministic gates above are green.

## Idempotence and recovery

Every step is a normal source edit under version control. Commit after each
milestone so any step can be rolled back with `git revert`/`git reset`. The
`rg` completeness check is safe to re-run. If a gate fails, fix forward within
the iteration tolerance; if it cannot be made green in 3 attempts, stop and
escalate.

## Artifacts and notes

Target API at the end of M1 — in `src/ai/pr_discussion_summary/model.rs`:

```rust
/// Logical review view a summary reference points at. Host-neutral.
///
/// Forward-looking: only [`ReviewView::CommentDetail`] exists today; further
/// review views will extend this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewView {
    /// Review-list comment-detail view for a selected comment.
    CommentDetail,
}

/// Host-neutral reference from a summary item back to a review view.
///
/// This type is deliberately free of any delivery-surface coupling and has no
/// `Display`/URI rendering: the `frankie://…` deep link is a presentation
/// concern, rendered by [`FrankieDeepLink`]. See ADR-010.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReviewViewRef {
    /// Review comment the reference targets.
    pub comment_id: GithubCommentId,
    /// Logical review view to open for the comment.
    pub view: ReviewView,
}

impl ReviewViewRef {
    #[must_use]
    pub const fn comment_detail(comment_id: GithubCommentId) -> Self {
        Self { comment_id, view: ReviewView::CommentDetail }
    }
}
// No `impl Display` here — rendering lives in `FrankieDeepLink` (see ADR-010).
```

In `src/ai/pr_discussion_summary/deep_link.rs`:

```rust
//! Presentation projection of a host-neutral [`ReviewViewRef`] as a Frankie
//! deep link. This is an adapter/presentation concern, NOT the canonical
//! contract — the canonical shared format is the serde representation of
//! `ReviewViewRef`. Shared by the CLI and TUI surfaces (see ADR-010, D8).

use std::fmt;
use super::model::ReviewViewRef;

/// Presentation wrapper rendering a [`ReviewViewRef`] as a Frankie deep link.
///
/// The `//` authority form is an authority-less deep link kept for
/// compatibility; see RFC 7595 §3.2.
pub struct FrankieDeepLink<'a>(&'a ReviewViewRef);

impl<'a> FrankieDeepLink<'a> {
    #[must_use]
    pub const fn new(view_ref: &'a ReviewViewRef) -> Self {
        Self(view_ref)
    }
}

impl fmt::Display for FrankieDeepLink<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "frankie://review-comment/{}?view={}",
            self.0.comment_id.as_u64(),
            self.0.view.label(),
        )
    }
}
```

## Interfaces and dependencies

Prescriptive end-state names and paths:

- `frankie::ai::ReviewView` — enum, variant `CommentDetail`,
  `pub(crate) const fn label(self) -> &'static str` → `"detail"`.
- `frankie::ai::ReviewViewRef` — struct
  `{ comment_id: GithubCommentId, view: ReviewView }`,
  `pub const fn comment_detail(GithubCommentId) -> Self`, serde
  `Serialize`/`Deserialize`, NO `Display`.
- `frankie::ai::FrankieDeepLink<'a>` — presentation newtype (private field) with
  `pub const fn new(&'a ReviewViewRef) -> Self` and `impl Display` rendering
  the deep link; the single shared renderer used by CLI and TUI. Defined in the
  `deep_link` presentation submodule (D8).
- `frankie::ai::DiscussionSummaryItem.view_ref: ReviewViewRef` (renamed from
  `tui_link`).
- `PrDiscussionSummaryViewState::selected_view_ref(&self) -> Option<&ReviewViewRef>`
  (renamed from `selected_link`).

No new external dependencies. `serde_json` is a confirmed runtime dependency
(`Cargo.toml` `[dependencies]`, verified during the panel review), so the
serialization test needs no manifest change.

## Signposted documentation and skills

Skills: `rust-router` (entry), `rust-types-and-apis` (newtype value object,
public API shape), `arch-crate-design` (module boundaries and re-exports),
`hexagonal-architecture` (keep the DTO a pure driven-port value object;
rendering is an adapter concern), `rust-unit-testing` and the `rstest` /
`rstest-bdd` skills (test shape), `leta` (navigation/rename),
`arch-decision-records` (ADR-008 amendment style), `commit-message` and
`pr-creation` (delivery).

Repository docs to consult: `docs/adr-008-pr-discussion-summary-contract.md`,
`docs/adr-010-close-review-adapter-capability-gap.md`, `docs/frankie-design.md`,
`docs/developers-guide.md`, `docs/documentation-style-guide.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`,
`docs/snapshot-testing-bubbletea-terminal-uis-with-insta.md`,
`docs/two-tier-testing-strategy-for-an-octocrab-github-client.md`,
`docs/building-idiomatic-terminal-uis-with-bubbletea-rs.md`,
`docs/rstest-bdd-users-guide.md`, and `docs/users-guide.md`.

Prior-art notes (from research): RFC 7595 §3.2 flags the `//` authority form as
mildly non-conformant for schemes without a true naming authority — preserved
deliberately here (format unchanged); the `Ref` suffix correctly denotes an
identifying value object without dereference/presentation semantics; the
newtype-`Display`-wrapper is the idiomatic Rust way to render a domain value
without putting `Display` on the domain type.

## Revision note

Initial draft (2026-06-18). Establishes the host-neutral rename (`TuiViewLink`→
`ReviewViewRef`, `TuiView`→`ReviewView`, `tui_link`→`view_ref`), the shared
`FrankieDeepLink` presentation helper, the exhaustive change inventory, the
red-green test strategy, and the documentation/roadmap updates.

Revision 2 (2026-06-18, post community-of-experts panel review). What changed:
added D8 (presentation-submodule placement and the boundary trade-off for
`FrankieDeepLink`); refined D4 (`FrankieDeepLink::new`, private field) and D6
(serde casing asymmetry acknowledged); strengthened the acceptance test to
assert exact JSON plus a Deserialize round-trip and dropped the `serde_json`
dependency hedge; recorded the sole `Display` compile-breaker at
`pr_discussion_summary_state.rs:203`; extended the inventory with the missed
rustdoc prose, the handler error string/binding, and the
`tui_link_formats_as_uri_like_token` test-fn name; and added explicit M1
rustdoc requirements (host-neutrality statement, bidirectional intra-doc links,
`Display`-removal rationale). Why: to close boundary, contract, completeness,
and maintainability gaps the panel surfaced. Effect on remaining work: the edit
list and acceptance criteria are now tighter; no scope change. Awaiting
approval before implementation.
