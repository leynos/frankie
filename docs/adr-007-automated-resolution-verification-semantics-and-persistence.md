# ADR-007: Automated resolution verification semantics and persistence

## Status

Accepted (2026-03-02): Adopt deterministic diff-replay verification and
SQLite-backed persistence.

## Date

2026-03-02

## Context and Problem Statement

Frankie requires an automated and deterministic way to evaluate whether review
comment concerns have been addressed. The decision must support both the
Command Line Interface (CLI) and Terminal User Interface (TUI), while
preserving explainable outcomes across repeated runs.

## Decision Drivers

- Deterministic behaviour without Large Language Model (LLM) dependence.
- Shared semantics across CLI and TUI.
- Evidence-rich outcomes for auditability and debugging.
- Persisted local cache to avoid unnecessary recomputation.
- Low operational complexity with existing repository/database architecture.

## Requirements

### Functional requirements

- Classify a comment as verified when referenced code is removed or changed.
- Classify a comment as unverified when content is unchanged.
- Classify a comment as unverified when required metadata is missing.
- Expose results and evidence in CLI summaries and TUI annotations.
- Persist verification results for reuse in later runs.

### Technical requirements

- Use deterministic diff replay against a specified target commit.
- Use shared verification service APIs consumed by CLI and TUI adapters.
- Store outcomes in SQLite keyed by `(github_comment_id, target_sha)`.
- Preserve evidence kind and optional evidence message with each result.
- Keep schema and persistence behaviour compatible with migration workflow.

## Options Considered

- **Option A: Deterministic diff replay with SQLite persistence**
  - Replays mappings from comment anchor to target commit state.
  - Stores outcomes and evidence locally for reuse.
- **Option B: Heuristic or AI-assisted verification**
  - Could infer broader intent resolution.
  - Introduces non-determinism and weaker explainability.
- **Option C: No persistence, compute on demand only**
  - Simplifies storage concerns.
  - Increases repeated verification cost and weakens continuity between runs.

## Decision Outcome / Proposed Direction

Adopt **Option A** with a conservative contract:

- A comment is **verified** when the referenced line is **removed** or its
  **content changes** between the comment commit and a target commit (typically
  local `HEAD`).
- A comment is **unverified** when the referenced line appears unchanged, or
  when verification cannot be performed deterministically (missing metadata,
  repository data unavailable, or out-of-bounds line numbers).

Verification behaviour is implemented through shared library APIs:

- `verification::ResolutionVerificationService` and
  `verification::DiffReplayResolutionVerifier` for diff replay and line
  comparison.
- `persistence::ReviewCommentVerificationCache` for local persistence.

Verification results are persisted in SQLite using a dedicated table keyed by
`(github_comment_id, target_sha)` so re-verification overwrites prior outcomes
for the same target commit while preserving evidence for each verdict.

## Goals and Non-Goals

- **Goals**
  - Deterministic and explainable verification outcomes.
  - Consistent semantics between CLI and TUI.
  - Persistent cache reuse across runs and sessions.
  - Evidence fields suitable for display and debugging.
- **Non-Goals**
  - Semantic intent analysis beyond line-level deterministic checks.
  - AI-based confidence scoring.
  - Cross-repository synchronisation of verification cache state.

## Migration Plan

1. Introduce verification domain model and deterministic diff-replay service.
2. Add SQLite persistence table and migration for verification records.
3. Implement persistence adapter with upsert and targeted query operations.
4. Wire CLI `--verify-resolutions` mode to verify and persist results.
5. Wire TUI verification handlers to reuse shared service and cache.
6. Add unit and behavioural tests for changed, unchanged, deleted, and metadata
   failure cases.

## Known Risks and Limitations

- Line-level comparison may miss broader refactors that satisfy intent without a
  direct line change.
- Missing or unavailable repository metadata forces conservative unverified
  results.
- Local cache integrity depends on successful migration and transaction
  handling.
- Verification quality is bounded by available Git history and mapping
  fidelity.

## Architectural Rationale

1. **Determinism**: The “line removed or changed” rule avoids heuristic checks
   that depend on AI interpretation or temporal assumptions.
2. **Conservatism**: When required inputs are absent or repository data cannot
   be inspected reliably, the result remains `unverified` with explicit
   evidence.
3. **Traceability**: Persisted evidence with each verdict enables users to
   understand why a comment was classified as verified or unverified.
4. **Surface parity**: Library-first verification logic keeps CLI and TUI
   behaviour aligned and testable.

## References

- `src/verification/*`
- `src/persistence/review_comment_verification_cache/*`
- `migrations/2026-03-02-000000_review_comment_verifications/*`
