# Architectural decision record (ADR) 001: incremental sync for review comments

## Status

Accepted.

## Date

2026-03-02.

## Context and problem statement

The Terminal User Interface (TUI) needs to keep review comments up to date
without losing the user's current selection or requiring a manual refresh.

The implementation must support both background refresh and explicit user
refresh while maintaining a consistent merge strategy and telemetry.

## Decision outcome / proposed direction

Implement timer-based background sync (30-second interval) with ID-based
selection tracking. Manual refresh delegates to the same sync logic.

## Rationale

1. Timer and manual refresh must share the same behaviour. One-shot timers with
   explicit re-arming prevent timer accumulation, and manual refresh can call
   into the same code path.
2. Comments need a stable identity for deterministic merges. `ReviewComment.id`
   is used as the stable identifier for insertion, update, and deletion, and
   results are sorted by ID.
3. Cursor positions are unstable across list mutations. Tracking
   `selected_comment_id` allows selection restoration by locating the new index
   after merge and clamping when the comment was deleted.
4. Observability is a first-class requirement. Sync latency is recorded via the
   `SyncLatencyRecorded` telemetry event, including duration, comment count,
   and whether the sync was incremental.

## Consequences

- Users see fresh data without manual intervention.
- Selection is preserved across syncs unless the selected comment is deleted.
- Latency metrics are available for performance monitoring.

## References

- `docs/frankie-design.md` §5.3.6 (ADR index)
- `src/tui/` (review sync handlers and state)
