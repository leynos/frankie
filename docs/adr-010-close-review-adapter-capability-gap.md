# ADR-010: Close review adapter capability gap

## Status

Accepted (2026-03-28): Define Frankie as an API-driven GitHub review adapter
and context engine, with host-neutral contracts for thread sync, anchors, time
travel, verification, summary references, and reply actions.

## Date

2026-03-28

## Context and problem statement

Frankie is already positioned as a GitHub pull request adapter with a
library-first delivery model, but parts of the current design and roadmap still
leave an architectural gap between that positioning and the capabilities an
embedding workflow owner needs.

The current public surface exposes raw `ReviewComment` values and some
library-first slices, but important review capabilities remain uneven:

- time-travel extraction is public, while `TimeTravelState` and orchestration
  remain partly TUI-scoped;
- reply drafting exists, but live review submission and queue-safe write
  semantics are not yet part of the shared contract;
- incremental sync is described primarily as TUI merge behaviour rather than a
  host-safe checkpoint and delta protocol;
- summary contracts currently expose `TuiViewLink`, which bakes one delivery
  surface into the shared model;
- review comments are still exposed mostly as flat raw payloads, leaving hosts
  to reconstruct thread and anchor semantics themselves.

At the same time, Frankie’s actual implementation direction is API-driven:
GitHub review intake flows through `octocrab`, historical context comes from
local Git operations via `git2`, and the library is intended to be embedded in
larger hosts. Workflow-owning systems such as Corbusier need Frankie to act as
the review adapter and context engine, while they retain canonical task and
review workflow state.

## Decision drivers

- Clear ownership boundaries between Frankie and workflow-owning hosts.
- Lossless preservation of GitHub review metadata.
- First-class modelling of review threads and actionable anchors.
- On-demand historical context from local Git instead of persisted snapshots.
- Host-neutral shared contracts that do not depend on TUI-specific types.
- Safe operational semantics for review writes, retries, and backoff.

## Requirements

### Functional requirements

- Expose raw review payloads, thread aggregates, and actionable anchors as
  shared library contracts.
- Support incremental synchronization through durable checkpoints and explicit
  deltas.
- Materialize time-travel context, verification inputs, and summary references
  on demand for library, CLI, TUI, and embedded-host consumers.
- Support reply rendering and review-action submission without forcing hosts to
  depend on TUI state or invent their own queue protocol.
- Keep Frankie’s local persistence focused on cache, checkpoints, verification,
  and queued write intents rather than canonical workflow state.

### Technical requirements

- Use API-driven GitHub review access through `octocrab`, not browser
  automation.
- Use local Git context through `git2` for time-travel and verification.
- Keep TUI orchestration types such as `bubbletea_rs::Cmd`, `OnceLock`, and
  TUI deep-link renderings out of shared review contracts.
- Expose durable retry or backoff metadata for review writes when immediate
  submission is not possible.

## Options considered

| Option                                                       | Ownership boundary                                                                  | Shared contract quality                                                       | Operational fit                                                   | Long-term maintenance                       |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------- |
| Option A: Frankie as review adapter and context engine       | Strong: Frankie owns review transport and derived context; host owns workflow state | Strong: thread, anchor, sync, verification, and action contracts are explicit | Strong: queue or backoff metadata can be surfaced once and reused | Strong: aligns with library-first direction |
| Option B: Frankie owns canonical review workflow state       | Weak: duplicates orchestration concerns already handled by hosts                    | Medium: more features in Frankie, but blurred responsibility                  | Medium: harder to integrate with external task governance         | Weak: overlaps with embedding hosts         |
| Option C: Keep the current mixed, partly TUI-centric surface | Weak: ownership remains ambiguous                                                   | Weak: hosts must reconstruct missing semantics                                | Weak: each host reinvents retries, links, and thread models       | Weak: capability drift grows over time      |

_Table: Option comparison for ownership boundary, shared contract quality,
operational fit, and long-term maintenance._

## Decision outcome / proposed direction

Adopt **Option A** with the following contract:

- Frankie owns GitHub review intake, normalization, thread aggregation, anchor
  derivation, time-travel context materialization, automated verification,
  summary preparation, reply rendering, and GitHub review-action transport.
- Embedded workflow owners own canonical task state, review-state projections,
  governance, message linkage, and merge or approval policies.
- Shared review modelling is layered:
  - `ReviewComment` remains the raw, lossless GitHub review payload.
  - `ReviewThread` groups a stable root comment with ordered replies and
    derived thread status.
  - `ReviewAnchor` captures actionable location metadata when the source
    comment includes enough information.
- Incremental intake uses opaque `ReviewSyncCheckpoint` values and explicit
  `ReviewSyncDelta { added, updated, removed, checkpoint }` results.
- Time-travel orchestration is exposed as a host-safe library service that does
  not require TUI command wrappers or global context.
- Reply handling is split into:
  - library-grade rendering over stable data transfer object (DTO) input;
  - review-action submission that can either post immediately or return a
    queued write intent plus retry or backoff metadata.
- Shared summary and navigation contracts use host-neutral review references.
  TUI deep links are rendered from those references as an adapter concern.
- Frankie’s design language and integration assumptions use GitHub API and
  local Git terminology, not browser-automation terminology.

## Goals and non-goals

- **Goals**
  - Make Frankie a cleanly embeddable review adapter for workflow-owning hosts.
  - Preserve GitHub review metadata and expose richer derived review contracts.
  - Finish the library-first extraction of time travel, reply drafting, and
    review actions.
  - Provide one operational contract for retries, rate limits, and queued
    writes.
- **Non-goals**
  - Moving canonical task or review workflow ownership into Frankie.
  - Persisting full historical file snapshots as canonical review state.
  - Making TUI deep links the canonical shared navigation format.

## Migration plan

1. Publish thread-aware review contracts, actionable anchors, and host-safe
   incremental sync checkpoints and deltas.
2. Finish public time-travel extraction by promoting state and orchestration
   into host-safe library services with configurable history limits.
3. Finish reply templating extraction with DTO-based rendering, public default
   templates, and host-neutral summary or navigation references.
4. Add review-action submission with queued write intents, retry metadata, and
   rate-limit backoff hints.
5. Continue reducing library dependence on TUI runtime concerns so embedded
   consumers can use Frankie without pulling in TUI-only behaviour.

## Known risks and limitations

- GitHub review APIs can still omit metadata needed for actionable anchors, so
  some comments will remain raw-only and context must degrade explicitly.
- Thread-root derivation from `in_reply_to_id.unwrap_or(id)` is stable for the
  current adapter shape, but native thread identifiers should replace it when a
  richer upstream API becomes available.
- Queue-safe write submission adds operational surface area, including
  idempotency, persistence, and replay semantics that must remain well tested.

## Architectural rationale

1. **Adapter-first ownership** keeps Frankie aligned with its embeddable
   library direction and avoids duplicating workflow engines in multiple
   repositories.
2. **Layered review contracts** separate raw transport fidelity from actionable
   review semantics.
3. **On-demand context materialization** keeps Frankie reproducible without
   bloating persistence with historical snapshots.
4. **Host-neutral references** prevent TUI navigation choices from leaking into
   every consumer.
5. **Shared operational semantics** for queued writes and backoff reduce
   duplicated retry logic across hosts.

## References

- `docs/adr-001-incremental-sync-for-review-comments.md`
- `docs/adr-004-inline-template-based-reply-drafting.md`
- `docs/adr-005-cross-surface-library-first-delivery.md`
- `docs/adr-008-pr-discussion-summary-contract.md`
- `docs/roadmap.md`
- `docs/frankie-design.md`
- `src/lib.rs`
- `src/github/models/mod.rs`
- `src/time_travel/mod.rs`
- `src/tui/state/time_travel.rs`
