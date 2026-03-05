# ADR-007: Automated resolution verification semantics and persistence

## Status

Accepted.

## Date

2026-03-02.

## Context

Frankie requires an automated, deterministic approach for verifying whether a
review comment has likely been addressed. Roadmap requirements specify that
verification must:

- Replay diffs and evaluate comment conditions.
- Annotate comments as verified or unverified in both the Command Line
  Interface (CLI) and Terminal User Interface (TUI).
- Persist results locally so subsequent sessions can reuse the latest
  verification status.

## Decision

Adopt a conservative, deterministic verification contract:

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

## Rationale

1. **Determinism**: The “line removed or changed” rule avoids heuristic checks
   that depend on AI interpretation or temporal assumptions.
2. **Conservatism**: When required inputs are absent or repository data cannot
   be inspected reliably, the result remains `unverified` with explicit
   evidence.
3. **Traceability**: Persisted evidence with each verdict enables users to
   understand why a comment was classified as verified or unverified.
4. **Surface parity**: Library-first verification logic keeps CLI and TUI
   behaviour aligned and testable.

## Consequences

- CLI provides a `--verify-resolutions` mode that verifies comments and exits.
- TUI provides `v` (verify selected) and `V` (verify filtered) shortcuts, with
  verified and unverified markers in review-list and detail views.
- Verification requires both a local repository and a migrated SQLite database
  (`--database-url` with `--migrate-db` completed at least once).

## References

- `src/verification/*`
- `src/persistence/review_comment_verification_cache/*`
- `migrations/2026-03-02-000000_review_comment_verifications/*`
