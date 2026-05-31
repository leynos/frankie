# Repository layout

This document explains the major paths in the Frankie repository and the
responsibilities attached to each location. The tree is an orientation sketch,
not an exhaustive file listing.

```plaintext
.
├── .codescene/
├── .github/
├── docs/
│   └── execplans/
├── migrations/
├── src/
│   ├── ai/
│   ├── cli/
│   ├── config/
│   ├── export/
│   ├── github/
│   ├── local/
│   ├── persistence/
│   ├── reply_template/
│   ├── time_travel/
│   ├── tui/
│   └── verification/
└── tests/
    ├── features/
    ├── fixtures/
    ├── snapshots/
    ├── steps/
    └── support/
```

_Figure 1: Major repository paths and documentation, source, and test areas._

| Path                  | Responsibility                                                                                                                |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `.codescene/`         | Stores CodeScene code-health rules for repository analysis.                                                                   |
| `.github/`            | Stores GitHub automation such as Dependabot and workflow configuration.                                                       |
| `docs/`               | Stores long-lived project documentation, guides, design documents, ADRs, and reference material.                              |
| `docs/execplans/`     | Stores living execution plans for non-trivial implementation work.                                                            |
| `migrations/`         | Stores database migration files. Migration names use timestamped directories and should remain append-only after publication. |
| `src/`                | Stores the Rust application and library source code. Feature modules are grouped by domain responsibility.                    |
| `src/ai/`             | Contains AI integration logic, Codex process handling, comment rewriting, and pull request discussion summaries.              |
| `src/cli/`            | Contains command-line interface parsing, command orchestration, export commands, and interactive mode entrypoints.            |
| `src/config/`         | Contains configuration loading, validation, and related tests.                                                                |
| `src/export/`         | Contains export behaviour shared by command-line and application surfaces.                                                    |
| `src/github/`         | Contains GitHub gateway, model, and adapter code.                                                                             |
| `src/local/`          | Contains local repository discovery, commit, Git operation, and remote handling code.                                         |
| `src/persistence/`    | Contains database-backed caches and persistence adapters.                                                                     |
| `src/reply_template/` | Contains reply-template parsing and rendering logic.                                                                          |
| `src/time_travel/`    | Contains time-travel service and state management code.                                                                       |
| `src/tui/`            | Contains terminal user-interface application, components, messages, and state.                                                |
| `src/verification/`   | Contains review resolution verification behaviour.                                                                            |
| `tests/`              | Stores behavioural, integration, snapshot, and support tests.                                                                 |
| `tests/features/`     | Stores feature files for behavioural test scenarios.                                                                          |
| `tests/fixtures/`     | Stores test fixtures, including external service simulation data.                                                             |
| `tests/snapshots/`    | Stores snapshot output owned by `insta` tests.                                                                                |
| `tests/steps/`        | Stores shared behavioural test step definitions.                                                                              |
| `tests/support/`      | Stores shared test support code and helpers.                                                                                  |

_Table 1: Repository path responsibilities._

Generated artefacts should not be committed unless they are deliberate test
fixtures, snapshots, migrations, or reviewed documentation assets. Keep new
domain code close to the feature it supports, and update
[documentation contents](contents.md) when adding, renaming, or removing
long-lived documentation.
