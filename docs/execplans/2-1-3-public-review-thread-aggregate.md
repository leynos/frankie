# Add a public `ReviewThread` aggregate above raw review comments

This execution plan (ExecPlan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Roadmap item 2.1.3 introduces a public `ReviewThread` aggregate above the raw
`ReviewComment` payloads that the GitHub adapter already returns. After this
change, an embedding host (or any library consumer) can iterate review threads
with a stable root identity, ordered replies, and a derived thread status
without importing TUI types and without re-implementing thread reconstruction.
The aggregate also unblocks roadmap items 2.1.4 (host-safe sync contracts) and
2.1.5 (actionable anchors) by establishing the host-neutral container they will
extend.

Success is observable in five ways. First, `frankie::review` exposes a
documented `ReviewThread` value type alongside a pure
`aggregate_review_threads` function (or equivalent) that turns a slice of
`ReviewComment` into a deterministic, ordered `Vec<ReviewThread>` without
panicking on root, reply, or orphaned-reply inputs. Second, the crate
re-exports the new types from `src/lib.rs` so external consumers can write
`use frankie::{ReviewThread, ReviewThreadId, aggregate_review_threads};`
without touching `frankie::tui`, `frankie::ai`, or `frankie::persistence`.
Third, the existing in-process thread builder in
`src/ai/pr_discussion_summary/threads.rs` delegates to the new aggregate so
behaviour stays in one place, while keeping the PR-summary-specific prompt
projection in the AI module. The delegation is gated on a new golden-output
regression test for the existing AI summary so any drift in prompt ordering is
caught before the refactor merges. Fourth, unit, property, and behavioural
tests cover root, reply, and orphaned-reply fixtures, including the
`in_reply_to_id.unwrap_or(id)` fallback explicitly named in the roadmap. Fifth,
`docs/frankie-design.md`, `docs/users-guide.md`, and `docs/developers-guide.md`
record the new public contract, and `docs/roadmap.md` is marked done only after
every validation gate passes.

Resolution status is intentionally *not* part of the 2.1.3 contract. ADR-010
states that `is_resolved` reflects the last known resolution status from sync
data, and Frankie has no sync source today that produces a non-`Unknown` value.
Adding a tri-state `Open`/`Resolved`/`Unknown` field now would let consumers
branch on variants that cannot be reached, and would have to be re-shaped when
2.1.4 wires GitHub GraphQL `PullRequestReviewThread.isResolved` in. Resolution
status lands in 2.1.4 alongside the API integration that gives it meaning. The
2.1.3 contract carries only what 2.1.3 can populate correctly.

This slice is intentionally about a host-neutral aggregate, not new
user-visible workflow. No new standalone command-line interface (CLI) mode is
expected because the aggregation has no non-interactive workflow of its own —
it is a building block consumed by existing CLI subcommands (export, verify,
summarize) once they migrate at their own pace. Per ADR-005, that exception is
documented explicitly here rather than left implicit.

## Relevant documentation and skills

The implementer should keep these repository documents open while working:

- `docs/roadmap.md` for the acceptance criteria of 2.1.3 and the forward
  dependencies in 2.1.4 and 2.1.5.
- `docs/adr-010-close-review-adapter-capability-gap.md` § "Contract invariants"
  (lines 171–248) for the target `ReviewThread` and `ReviewComment` invariants.
- `docs/adr-005-cross-surface-library-first-delivery.md` for the host-neutral
  library-first delivery contract.
- `docs/adr-001-incremental-sync-for-review-comments.md` for how the aggregate
  must remain compatible with future incremental sync semantics.
- `docs/adr-008-pr-discussion-summary-contract.md` for the existing thread-root
  summary contract that already groups by stable root.
- `docs/frankie-design.md` § 6.6.1.3 (lines 4441–4528) for the field-level
  invariant table and the persistence-vs-public boundary, and § 6.6.x review
  adapter responsibilities (lines 3470–3502).
- `docs/users-guide.md` for any library-consumer-facing wording that changes.
- `docs/developers-guide.md` to record the new internal boundary alongside the
  time-travel boundary already documented there.
- `docs/rust-testing-with-rstest-fixtures.md` for the required fixture style.
- `docs/rstest-bdd-users-guide.md` for the behavioural test harness rules.
- `docs/reliable-testing-in-rust-via-dependency-injection.md` and
  `docs/two-tier-testing-strategy-for-an-octocrab-github-client.md` for how to
  keep test boundaries clean when verifying the aggregate against fixtures
  derived from real API payloads.
- `docs/rust-doctest-dry-guide.md` for public API examples.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` to keep the
  aggregation function from accreting unrelated responsibilities.
- `docs/ortho-config-users-guide.md` for the configuration precedent if any
  follow-up CLI bridging is needed (none is expected in this slice).
- `docs/snapshot-testing-bubbletea-terminal-uis-with-insta.md` only if the TUI
  list view ends up rendering threads in this slice (default: no).
- `docs/building-idiomatic-terminal-uis-with-bubbletea-rs.md` for the same
  adapter-boundary expectations applied to any TUI hook-up.

The most relevant skills for the implementation session are `execplans` (this
document stays current), `leta` for semantic code navigation, and `rust-router`
plus the targeted sub-skills `arch-crate-design` (for the new public module
boundary), `rust-types-and-apis` (for the aggregate's public shape),
`rust-errors` (the function is total, but reply ordering edge cases deserve a
typed contract), and `hexagonal-architecture` (the aggregate is the domain core
that adapters in TUI, AI summary, export, and persistence will consume).

## Constraints

- Place the shared aggregation in a new top-level module
  `src/review/` (preferred name: `frankie::review`) so the contract is visible
  outwith both the raw transport (`frankie::github`) and any single consumer
  (`frankie::ai`, `frankie::tui`, `frankie::export`). The module is the entry
  point for forthcoming `ReviewAnchor` (2.1.5) and `ReviewSyncDelta` (2.1.4)
  contracts and should be sized for that growth from the start.
- The public surface must depend only on `frankie::github::ReviewComment` (or
  primitive types) and `std`. It must not import or expose `bubbletea_rs`,
  `tokio`, `minijinja`, `octocrab`, `diesel`, `git2`, or any `frankie::tui::*`,
  `frankie::ai::*`, `frankie::persistence::*`, or `frankie::verification::*`
  types. Hosts must be able to consume the aggregate with only
  `frankie::review` and `frankie::github` in scope.
- `ReviewThread` must encode the thread-root invariants from ADR-010:
  the root is identified by `in_reply_to_id.is_none()` on the earliest ingested
  comment; the stable root key is the root comment's `github_comment_id`; for
  replies (including any nested case the API may surface) the stable root key
  equals the top-most root's `github_comment_id`, not the immediate parent.
- The aggregation function must implement the roadmap's named fallback
  `in_reply_to_id.unwrap_or(id)` as the stable thread root key. The
  *implementation* walks the parent chain to a fix-point, stopping when either
  `in_reply_to_id` is `None` or the parent is absent from the input slice. For
  REST-sourced data — where GitHub flattens replies to one level — the walk
  terminates after at most one hop, so the literal roadmap shortcut and the
  fix-point walk return the same result. Keeping the walk transitive preserves
  correctness if the same aggregate is later fed nested-reply data (for example
  via GraphQL `PullRequestReviewThread` in 2.1.4) and matches the pre-existing
  logic in `src/ai/pr_discussion_summary/threads.rs` that the AI summary
  delegation must remain identical to. The membership filter on the parent also
  handles orphan replies (a reply whose root is not present in the input
  slice): such a reply becomes its own degenerate single-comment thread keyed
  by its own `id`.
- Replies must be ordered by `(created_at, id)` within the thread, matching the
  existing `src/ai/pr_discussion_summary/threads.rs` ordering. The aggregate
  must be deterministic for any permutation of the input slice.
- Threads themselves must be returned in a documented, stable order. The
  default order is `(earliest created_at across the thread, root id)` so that
  threads sort the same way the underlying conversations were created and so
  that two calls on the same input always return the same `Vec`.
- The aggregate is host-neutral. `ReviewThread` must not carry
  `TuiViewLink`, `frankie://…` deep links, or any `Cmd`-shaped callbacks. A
  later ADR-008 follow-up (3.3.3 / `ReviewViewRef`) will introduce a structural
  reference type; nothing in this slice should pre-empt that decision.
- Resolution status is out of scope for 2.1.3. Do not add a
  `ReviewThreadStatus` enum, an `is_resolved` field, or any other resolution
  carrier to `ReviewThread` in this slice. The ADR-010 invariant "`is_resolved`
  reflects the last known resolution status from sync data" is satisfied by
  *omitting* the field while sync data is unavailable. Resolution lands in
  2.1.4 alongside GraphQL `PullRequestReviewThread` intake; adding it here
  would advertise unreachable variants and force a re-shape later.
- Raw `ReviewComment` payloads must be preserved losslessly inside the
  aggregate. Store the original `ReviewComment` values verbatim and expose them
  through accessor methods rather than public fields, so the aggregate can
  enforce its own invariants (non-empty `comments`, root at position 0) and so
  the underlying storage representation stays an implementation detail. Do not
  flatten fields away during aggregation.
- The new module must not break any existing public re-exports. The existing
  re-exports listed in `src/lib.rs` continue to work unchanged.
- Every new Rust module begins with a `//!` module-level comment.
- All new public items carry Rustdoc comments with at least one compiling
  doctest where the example is not a tautology over assumed properties.
- Unit tests must use `rstest`. Behavioural tests must use `rstest-bdd` v0.5.0
  with `#[scenario(path = …)]`. Behavioural helpers returning `Result` must
  return explicit errors rather than panicking to satisfy
  `clippy::panic_in_result_fn`.
- Property tests must use `proptest`. At least one property covers
  permutation-invariance of the aggregate output for a randomly generated
  bundle of root and reply comments. A `kani` harness is not justified at this
  slice because the input is unbounded and the property is permutation, not a
  fixed-size state space — record this decision in the Decision Log.
- Documentation updates must use en-GB-oxendict spelling and follow
  `docs/documentation-style-guide.md`.
- Do not mark roadmap item 2.1.3 as done until code, tests, docs, and all
  validation gates have passed.
- Frankie's library remains a single crate. No workspace split, no new
  features, no new `Cargo.toml` dependencies. If `proptest` is not already in
  `[dev-dependencies]` (it is — `proptest = "1.10.0"`), the only acceptable
  change is to use it; introducing any other dependency is out of scope and
  must escalate.

## Tolerances (exception triggers)

- Scope: if the implementation needs more than 16 files or more than 900 net
  new lines (including tests and docs), stop and escalate with a narrower
  decomposition. The aggregate itself, its conversion functions, and tests
  should comfortably fit well below this.
- Interface: if implementation appears to require changing the public shape of
  `ReviewComment`, `ReviewCommentGateway`, or any other type currently
  re-exported from `src/lib.rs`, stop and escalate. Adding fields to public
  contracts in this slice would silently pre-empt 2.1.4 and 2.1.5.
- Semantics: if the aggregation cannot satisfy both the roadmap shortcut
  (`in_reply_to_id.unwrap_or(id)`) and the ADR-010 invariant (top-most root for
  nested replies) without choosing one over the other, stop and document the
  competing contract options before proceeding. (Expectation: the two are
  identical for REST-sourced data; the shortcut is the operational form.)
- Dependencies: if a new external dependency is required, stop and escalate.
- Iterations: if `make lint` or `make test` still fails after three focused
  fix cycles, stop and escalate with logs and the current diff.
- Validation: if `make fmt`, `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, or `make test` fail after three focused fix
  cycles, stop and escalate.
- Refactor reach: if extracting the new aggregate from the existing
  `build_discussion_threads` function in
  `src/ai/pr_discussion_summary/threads.rs` requires changing the prompt
  contract or any other AI-summary-visible behaviour, stop and escalate. The AI
  module should keep its own prompt projection on top of the new shared
  aggregate.
- Coupling: if the only way to keep the AI summary, export, and TUI consumers
  compiling is to widen the public surface beyond `ReviewThread`,
  `ReviewThreadId`, and the aggregation entry point, stop and escalate before
  adding to the public API.

## Risks

- Risk: the AI summary module already implements an in-process thread builder
  with subtly different ordering (`(created_at.as_deref(), id)` with `&str`
  comparison). The new shared aggregate must match it byte-for-byte to allow a
  no-op delegation, or prompt outputs may shift in ways the existing tests do
  not detect. Severity: medium. Likelihood: medium. Mitigation: before Stage B,
  lock in a golden-output regression test (see Stage A.5) for
  `build_discussion_threads` over a fixture that exercises the risky edges —
  `None` timestamps mixed with `Some`, identical `created_at` across two
  comments, a reply pre-dating its root, and a nested-style reply chain. Only
  then lift the existing logic verbatim into the new module and have
  `build_discussion_threads` call the new aggregate, decorated with
  prompt-specific projections.
- Risk: a thread keyed `N` today may be re-keyed `M` on a later sync.
  Concretely, a reply with `in_reply_to_id: Some(999)` is an orphan today (root
  keyed `7`), and tomorrow's sync brings in comment `999` so the same
  conversation becomes one thread keyed `999`. Downstream caches keyed on
  `ReviewThreadId` (TUI selection state, summary caches, verification
  shortcuts) will silently re-key. Severity: medium. Likelihood: medium.
  Mitigation: document the re-rooting hazard prominently in the public Rustdoc
  for `aggregate_review_threads` and in the developers-guide section. Flag it
  as a constraint that 2.1.4 must address explicitly when it introduces
  `ReviewSyncDelta`.
- Risk: the public `ReviewThread` shape might accidentally bake in fields that
  belong to 2.1.5 (`ReviewAnchor`) or 2.1.4 (`ReviewSyncDelta`, resolution
  status). Severity: medium. Likelihood: medium. Mitigation: keep
  `ReviewThread` minimal — stable root id, root comment, ordered replies — and
  resist adding anchor, sync, or status hooks even when they "obviously fit".
  Record forward-compatible extension points in the Decision Log.
- Risk: existing TUI list and detail rendering paths in
  `src/tui/components/review_list.rs` consume a flat `Vec<ReviewComment>`.
  Switching them to threads in this slice would balloon scope. Severity:
  medium. Likelihood: low. Mitigation: do not change TUI rendering in this
  slice; expose the aggregate as a read-only helper and leave TUI migration for
  a later roadmap item.
- Risk: persistence-layer code in `src/persistence/` may grow a thread cache as
  per the design doc's `ReviewThreadProjectionRow`. Severity: low. Likelihood:
  low. Mitigation: persistence changes are explicitly out of scope; document
  that the new aggregate is computed in memory from the existing
  `ReviewCommentGateway` output. A future migration may add a persistence
  projection without touching the public contract.
- Risk: doctest examples for `aggregate_review_threads` accidentally rely on
  ordering invariants that are not yet guaranteed. Severity: low. Likelihood:
  medium. Mitigation: write doctests that assert deterministic outputs over a
  fixed three-comment fixture (one root, one reply, one orphan) so the contract
  is observable.
- Risk: `ReviewComment` carries `created_at: Option<String>` rather than a
  typed timestamp, so ordering across `None` and `Some` is brittle. Severity:
  low. Likelihood: medium. Mitigation: order by
  `(comment.created_at.as_deref(), comment.id)` exactly as
  `pr_discussion_summary::threads` already does, so comments without a
  timestamp sort before timestamped ones consistently, and ties break on numeric
  `id`.
- Risk: en-GB-oxendict spelling slips when copying code or comments from the
  existing AI summary module. Severity: low. Likelihood: medium. Mitigation: run
  `make markdownlint` and review prose for `-ize` / `-yse` / `-our` spelling
  before each commit.

## Progress

- [ ] (YYYY-MM-DD HH:MMZ) Read `docs/roadmap.md`,
      `docs/adr-010-close-review-adapter-capability-gap.md`,
      `docs/adr-005-cross-surface-library-first-delivery.md`,
      `docs/adr-001-incremental-sync-for-review-comments.md`,
      `docs/adr-008-pr-discussion-summary-contract.md`, and
      `docs/frankie-design.md` § 6.6.1.3.
- [ ] (YYYY-MM-DD HH:MMZ) Confirm the current baseline: thread reconstruction
      lives only in `src/ai/pr_discussion_summary/threads.rs`; no
      `frankie::review` module exists yet; persistence has no `review_threads`
      table.
- [ ] (YYYY-MM-DD HH:MMZ) Draft this ExecPlan for roadmap item 2.1.3 and
      obtain user approval.
- [ ] Stage A: introduce the `frankie::review` module with `ReviewThread`,
      `ReviewThreadId`, and the pure `aggregate_review_threads` function. No
      consumers wired yet.
- [ ] Stage A.5: lock a golden-output regression test for the existing
      `build_discussion_threads` over a tricky-ordering fixture before any
      delegation. Verify the test fails when the function is intentionally
      broken (e.g. swap two comments) and passes on `main`.
- [ ] Stage B: replace the inner workings of
      `src/ai/pr_discussion_summary/threads.rs` with a thin adapter over the
      new aggregate. The Stage A.5 golden test must stay green across the
      refactor.
- [ ] Stage C: cover the new public API with `rstest` unit cases, including a
      `proptest` permutation-invariance property, and add a behavioural
      suite under `tests/review_thread_aggregate_bdd.rs` plus
      `tests/features/review_thread_aggregate.feature`.
- [ ] Stage D: update `docs/frankie-design.md` § 6.6.1.3 to point at the now
      live public type, add a `## Review thread aggregate` section to
      `docs/users-guide.md`, document the internal boundary in
      `docs/developers-guide.md`, and mark item 2.1.3 done in
      `docs/roadmap.md`.
- [ ] Stage E: run the full validation suite (`make fmt`, `make markdownlint`,
      `make nixie`, `make check-fmt`, `make lint`, `make test`) and
      `coderabbit review --agent` for the milestone closeout.

## Surprises & discoveries

- (To be filled in as work proceeds.)

## Decision log

- Decision (2026-06-02): place the new aggregate in a top-level
  `frankie::review` module rather than extending `frankie::github`. Rationale:
  `frankie::github` owns the raw transport types from the GitHub API;
  `ReviewThread` is a derived host-facing contract that will grow to include
  `ReviewAnchor` and `ReviewSyncDelta` per ADR-010. Co-locating the three under
  `frankie::review` keeps the public boundary coherent and avoids re-shaping
  `frankie::github` for every new contract.
- Decision (2026-06-02): implement the stable root key as a fix-point walk
  of the parent chain, filtered by membership in the input slice (i.e. the same
  loop already used in `src/ai/pr_discussion_summary/threads.rs`), rather than
  the literal roadmap shortcut `in_reply_to_id.unwrap_or(id)`. Rationale: the
  literal form would point orphan replies (whose root is not present in the
  input slice) at a missing parent rather than at themselves, breaking the
  orphaned-reply acceptance test; the literal form would also silently degrade
  if the same aggregate were later fed nested-reply data from GraphQL
  `PullRequestReviewThread` in 2.1.4. The fix-point walk preserves correctness
  in both cases, terminates in one hop for REST-sourced data, and matches the
  AI summary code that the delegation in Stage B must remain identical to.
- Decision (2026-06-02): omit any resolution-status field from `ReviewThread`
  in 2.1.3. Rationale: every value Frankie could populate today would be
  "unknown / open"; advertising `Open`/`Resolved` variants would let consumers
  branch on unreachable states and force a re-shape when 2.1.4 introduces
  GraphQL `PullRequestReviewThread.isResolved`. Adding the field in 2.1.4
  alongside the API integration that gives it meaning is cheaper than
  retrofitting consumers around a stub today.
- Decision (2026-06-02): keep `ReviewThread`'s storage private and expose
  conversation access through methods (`root()`, `replies()`, `comments()`,
  `root_id()`). Rationale: invariants such as "`comments` is non-empty and
  starts with the root" cannot be enforced by public fields. Methods also let
  the implementation switch to `Arc<[ReviewComment]>` or a borrowed view later
  without a SemVer break.
- Decision (2026-06-02): define `ReviewThreadId` as a fully opaque newtype
  in `frankie::review` with no public `u64` constructor and no `as_u64()`
  accessor. Rationale: ADR-010 distinguishes between "stable thread key" and
  "GitHub comment id" even though the two share the same `u64` today; letting
  consumers round-trip through `u64` would let that distinction rot silently.
  The aggregate is the only producer; consumers compare ids by equality, hash
  them for caches via `Hash`, and render them for logs via `Display`. Reusing
  `crate::verification::GithubCommentId` was considered and rejected because it
  would couple `frankie::review` to `frankie::verification` for what is a
  coincidental representation, and because the two concepts will diverge once
  GraphQL Node IDs land.
- Decision (2026-06-02): no new `Cargo.toml` dependencies. `proptest` is
  already in `[dev-dependencies]`. Rationale: the slice does not need any new
  runtime crate, and adding one in a contract-shaping slice would conflate
  separate concerns.
- Decision (2026-06-02): defer all persistence changes
  (`review_threads` table, `ReviewThreadProjectionRow`). Rationale: ADR-010
  treats persistence projections as implementation details behind the public
  contract; a memory-only aggregate over the existing gateway output is
  sufficient to satisfy 2.1.3. A persistence projection can land later without
  touching `ReviewThread`.
- Decision (2026-06-02): no `kani` harness. Rationale: the aggregate's
  invariants are over unbounded multisets of comments, which is the natural fit
  for `proptest`-style permutation properties rather than bounded exhaustive
  search. `kani`'s value would be marginal compared to the cost of bounding the
  model.
- Decision (2026-06-02): no standalone CLI subcommand. Rationale: this slice
  is a library aggregate consumed by existing CLI commands at their own pace;
  ADR-005 only requires CLI surfaces for features with their own
  non-interactive workflow. Document this exception in
  `docs/developers-guide.md`.
- Decision (2026-06-02): keep `DiscussionThread` and
  `DiscussionThreadComment` in `src/ai/pr_discussion_summary/threads.rs` as
  prompt-specific projections built *over* the new aggregate. Rationale: these
  types carry prompt concerns (`related_comment_ids`, normalized body fallback,
  verification status injection) that do not belong in the host-neutral
  `ReviewThread`. Lift the threading logic, not the projection.
- Decision (2026-06-02): reduce the `rstest-bdd` suite to one host-neutral
  boundary scenario, and move the remaining acceptance cases into parameterised
  `rstest` unit cases. Rationale: the BDD layer's load-bearing job here is to
  *prove the host-neutral boundary* — a feature file that imports only
  `frankie::review` and `frankie::github` is structural evidence that the
  contract works without TUI, AI, or persistence. Adding more scenarios for
  variations the unit tests already cover would duplicate coverage without
  strengthening the boundary witness.

## Outcomes & retrospective

- (To be filled in at completion.)

## Context and orientation

This section names every file the implementer must read or touch, so a reader
new to the codebase can find the work without prior context.

### Current raw transport

`src/github/models/mod.rs` defines `ReviewComment` (lines 47–75). The struct
carries `id: u64`, `body: Option<String>`, `author: Option<String>`,
`file_path: Option<String>`, `line_number: Option<u32>`,
`original_line_number: Option<u32>`, `diff_hunk: Option<String>`,
`commit_sha: Option<String>`, `in_reply_to_id: Option<u64>`,
`created_at: Option<String>`, and `updated_at: Option<String>`. The field names
do not yet match the eventual ADR-010 names (`github_comment_id`,
`thread_root_github_comment_id`, `reviewer_id`); per ADR-010, treat `id` as the
equivalent of `github_comment_id` and `author` as the equivalent of
`reviewer_id` for the duration of this slice. Do not rename the existing fields
here — that is a separate migration tracked by ADR-010's own "Current
deviations from the target contract" list.

`src/github/gateway/review_comments/mod.rs` fetches and deserializes review
comments and returns `Vec<ReviewComment>`. It does not order them or attach
thread information.

`src/github/models/test_support.rs` provides `minimal_review(id, body, author)`,
`review_with_id(id)`, `review_with_different_id(base, new_id)`, and
`create_reviews(count)`. These fixtures are the right starting point for
aggregate tests.

### Current thread reconstruction (to be reused)

`src/ai/pr_discussion_summary/threads.rs` builds `DiscussionThread` values from
`PrDiscussionSummaryRequest::review_comments()`. The relevant logic spans lines
40–101 and is reproduced here so the implementer can compare it against the new
aggregate when extracting:

```rust
let comments_by_id: BTreeMap<u64, &ReviewComment> = request
    .review_comments()
    .iter()
    .map(|comment| (comment.id, comment))
    .collect();

let mut grouped_ids: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
for comment in request.review_comments() {
    let mut root_id = comment.id;
    while let Some(parent_id) = comments_by_id
        .get(&root_id)
        .and_then(|current| current.in_reply_to_id)
        .filter(|parent_id| comments_by_id.contains_key(parent_id))
    {
        root_id = parent_id;
    }
    grouped_ids.entry(root_id).or_default().push(comment.id);
}
```

Replies sort by `(created_at.as_deref(), id)` (line 72). The PR-summary code
also injects `verification_status` per comment and a fall-back
`(general discussion)` file path. Those projections belong to the AI module and
must not leak into the host-neutral `ReviewThread`.

### Current persistence

Migrations under `migrations/2025-12-14-000000_initial_schema/up.sql` define
`review_comments` with no `thread_root_github_comment_id` column and no
`review_threads` table. The verification cache
(`migrations/2026-03-02-000000_review_comment_verifications/up.sql`) is keyed by
`(github_comment_id, target_sha)` and is unaffected by this slice.

Frankie does not currently project thread topology into SQLite. This slice
keeps it that way: the aggregate is computed in memory.

### Current consumers of `ReviewComment`

The exhaustive map (from the codebase survey):

- AI: `src/ai/pr_discussion_summary/{model,service,threads}.rs`,
  `src/ai/comment_rewrite/model.rs`.
- Gateway: `src/github/gateway/review_comments/mod.rs`,
  `src/github/gateway/mod.rs`.
- TUI: `src/tui/components/review_list.rs`,
  `src/tui/components/comment_detail.rs`, `src/tui/storage.rs`,
  `src/tui/app/{mod,navigation,verification_handlers,verification_state}.rs`,
  `src/tui/state/{filter_state,diff_context}.rs`.
- Verification: `src/verification/service.rs`.
- CLI:
  `src/cli/{review_tui,export_comments,verify_resolutions,summarize_discussions}.rs`.
- Export: `src/export/{model,ordering}.rs`.
- Reply template: `src/reply_template/mod.rs`.
- Time travel: `src/time_travel/mod.rs`.

Only the AI summary module will switch to the new aggregate in this slice. The
other consumers stay on the flat `Vec<ReviewComment>` for now; future roadmap
items may migrate them on their own timelines.

### Current public surface

`src/lib.rs` already re-exports `ReviewComment`, `ReviewCommentGateway`,
`OctocrabReviewCommentGateway`, and friends from `frankie::github`, and exposes
`frankie::reply_template`, `frankie::time_travel`, `frankie::verification`,
`frankie::export`, and `frankie::config`. The new `frankie::review` module
slots in as a peer.

### Recommended target shape

`src/review/mod.rs` exposes:

- `pub struct ReviewThread { /* private fields */ }` with accessor methods
  `root_id() -> ReviewThreadId`, `root() -> &ReviewComment`,
  `comments() -> &[ReviewComment]`, and
  `replies() -> impl Iterator<Item = &ReviewComment>`.
- `pub struct ReviewThreadId { /* private */ }` — a fully opaque newtype
  with no public `u64` constructor and no `as_u64()` accessor. Derives `Debug`,
  `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, and `Ord`.
  Implements `Display` for log rendering. Only the aggregate produces instances.
- `pub fn aggregate_review_threads(comments: &[ReviewComment]) -> Vec<ReviewThread>`

`ReviewThread` invariants enforced by construction: `comments()` is non-empty,
position 0 is the root (`in_reply_to_id` is `None` or points outside the
slice), and replies (positions 1..) are ordered by `(created_at, id)`.
`ReviewThread` implements `Debug`, `Clone`, `PartialEq`, and `Eq`. Resolution
status is deliberately absent (see Decision Log).

The exact names may be adjusted during implementation, but they must live in
`frankie::review` and must not require any TUI, AI, persistence, or networking
import to use.

## Plan of work

### Stage A: introduce the public aggregate

Create `src/review/mod.rs` with a `//!` module comment and the public types
listed in the recommended target shape. Re-export the new types from
`src/lib.rs` so callers can write
`use frankie::{ReviewThread, ReviewThreadId, aggregate_review_threads};`.

Implement `aggregate_review_threads` as a pure function. The function builds a
`BTreeMap<u64, &ReviewComment>` index over the input, computes the stable root
key for each comment by walking the parent chain to a fix-point — stopping when
`in_reply_to_id` is `None` or the parent is absent from the index — groups
references by root, sorts each group by
`(comment.created_at.as_deref(), comment.id)`, and yields `ReviewThread` values
in `(earliest_created_at, root_id)` order. The function clones underlying
`ReviewComment` values into the aggregate so the return type owns its data
(Frankie's `ReviewComment` is small and already `Clone`). `ReviewThread`
instances are constructed only through a crate-private constructor that
enforces the non-empty, root-first invariant; the public accessors guarantee
callers can never observe a violated invariant.

Document the public types with Rustdoc and at least three compiling doctests: a
root-only thread, a root with one reply, and an orphan reply. Each doctest
should assert deterministic outputs. The Rustdoc for `aggregate_review_threads`
must call out the orphan re-rooting hazard explicitly (a thread keyed `N` today
may be keyed `M` after a future sync brings the missing parent in).

Stage A finishes with the new module compiling, the re-exports landing, and no
behaviour change in consumers because none has been wired yet.

### Stage A.5: lock golden output for the existing AI summary

Before any delegation, add a regression test in
`src/ai/pr_discussion_summary/threads.rs` (or alongside it) that captures the
current `build_discussion_threads` output for a deliberately tricky fixture: at
least one root, at least two replies with skewed `created_at` values (one
missing), one reply whose `created_at` pre-dates its root (simulating clock
skew), and one nested-style reply chain (comment A is reply to B is reply to C,
even though REST does not produce such chains today; the existing transitive
walk must reduce it to root C). Assert the full `Vec<DiscussionThread>` shape —
root id, ordered `related_comment_ids`, file path, and per-comment
author/body/created_at — as a structured equality check (not a snapshot, so
failure messages are diff-able).

The Stage A.5 test must:

1. Pass on the current code before any change in this slice.
2. Fail when an unrelated micro-edit is made to the comparison key inside
   `build_discussion_threads` (sanity check: swap `created_at` and `id` in the
   sort tuple and confirm the test goes red, then revert).
3. Remain in the suite after Stage B as the load-bearing regression gate.

If step 2 does not fail the test, the fixture is not tricky enough and the
implementer must broaden it before proceeding to Stage B.

### Stage B: route the AI summary through the new aggregate

In `src/ai/pr_discussion_summary/threads.rs`, replace the inner loop with a
call to `crate::review::aggregate_review_threads(request.review_comments())`.
The AI module then iterates the resulting `Vec<ReviewThread>` and builds its
existing `DiscussionThread` projections from each one (root comment, normalized
body, verification status, related comment IDs, general-discussion file
fallback).

Keep the file path under 400 lines. Move the prompt-specific helpers
(`normalized_body`, the `GENERAL_DISCUSSION_FILE_PATH` fallback, and
verification injection) into the same file or a focused submodule, but do not
promote them to the public surface.

The existing unit tests in `src/ai/pr_discussion_summary/threads.rs` (lines
117–238) and the Stage A.5 golden-output regression test must both continue to
assert the same observable behaviour. Update their imports if necessary but do
not change their assertions.

Stage B finishes with the AI summary still emitting identical
`DiscussionThread` sequences for the same input, now built on top of the shared
aggregate, with both the legacy unit cases and the Stage A.5 golden regression
test green.

### Stage C: extend test coverage

Add `rstest` unit tests under `src/review/tests.rs` (or `src/review/mod.rs`'s
`#[cfg(test)] mod tests`) covering:

- A single root comment with no replies returns one thread whose `root_id`
  equals the comment's id.
- A root with two replies returns one thread, comments ordered by
  `(created_at, id)`, with the root in position 0.
- An orphan reply (a reply whose `in_reply_to_id` is not in the input slice)
  becomes its own single-comment thread, keyed by its own id, not by the
  missing parent id.
- A nested-style reply chain (A replies to B replies to C, all present in
  the slice) collapses to one thread keyed `C`. This guards the transitive root
  walk and prevents regression to a one-hop shortcut.
- Two independent threads in mixed insertion order return two threads, each
  internally ordered, with the outer sequence ordered by earliest `created_at`.
- Two roots with identical `created_at` break ties on `id`.
- A reply whose `created_at` is `None` sorts before timestamped replies,
  ties broken on `id`.
- Constructing an empty thread is impossible: `aggregate_review_threads(&[])`
  returns an empty `Vec`, and there is no public way to build a `ReviewThread`
  with zero comments.

Add a `proptest` property at the bottom of the same test module: for any
randomly generated bundle of comments where each non-root reply points at a
randomly chosen root from the same bundle, the aggregate is invariant under
input permutation — that is, sorting the input by any deterministic key yields
the same `Vec<ReviewThread>` (compared by sequence equality) as processing the
input in its original order. Use `proptest::collection::vec` with a bounded
size (e.g. up to 16 comments) and `proptest::strategy::Strategy` for the
timestamp choices to keep the search space tractable.

Add one behavioural scenario in `tests/review_thread_aggregate_bdd.rs` with a
companion `tests/features/review_thread_aggregate.feature`. The single
scenario's job is to prove the host-neutral boundary structurally: importing
only `frankie::review` and `frankie::github` (plus test fixtures), it walks a
small mixed input — one root with one reply and one orphan reply — and asserts
both thread count and root id. The duplicated coverage from earlier drafts of
this plan moves into the `rstest` unit list above; the BDD layer is the
boundary witness, not a second coverage tier.

The behavioural file must not import `frankie::tui`, `frankie::ai`,
`frankie::persistence`, or `frankie::verification`. If it can, the
host-neutrality constraint is broken regardless of test outcome.

### Stage D: update documentation and close the roadmap item

Update `docs/frankie-design.md` § 6.6.1.3 to note that `ReviewThread` is now a
live public type in `frankie::review`, and add or extend the "Persistence-layer
structs vs public API contracts" table accordingly. Cross-reference the new
module from the review adapter responsibilities table (§ 6.6.x) so the boundary
diagram reflects reality.

Add a `## Review thread aggregate` section to `docs/users-guide.md` under the
library API area, including a short code example using
`use frankie::{ReviewThread, aggregate_review_threads};`. The section must
state that 2.1.3 deliberately omits resolution status, and that resolution
status arrives in 2.1.4 alongside the GraphQL `PullRequestReviewThread`
integration that gives it a non-ambiguous value. The section must also warn
consumers about the orphan re-rooting hazard so embedders do not silently
corrupt caches keyed on `ReviewThreadId` across syncs.

Add a `## Review thread aggregate boundary` subsection to
`docs/developers-guide.md`, modelled on the existing "Time-travel service
boundary" subsection. State that `frankie::review` is the canonical home for
host-neutral review aggregates and that `frankie::ai::pr_discussion_summary`,
`frankie::tui`, and any future persistence projection consume rather than own
these types. Document the deliberate absence of a standalone CLI mode.

Mark roadmap item 2.1.3 as done in `docs/roadmap.md` only after every
validation gate has passed.

### Stage E: close out with CodeRabbit review

After Stage D's documentation lands and all `make` gates pass, request a
`coderabbit review --agent` pass over the branch. Resolve every concern before
marking the milestone complete. If CodeRabbit surfaces a concern that would be
caught by `make check-fmt`, `make lint`, or `make test`, treat that as a
process failure — re-run the deterministic gates first and fix the underlying
issue in code, not by silencing the lint.

## Concrete steps

The following commands are the exact gates the implementer must run. Each is
shown with `tee` to a per-action log file under `/tmp` per the project's
shared-cache convention.

```bash
set -o pipefail
make fmt 2>&1 | tee /tmp/2-1-3-fmt-frankie-$(git branch --show-current).out
```

```bash
set -o pipefail
make markdownlint 2>&1 | tee /tmp/2-1-3-markdownlint-frankie-$(git branch --show-current).out
```

```bash
set -o pipefail
make nixie 2>&1 | tee /tmp/2-1-3-nixie-frankie-$(git branch --show-current).out
```

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/2-1-3-check-fmt-frankie-$(git branch --show-current).out
```

```bash
set -o pipefail
make lint 2>&1 | tee /tmp/2-1-3-lint-frankie-$(git branch --show-current).out
```

```bash
set -o pipefail
make test 2>&1 | tee /tmp/2-1-3-test-frankie-$(git branch --show-current).out
```

```bash
coderabbit review --agent 2>&1 | tee /tmp/2-1-3-coderabbit-frankie-$(git branch --show-current).out
```

## Validation and acceptance

Acceptance is met when:

- `cargo doc --no-deps` and `make lint` pass with the new module's Rustdoc
  links resolving, and the doctests for `aggregate_review_threads` run and pass.
- `make test` passes with the new unit, property, and behavioural suites
  green. The behavioural suite name is `tests/review_thread_aggregate_bdd.rs`
  and the feature file is `tests/features/review_thread_aggregate.feature`.
- A new Rust source file `src/review/mod.rs` exists, begins with a `//!`
  module comment, and exports `ReviewThread`, `ReviewThreadId`, and
  `aggregate_review_threads` publicly. `src/lib.rs` re-exports each name at the
  crate root. `ReviewThreadStatus` is **not** present in this slice.
- The existing AI summary tests in `src/ai/pr_discussion_summary/threads.rs`
  still pass without modification of their assertions.
- `docs/frankie-design.md`, `docs/users-guide.md`, and
  `docs/developers-guide.md` are updated as described in Stage D.
- `docs/roadmap.md` item 2.1.3 is marked done.
- `coderabbit review --agent` reports no remaining concerns.

Quality criteria:

- Tests: new aggregate behaviour fully covered by unit, property, and
  behavioural tests; existing AI summary behaviour unchanged.
- Lint and typecheck: `make check-fmt`, `make lint`, and `cargo doc --no-deps`
  succeed without warnings (the `RUSTFLAGS="-D warnings"` policy already
  promotes any warning to a failure).
- Performance: not a perf-critical path. Aggregation is `O(N log N)` over the
  input slice for sorting; no benchmark gate is required at this slice.
- Security: no new dependency, no new surface that handles credentials.

Quality method: the deterministic gates above plus a
`coderabbit review --agent` pass.

## Idempotence and recovery

All steps are idempotent. Re-running `aggregate_review_threads` over the same
input produces the same `Vec<ReviewThread>`. Re-running the test gates is a
no-op. There is no persistence change in this slice, so no migration needs to
be rolled back.

If implementation needs to abort mid-Stage, the safe rollback is to revert the
unmerged commits with `git restore --source=HEAD~1 -- <paths>` and re-run
`make test`. Do not amend published commits.

## Artifacts and notes

Once implementation begins, capture for the Outcomes section:

- The final public API signatures of `ReviewThread`, `ReviewThreadStatus`,
  `ReviewThreadId`, and `aggregate_review_threads` exactly as shipped.
- A short transcript of the behavioural test run for one happy and one
  unhappy scenario.
- A short transcript showing the AI summary test suite still green after
  Stage B.

## Interfaces and dependencies

In `src/review/mod.rs`, define:

```rust
/// Stable identity of a review thread.
///
/// `ReviewThreadId` is intentionally opaque: it has no public `u64`
/// constructor and no `u64` accessor. The only way to obtain one is from
/// `aggregate_review_threads`, which guarantees the id corresponds to a
/// real root comment in the input slice. Consumers compare ids by
/// equality, hash them into caches via [`std::hash::Hash`], and render
/// them through [`std::fmt::Display`].
///
/// Today the underlying value equals the GitHub comment id of the
/// thread-root comment. That representation is an implementation detail.
/// When GitHub GraphQL `PullRequestReviewThread.id` is integrated in a
/// later roadmap item, a separate `ReviewThreadNodeId` type will be added
/// for the synthetic Node id; `ReviewThreadId` will continue to mean
/// "stable key for the thread root comment".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReviewThreadId(u64);

impl std::fmt::Display for ReviewThreadId { /* ... */ }

/// Host-neutral aggregate of a review conversation.
///
/// Fields are private. Construct through [`aggregate_review_threads`].
/// Access through the methods below; the aggregate guarantees that
/// `comments()` is non-empty, that the root is at position 0, and that
/// replies (positions 1..) are ordered by `(created_at, id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewThread {
    root_id: ReviewThreadId,
    comments: Vec<ReviewComment>,
}

impl ReviewThread {
    /// The thread's stable identity.
    #[must_use]
    pub const fn root_id(&self) -> ReviewThreadId { self.root_id }

    /// The root comment (always present).
    #[must_use]
    pub fn root(&self) -> &ReviewComment { /* ... */ }

    /// The full ordered conversation, root first.
    #[must_use]
    pub fn comments(&self) -> &[ReviewComment] { &self.comments }

    /// Replies only, in `(created_at, id)` order.
    pub fn replies(&self) -> impl Iterator<Item = &ReviewComment> { /* ... */ }
}

/// Build deterministic review-thread aggregates from a slice of comments.
///
/// The stable thread root key is computed by walking the `in_reply_to_id`
/// chain to a fix-point, stopping when `in_reply_to_id` is `None` or the
/// parent is not in `comments`. For REST-sourced data the walk terminates
/// after at most one hop, equivalent to the roadmap's named shortcut
/// `in_reply_to_id.unwrap_or(id)`. Orphan replies whose root is not
/// present in `comments` become their own degenerate single-comment
/// threads.
///
/// # Orphan re-rooting hazard
///
/// A thread keyed `N` today may be keyed `M` after a future sync brings
/// in the missing parent. Embedders that cache by [`ReviewThreadId`]
/// across syncs must be prepared for the id to change for the same
/// conversation. The 2.1.4 `ReviewSyncDelta` contract will surface this
/// explicitly; until then, treat any persisted [`ReviewThreadId`] as a
/// snapshot of "the thread as last observed".
#[must_use]
pub fn aggregate_review_threads(comments: &[ReviewComment]) -> Vec<ReviewThread>;
```

In `src/lib.rs`, add `pub mod review;` and re-export `ReviewThread`,
`ReviewThreadId`, and `aggregate_review_threads` alongside the existing
re-exports. The exact lines are added next to the existing
`pub use reply_template::{...}` block. `ReviewThreadStatus` is not re-exported
because it is not introduced in this slice.

No `Cargo.toml` changes. No new external dependencies. Existing
`proptest = "1.10.0"` and `rstest = "0.22.0"` cover the test side.

## Revision note

- (2026-06-02) Initial draft of the ExecPlan for roadmap item 2.1.3.
  No prior version exists. Awaiting user approval before implementation begins.
- (2026-06-02) Revised after a Logisphere multi-expert design review.
  Changes, in priority order:
  - Removed `ReviewThreadStatus` from the 2.1.3 contract. Resolution status
    is deferred to 2.1.4 alongside GitHub GraphQL
    `PullRequestReviewThread.isResolved` integration. Affected sections:
    Purpose, Constraints, Recommended target shape, Stage A, Interfaces
    and dependencies, Acceptance criteria, Decision Log.
  - Specified the root-key implementation as a fix-point parent walk (with
    membership filtering) rather than a one-hop shortcut, to preserve
    correctness under nested replies that GraphQL data may eventually
    surface. Affected sections: Constraints, Decision Log, Plan of work
    Stage A, Interfaces and dependencies.
  - Added Stage A.5: a golden-output regression test for the existing
    `build_discussion_threads`, locked before any delegation in Stage B,
    so subtle ordering shifts in the AI summary surface deterministically.
    Affected sections: Progress, Plan of work, Risks (medium-likelihood
    risk on AI summary drift).
  - Made `ReviewThreadId` fully opaque (no `as_u64`, no `new(u64)`
    constructor) so callers cannot round-trip through `u64` and the
    representation can diverge from "GitHub comment id" later without a
    SemVer break. Affected sections: Decision Log, Recommended target
    shape, Interfaces and dependencies.
  - Privatised `ReviewThread`'s fields and added accessor methods so the
    aggregate's invariants (non-empty conversation, root in position 0)
    are enforced by construction. Affected sections: Constraints,
    Recommended target shape, Interfaces and dependencies.
  - Reduced the `rstest-bdd` suite from four scenarios to one host-neutral
    boundary witness; moved the dropped scenarios into the `rstest` unit
    list (which already covered the same ground). Affected sections: Plan
    of work Stage C, Decision Log.
  - Added an orphan-re-rooting hazard call-out to the public Rustdoc, the
    users-guide section, and the Risks list. Affected sections: Risks,
    Stage A, Interfaces and dependencies, Stage D.
