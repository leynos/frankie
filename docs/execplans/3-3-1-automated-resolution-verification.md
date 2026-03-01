# Implement automated resolution verification (diff replay + conditions)

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Add an automated verification step that checks whether a review comment has
been resolved by replaying repository diffs and evaluating comment-specific
conditions. After this change, a user can run verification and see each comment
annotated as `verified` or `unverified`. Verification results persist locally
so subsequent sessions can show prior verification state without re-running the
check.

This step must ship as shared library behaviour with both text-based user
interface (TUI) and command-line interface (CLI) access paths.

Success is observable when:

- Running verification annotates each review comment as verified/unverified.
- Verification results persist in the local cache and are reloaded on the next
  run (for both CLI and TUI).
- The library exposes reusable APIs so verification logic is not duplicated in
  adapters.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.5.0) cover happy
  paths, unhappy paths, and edge cases.
- `docs/frankie-design.md` records the design decisions for verification
  semantics and persistence.
- `docs/users-guide.md` documents the new behaviour and the user-facing
  interface (CLI flags and TUI key bindings).
- The roadmap entry in `docs/roadmap.md` is marked done only after all
  acceptance gates pass.
- `make check-fmt`, `make lint`, and `make test` all pass.

## Constraints

- Keep core verification logic in shared library modules (new module under
  `src/`); TUI and CLI must remain orchestration/presentation adapters.
- Preserve TUI MVU separation:
  - input mapping in `src/tui/input.rs`
  - message definitions in `src/tui/messages.rs`
  - state transitions in `src/tui/app/`
  - render queries in `src/tui/components/` and `src/tui/app/rendering.rs`
- Avoid external network calls in verification logic and tests.
- Verification must be deterministic given a repository state, comment payload,
  and configuration; tests must not depend on wall-clock time or environment
  mutation.
- Use dependency injection for git operations and persistence so tests can use
  mocks/stubs (see `docs/reliable-testing-in-rust-via-dependency-injection.md`).
- Every new Rust module must start with a `//!` module-level comment.
- New public APIs must have Rustdoc comments, and non-trivial functions should
  include short usage examples.
- No single source file may exceed 400 lines; split modules/handlers/tests as
  needed.
- Behavioural tests must use `rstest-bdd` v0.5.0 with feature files under
  `tests/features/`.
- Cache persistence must use the existing SQLite database/migration
  infrastructure under `src/persistence/` and `migrations/`.
- Do not mark `docs/roadmap.md` step 3.3.1 as done until all acceptance gates
  succeed.

## Tolerances (exception triggers)

- Semantics: if the definition of `verified` cannot be made precise enough to
  test deterministically (without AI), stop and escalate with options.
- Scope: if implementation requires touching more than 35 files or more than
  2,500 net new lines, stop and escalate with a staged alternative.
- Persistence: if implementing persistence requires adopting Diesel schema code
  generation beyond current patterns in `src/persistence/`, stop and escalate.
- UX: if the proposed TUI annotation meaningfully degrades list readability on
  narrow terminals (≤ 80 columns) and cannot be fixed without a larger layout
  redesign, stop and escalate.
- Iterations: if any stage fails validation after three fix cycles, stop and
  escalate with logs.

## Risks

- Risk: verification heuristics produce false positives/negatives that confuse
  users. Severity: high. Likelihood: medium. Mitigation: explicitly document
  what "verified" means, prefer conservative classification, and persist
  "evidence" alongside the verdict so users can understand why.
- Risk: repositories missing comment commit SHAs (force pushes, deleted
  commits) cause verification to fail noisily. Severity: medium. Likelihood:
  medium. Mitigation: treat missing commit data as `unverified` with an
  actionable explanation and store the failure detail for later inspection.
- Risk: persistence schema becomes awkward if stored at the wrong granularity
  (per PR vs per comment vs per commit). Severity: medium. Likelihood: medium.
  Mitigation: key verification results by GitHub comment ID and target commit
  SHA, and allow re-verification to overwrite entries.
- Risk: TUI async verification introduces stale updates (results arriving after
  selection changes). Severity: medium. Likelihood: medium. Mitigation: include
  request IDs in messages (pattern used by AI rewrite preview flow).

## Progress

- [ ] (2026-03-01 00:00Z) Draft ExecPlan created.
- [ ] Stage A: Define verification semantics and library API.
- [ ] Stage B: Implement git diff replay + condition evaluation.
- [ ] Stage C: Persist verification results in SQLite cache.
- [ ] Stage D: Add CLI access path for verification.
- [ ] Stage E: Add TUI access path and annotations.
- [ ] Stage F: Add unit + behavioural tests (happy/unhappy/edge).
- [ ] Stage G: Update docs, mark roadmap done, pass gates.

## Surprises & discoveries

- None recorded yet.

## Decision log

- Decision (pending): define a minimal, deterministic meaning for
  "verified/unverified" that can be derived from repository diffs plus comment
  fields (`file_path`, `commit_sha`, `line_number`, `diff_hunk`). Rationale:
  acceptance requires verification without relying on AI calls. Date/Author:
  pending / plan author.
- Decision (pending): persist verification status in a dedicated SQLite table
  keyed by GitHub comment ID and the verification target SHA. Rationale:
  avoids requiring full review-comment persistence before this step and keeps
  cache reads simple for adapters. Date/Author: pending / plan author.

## Outcomes & retrospective

- Pending implementation.

## Context and orientation

Relevant existing code and patterns:

- `src/github/models/mod.rs` defines `ReviewComment` including `diff_hunk` and
  `commit_sha` fields needed to anchor verification inputs.
- `src/local/git_ops/` provides the `GitOperations` trait and a `git2`-backed
  implementation (`Git2Operations`) used for time-travel and line mapping.
- `src/local/commit.rs` contains `LineMappingRequest` and
  `LineMappingVerification`, which already "replay diffs" to map a comment line
  from an old commit to a new commit.
- `src/persistence/` currently provides a SQLite-backed PR metadata cache with
  explicit migrations and "thin wrapper around SQL" patterns, plus unit tests.
- TUI uses MVU message handlers and DI patterns to keep async logic testable
  (see `src/tui/app/time_travel_handlers/` and `src/tui/app/reply_draft_handlers.rs`).

Terminology used in this plan:

- Verification: computing a verdict for a comment at a specific repository
  target SHA (typically `HEAD`) using diff replay and conditions.
- Verdict: `verified` or `unverified`.
- Evidence: a short, user-facing explanation of why a comment was classified as
  verified/unverified (for example, "line deleted" or "insufficient comment
  metadata").
- Cache: SQLite persistence storing the latest known verdict for a comment at a
  target SHA.

Open questions that must be resolved (and recorded in `docs/frankie-design.md`)
before implementation completes:

1. What exact semantics should "verified" represent?
   - Option A (conservative): verified means "the commented line content no
     longer matches what was commented on" (changed or deleted).
   - Option B (workflow-based): verified means "comment has been replied to
     and the code around it has changed".
   - Option C (explicit conditions): verified means "a machine-readable
     condition set is satisfied" (requires defining a condition language and
     how to attach conditions to comments).
2. Should verification run per selected comment, per current filter set, or
   always for all comments?
3. How should verification behave when `database_url` is not configured?
   (require it, or default to an XDG state-path database).

This plan proceeds assuming Option A (conservative, deterministic) as the
baseline. If the project requires Option C, treat that as a tolerance breach
and escalate because it expands scope significantly.

## Plan of work

### Stage A: Define verification semantics and library API

Goal: establish a small, reusable library surface that both CLI and TUI can
call to verify comments, and a clear definition of "verified/unverified" that
is observable and testable.

1. Add a new shared library module, for example `src/verification/`:
   - `src/verification/mod.rs` (public exports)
   - `src/verification/model.rs` (domain types)
   - `src/verification/service.rs` (service trait + default implementation)
2. Define domain types:
   - `VerificationTarget` (typically `{ repo_path, head_sha }`)
   - `CommentVerificationStatus` (`Verified`, `Unverified`)
   - `CommentVerificationEvidence` (short enum/struct describing why)
   - `CommentVerificationResult` (status + evidence + target SHA + timestamp)
   - `CommentVerificationRequest` (comment + target SHA + options)
3. Define a trait for the core verifier (DI-friendly), for example:
   - `ResolutionVerificationService::verify_comment(&self, ...)`
   - `ResolutionVerificationService::verify_comments(&self, ...)`
4. Ensure the verifier depends on existing `GitOperations` (or a small adapter
   trait built on it) rather than calling `git2` directly.

Go/no-go for Stage A:

- Go when a testable meaning for `verified` exists and can be represented as
  pure data (status + evidence) without UI coupling.
- No-go if meaning cannot be stated without involving AI interpretation or
  external services; escalate and propose an explicit condition language.

### Stage B: Implement git diff replay + condition evaluation

Goal: compute verification verdicts deterministically by comparing repository
state between the comment commit and the target commit.

Baseline algorithm (Option A):

1. Input: `ReviewComment` plus target SHA (default `HEAD`).
2. Preconditions:
   - `comment.commit_sha`, `comment.file_path`, and at least one of
     `comment.line_number` / `comment.original_line_number` must be present.
   - If preconditions are missing, return `Unverified` with evidence
     `InsufficientMetadata`.
3. Use `GitOperations::verify_line_mapping` with:
   - `old_sha = comment.commit_sha`
   - `new_sha = target_sha`
   - `file_path = comment.file_path`
   - `line = comment.original_line_number.unwrap_or(comment.line_number)`
4. If line mapping indicates `Deleted` or `NotFound`, classify as `Verified`
   with evidence `LineRemoved`.
5. Otherwise, fetch file content for both commits:
   - `GitOperations::get_file_at_commit(old_sha, file_path)`
   - `GitOperations::get_file_at_commit(new_sha, file_path)`
6. Compare the old line content to the new line content at the mapped position:
   - If the line content differs (after normalising line endings), classify as
     `Verified` with evidence `LineChanged`.
   - If the line content is identical, classify as `Unverified` with evidence
     `LineUnchanged`.
7. If fetching file content fails (missing commit, missing file), classify as
   `Unverified` with evidence `RepositoryDataUnavailable { message }`.

Note: `diff_hunk` is not required for the baseline algorithm, but can be used
to improve evidence and reduce false positives by comparing a small context
window around the line. If `diff_hunk` is used, keep parsing logic in the
library and cover edge cases (no prefixes, malformed hunks, windows crossing
file bounds).

### Stage C: Persist verification results in SQLite cache

Goal: store and retrieve the latest verification result so adapters can
annotate comments without recomputation.

1. Add a migration that introduces a dedicated cache table, for example:
   `review_comment_verifications` with columns:
   - `github_comment_id` (unique key)
   - `target_sha` (text, the commit SHA verified against)
   - `status` (text: `verified` / `unverified`)
   - `evidence_kind` (text)
   - `evidence_message` (text, nullable)
   - `verified_at_unix` (integer)
   - `updated_at` (timestamp default current timestamp)
2. Add a persistence wrapper under `src/persistence/`, following the
   `pr_metadata_cache` patterns:
   - `get_for_comments(&[u64], target_sha)` returning a map
   - `upsert_result(github_comment_id, result)`
3. Decide how `database_url` is sourced:
   - Preferred: require `database_url` for verification features and surface a
     configuration error when missing.
   - Alternative: introduce a default database path under XDG state (requires
     doc updates and careful backward-compatibility analysis).
4. Provide unit tests using a temporary SQLite database:
   - schema missing returns a typed error
   - upsert overwrites older results for same comment/target
   - round-trip preserves status and evidence

### Stage D: Add CLI access path

Goal: enable non-interactive verification from the command line.

1. Add new config fields and mode selection in `src/config/mod.rs`, for
   example:
   - `--verify-resolutions` (boolean) selecting a new operation mode.
2. Add a new `OperationMode` variant, for example `VerifyResolutions`, and
   dispatch in `src/main.rs`.
3. Implement `src/cli/verify_resolutions.rs`:
   - Load PR comments (same gateway as export/TUI).
   - Determine repository path (reuse existing local discovery rules).
   - Run verification via the library service.
   - Persist results and print a summary, including counts and a per-comment
     line like:
     - `✓ verified 123456 (src/lib.rs:42) Line changed`
     - `✗ unverified 123457 (src/lib.rs:42) Line unchanged`
4. Add unit tests for mode selection and output formatting using existing CLI
   test utilities.

Behavioural tests (Tier 2) for CLI:

- Use `rstest-bdd` with a temporary git repository and minimal commits so the
  verifier can map and compare lines deterministically.
- Scenarios should include:
  - Verified when line changes between commits.
  - Verified when line is deleted.
  - Unverified when line is unchanged.
  - Unverified when commit SHA is missing or not found.
  - Cache persists and a second run loads cached annotations.

### Stage E: Add TUI access path and annotations

Goal: allow interactive verification and show status in review list/detail.

1. Add a key binding in `src/tui/input.rs` for verification:
   - `v`: verify selected comment
   - `V`: verify all comments in current filter (optional, but useful)
2. Add new messages in `src/tui/messages.rs`, for example:
   - `AppMsg::VerifyCommentRequested { request_id, comment_id }`
   - `AppMsg::VerifyCommentReady { request_id, comment_id, result }`
   - `AppMsg::VerifyAllReady { request_id, results }`
3. Add handlers in `src/tui/app/` that:
   - spawn verification via async command (using injected verifier/persistence)
   - ignore stale responses using request IDs (pattern from AI rewrite)
   - update in-memory comment annotations from cache results
4. Rendering:
   - In the review list, append a small status marker (for example `[V]` or
     `✓/✗`) that does not break narrow layouts.
   - In the comment detail pane, show the evidence summary when present.
5. Add snapshot tests for the TUI rendering (see
   `docs/snapshot-testing-bubbletea-terminal-uis-with-insta.md`), covering:
   - no verification state
   - verified marker
   - unverified marker
   - narrow terminal width behaviour

### Stage F: Unit + behavioural tests

Goal: achieve deterministic coverage for library logic, persistence, and both
adapters.

Unit tests (`rstest`):

- Library:
  - missing metadata returns `Unverified(InsufficientMetadata)`
  - deleted line maps to `Verified(LineRemoved)`
  - changed line maps to `Verified(LineChanged)`
  - unchanged line maps to `Unverified(LineUnchanged)`
  - failures from GitOperations map to `Unverified(RepositoryDataUnavailable)`
- Persistence:
  - upsert + get round trips
  - schema missing errors are clear and typed
- CLI/TUI:
  - mode-selection logic (operation mode precedence)
  - output formatting (CLI)
  - state update tests for message handlers (TUI)

Behavioural tests (`rstest-bdd` v0.5.0):

- CLI scenario feature file, for example:
  `tests/features/verify_resolutions.feature`
- TUI scenario feature file, for example:
  `tests/features/tui_verify_resolutions.feature`

Follow the patterns described in:

- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rstest-bdd-users-guide.md`
- `docs/two-tier-testing-strategy-for-an-octocrab-github-client.md`

### Stage G: Documentation and completion gates

Goal: ensure the feature is documented, decisions are captured, and the repo
passes all required gates.

1. Update `docs/frankie-design.md`:
   - Add `ADR-007` describing:
     - the chosen semantics for verified/unverified (and why)
     - persistence schema and rationale
     - adapter responsibilities and DI approach
2. Update `docs/users-guide.md`:
   - Document CLI usage and example output.
   - Document TUI key bindings and where verification state is displayed.
   - Document the requirement for a local git repository and any database
     configuration (`--database-url`) needed for persistence.
3. Mark roadmap entry 3.3.1 as done in `docs/roadmap.md` only once all tests
   and gates pass.
4. Run and record validation:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/frankie-check-fmt.log
make lint 2>&1 | tee /tmp/frankie-lint.log
make test 2>&1 | tee /tmp/frankie-test.log
```

