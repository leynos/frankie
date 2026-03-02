# Architectural decision record (ADR) 005: cross-surface, library-first delivery

## Status

Accepted.

## Date

2026-03-02.

## Context and problem statement

Frankie capabilities must be usable both in the interactive Terminal User
Interface (TUI) and as reusable library functions embedded in a larger
agent-hosting tool. Some workflows also need non-interactive Command Line
Interface (CLI) execution for automation and batch orchestration.

Without an explicit delivery contract, features tend to accumulate TUI-only
business logic, making parity across surfaces hard to maintain and increasing
the cost of testing.

## Decision outcome / proposed direction

Implement feature behaviour in shared `frankie` library modules first, then
expose that behaviour through TUI and CLI adapters. For every roadmap item,
completion requires:

1. A documented, test-covered public library API that external hosts can call
   directly.
2. TUI integration for interactive review workflows.
3. A pure CLI surface for non-interactive workflows, or an explicit documented
   rationale when CLI is not applicable.
4. Explicitly documented exceptions for:
   - User experience (UX)-only TUI polish that does not introduce reusable
     domain behaviour.
   - Documentation-only roadmap or design updates.

## Rationale

1. Capability reuse: external hosts should not need to reimplement TUI-only
   logic to use Frankie features.
2. Operational consistency: shared library behaviour keeps TUI, CLI, and
   embedded automation aligned on validation, error handling, and outcomes.
3. Testability: library-first boundaries reduce UI-coupled tests and make
   deterministic behavioural testing easier.
4. Roadmap durability: surface parity prevents features from becoming
   inaccessible to non-TUI consumers.

## Consequences

- New feature work avoids storing core business logic only in `src/tui/`.
- Existing TUI-centric capabilities (for example reply templating and
  time-travel orchestration) are progressively extracted into shared library
  modules with TUI wrappers.
- Documentation and acceptance criteria treat library, TUI, and CLI support as
  part of feature completion.
- Surface-specific exceptions are allowed only when explicitly documented with
  rationale and out-of-scope boundaries.

## References

- `docs/frankie-design.md` §5.3.6 (ADR index)
- `docs/roadmap.md` (cross-surface delivery contract references)
