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

- [ADR 0001: Incremental sync for review comments](adrs/adr-0001-incremental-sync-for-review-comments.md)
  records the accepted review comment synchronization approach.
- [ADR 0002: Codex execution stream and transcript model](adrs/adr-0002-codex-execution-stream-and-transcript-model.md)
  records the execution-stream and transcript architecture.
- [ADR 0003: Session resumption for interrupted Codex runs](adrs/adr-0003-session-resumption-for-interrupted-codex-runs.md)
  records session-resumption behaviour.
- [ADR 0004: Inline template-based reply drafting](adrs/adr-0004-inline-template-based-reply-drafting.md)
  records reply drafting through inline templates.
- [ADR 0005: Cross-surface library-first delivery](adrs/adr-0005-cross-surface-library-first-delivery.md)
  records the library-first delivery strategy.
- [ADR 0006: AI rewrite preview and fallback contract](adrs/adr-0006-ai-rewrite-preview-and-fallback-contract.md)
  records rewrite preview and fallback behaviour.
- [ADR 0007: Automated resolution verification semantics and persistence](adrs/adr-0007-automated-resolution-verification-semantics-and-persistence.md)
  records resolution verification semantics and storage.
- [ADR 0008: PR discussion summary contract](adrs/adr-0008-pr-discussion-summary-contract.md)
  records pull request discussion summary contracts.
- [ADR 0009: Review banner translation contract](adrs/adr-0009-review-banner-translation-contract.md)
  records review banner translation contracts.
- [ADR 0010: Close review adapter capability gap](adrs/adr-0010-close-review-adapter-capability-gap.md)
  records the close-review adapter decision.
- [ADR 0011: Adopt the Rust build standard for development builds](adrs/adr-0011-adopt-the-rust-build-standard-for-development-builds.md)
  records why development builds use the fast flags while coverage and the
  release recipe and workflow override them. Direct `cargo build --release`
  retains the configured flags unless `RUSTFLAGS` is assigned.

## Plans

- [Execution plans](execplans/) contains living implementation plans for
  non-trivial work.
