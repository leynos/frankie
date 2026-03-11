# ADR-008: PR discussion summary contract

## Status

Accepted (2026-03-09): Adopt thread-root summarization with shared severity and
TUI-link models across library, CLI, and TUI surfaces.

## Date

2026-03-09

## Context and problem statement

Frankie needs a PR-level discussion summary workflow that condenses review
comment threads into a concise, grouped view without forcing users into a
single surface. The feature must support the Command Line Interface (CLI),
Terminal User Interface (TUI), and library consumers, while keeping grouping,
ordering, and link behaviour deterministic and shared.

## Decision drivers

- Shared, stable library contract for CLI and TUI parity.
- Deterministic grouping by file and severity.
- Traceable links back to concrete TUI comment-detail views.
- Explicit failure for AI-provider schema or configuration problems.
- Low-friction end-to-end testing against an OpenAI-compatible local mock.

## Requirements

### Functional requirements

- Summarize review discussions at the thread-root level rather than per
  comment row.
- Group summary output by file and severity.
- Emit stable links back to TUI comment-detail views.
- Expose the workflow through both a standalone CLI mode and a dedicated TUI
  summary view.

### Technical requirements

- Keep core behaviour in shared library modules under `src/ai/`.
- Use a shared `DiscussionSeverity` taxonomy with `high`, `medium`, and `low`.
- Keep file grouping, severity ordering, and TUI-link construction outside the
  AI provider.
- Validate provider responses against the requested thread-root IDs.
- Cover the real OpenAI-compatible adapter with `vidaimock`.

## Options considered

| Option                                                         | Sharedness                                | Determinism                                                          | Navigation quality                                        | Delivery cost                                        |
| -------------------------------------------------------------- | ----------------------------------------- | -------------------------------------------------------------------- | --------------------------------------------------------- | ---------------------------------------------------- |
| Option A: Thread-root summary with deterministic grouping      | Strong shared library contract            | Strong: file grouping, severity buckets, and links are library-owned | Strong: each item links to a concrete comment-detail view | Moderate, but aligned with cross-surface delivery    |
| Option B: Per-comment AI summary with adapter-local grouping   | Weak: adapters must regroup independently | Lower: grouping and linking can drift per surface                    | Medium: links point to rows, not discussion threads       | Higher long-term maintenance and behavioural drift   |
| Option C: CLI-only prose summary with no structured link model | Weak: no reusable library DTOs            | Medium: text can be deterministic, but not reusable                  | Weak: TUI must re-derive navigation separately            | Lower initial cost, but fails cross-surface contract |

_Table: Option comparison for sharedness, determinism, navigation quality, and
delivery cost._

## Decision outcome / proposed direction

Adopt **Option A** with the following contract:

- Frankie summarizes one **discussion thread** per **thread root**. Replies are
  collapsed into the root thread and do not create duplicate summary items.
- The shared model consists of:
  - `PrDiscussionSummary`
  - `FileDiscussionSummary`
  - `SeverityBucket`
  - `DiscussionSummaryItem`
  - `DiscussionSeverity`
  - `TuiViewLink`
- File grouping and severity ordering are deterministic in library code:
  - file groups sort alphabetically, with `(general discussion)` last;
  - severities sort `high`, then `medium`, then `low`;
  - items sort by root comment ID.
- Links back to the TUI are represented structurally as
  `TuiViewLink { comment_id, view: CommentDetail }` and rendered as
  `frankie://review-comment/<id>?view=detail` when needed.
- AI-provider failures, malformed JSON, invalid severities, unknown thread
  IDs, and missing required fields fail explicitly rather than falling back to
  heuristic prose.

The OpenAI-compatible adapter uses `vidaimock` end-to-end coverage via disk
template overrides under
`tests/fixtures/vidaimock/pr_discussion_summary/templates/openai/`.

## Goals and non-goals

- **Goals**
  - Shared summary data transfer objects (DTOs) consumed by library, CLI, and
    TUI code.
  - Deterministic grouping and navigation semantics.
  - Clear CLI output and direct TUI jump-back behaviour.
  - Real adapter coverage against a local OpenAI-compatible mock.
- **Non-goals**
  - General-purpose deep-link routing across unrelated TUI views.
  - Heuristic fallback summaries when the AI contract fails.
  - Semantic issue tracking beyond review-comment threads.

## Migration plan

1. Add shared PR-discussion summary DTOs, severity taxonomy, and thread
   grouping helpers.
2. Implement summary orchestration plus the OpenAI-compatible adapter.
3. Add standalone CLI `--summarize-discussions` mode.
4. Add TUI summary view plus jump-back navigation into comment detail.
5. Cover library, CLI, TUI, and adapter behaviour with unit, behavioural, and
   `vidaimock` integration tests.

## Known risks and limitations

- Severity assignment still depends on the AI provider, so prompt drift can
  affect prioritization quality even when schema validation passes.
- Thread-root summarization cannot distinguish between semantically unrelated
  replies on the same root; Frankie treats the thread as one unit.
- The TUI link contract intentionally targets only the comment-detail view in
  the current review list, not arbitrary future surfaces.

## Architectural rationale

1. **Thread-level modelling** avoids duplicate summary items for root comments
   and replies.
2. **Library-owned grouping** keeps CLI and TUI output aligned even if the AI
   text differs slightly over time.
3. **Structured links first** let the TUI navigate directly while the CLI can
   still print a stable token.
4. **Explicit failure** is safer than fabricated prose for prioritization
   workflows.
5. **Template-override `vidaimock` coverage** exercises the production HTTP
   adapter without relying on a live provider.

## References

- `src/ai/pr_discussion_summary/*`
- `src/cli/summarize_discussions.rs`
- `src/tui/app/pr_discussion_summary_*`
- `src/tui/components/pr_discussion_summary.rs`
- `tests/pr_discussion_summary_bdd.rs`
- `tests/tui_pr_discussion_summary_bdd.rs`
- `tests/pr_discussion_summary_vidaimock.rs`
