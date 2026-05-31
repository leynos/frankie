# Documentation contents

[Documentation contents](contents.md) is the index for Frankie documentation.

## Project orientation

- [Repository layout](repository-layout.md) explains where source code,
  tests, migrations, configuration, and long-lived documentation belong.
- [Frankie design](frankie-design.md) describes the product architecture,
  boundaries, data flow, and accepted design constraints.
- [Roadmap](roadmap.md) records planned delivery work and implementation
  sequencing.

## Guides

- [Users' guide](users-guide.md) explains user-facing workflows, command-line
  behaviour, configuration, and operational expectations.
- [Developers' guide](developers-guide.md) explains maintainer workflows,
  build and test commands, and internal development conventions.
- [Ortho configuration users' guide](ortho-config-users-guide.md) explains the
  Ortho configuration surface used by Frankie.

## Reference documents

- [Documentation style guide](documentation-style-guide.md) defines writing,
  formatting, and documentation-structure conventions.
- [Review banner translation](review-banner-translation.md) describes review
  banner translation rules and contracts.
- [Scripting standards](scripting-standards.md) defines shell and automation
  conventions for repository scripts.
- [Bubble Tea terminal UI guide](building-idiomatic-terminal-uis-with-bubbletea-rs.md)
  records terminal user-interface implementation guidance.
- [Snapshot testing Bubble Tea terminal UIs with insta](snapshot-testing-bubbletea-terminal-uis-with-insta.md)
  explains snapshot testing conventions for terminal UI output.
- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md)
  explains testability patterns for Rust code that touches external state.
- [Rust doctest dry guide](rust-doctest-dry-guide.md) explains how to avoid
  duplicated Rust documentation examples.
- [Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md)
  explains fixture and parameterization practices.
- [rstest-bdd users' guide](rstest-bdd-users-guide.md) explains behavioural
  test authoring with `rstest-bdd`.
- [Two-tier testing strategy for an Octocrab GitHub client](two-tier-testing-strategy-for-an-octocrab-github-client.md)
  explains GitHub client test layering.
- [Complexity antipatterns and refactoring strategies](complexity-antipatterns-and-refactoring-strategies.md)
  records maintainability risks and refactoring guidance.

## Decision records

- [ADR 001: Incremental sync for review comments](adr-001-incremental-sync-for-review-comments.md)
  records the accepted review comment synchronization approach.
- [ADR 002: Codex execution stream and transcript model](adr-002-codex-execution-stream-and-transcript-model.md)
  records the execution-stream and transcript architecture.
- [ADR 003: Session resumption for interrupted Codex runs](adr-003-session-resumption-for-interrupted-codex-runs.md)
  records session-resumption behaviour.
- [ADR 004: Inline template-based reply drafting](adr-004-inline-template-based-reply-drafting.md)
  records reply drafting through inline templates.
- [ADR 005: Cross-surface library-first delivery](adr-005-cross-surface-library-first-delivery.md)
  records the library-first delivery strategy.
- [ADR 006: AI rewrite preview and fallback contract](adr-006-ai-rewrite-preview-and-fallback-contract.md)
  records rewrite preview and fallback behaviour.
- [ADR 007: Automated resolution verification semantics and persistence](adr-007-automated-resolution-verification-semantics-and-persistence.md)
  records resolution verification semantics and storage.
- [ADR 008: PR discussion summary contract](adr-008-pr-discussion-summary-contract.md)
  records pull request discussion summary contracts.
- [ADR 009: Review banner translation contract](adr-009-review-banner-translation-contract.md)
  records review banner translation contracts.
- [ADR 010: Close review adapter capability gap](adr-010-close-review-adapter-capability-gap.md)
  records the close-review adapter decision.

## Plans

- [Execution plans](execplans/) contains living implementation plans for
  non-trivial work.
