# Frankie goes to code review roadmap

This roadmap translates the design in `docs/frankie-design.md` into phased,
measurable delivery slices. It focuses on outcomes, avoids time commitments,
and keeps tasks small enough to complete within a few weeks. Completion
criteria emphasize observable behaviours and tests rather than intent.

## Scope and principles

- Anchor delivery on GitHub pull request workflows, local-first execution, and
  AI-assisted resolution, as defined in the design.
- Sequence work to reduce risk: establish data and access layers before
  experience and automation layers.
- Keep each task independently testable, avoiding hidden coupling between
  phases.
- Build capabilities library-first, so core behaviour is reusable in embedded
  agent hosts, with Terminal User Interface (TUI) and Command Line Interface
  (CLI) surfaces acting as adapters.

## Cross-surface delivery contract

See `docs/frankie-design.md` §ADR-005.

- For every roadmap item that is still unchecked, completion requires a stable
  library API in `frankie` and an interactive TUI integration when the feature
  involves in-session review workflows.
- Where a workflow can run non-interactively, completion also requires a pure
  CLI surface (flag, subcommand, or command mode), or an explicit documented
  rationale for why CLI is not applicable.
- New features must keep behavioural logic in shared library modules; TUI and
  CLI layers must remain thin orchestration and presentation adapters.
- Acceptance and regression testing for unchecked items must verify library
  behaviour and each exposed user surface.

## 1. Access and data foundations

Establish reliable repository intake, authentication, and local persistence to
support later workflow features.

### 1.1. GitHub and repository intake

- [x] 1.1.1. Provide PR URL intake using octocrab with token validation and
  explicit error surfacing; acceptance: opening a valid PR URL loads metadata
  and comments, invalid tokens return a user-readable failure message, and
  integration tests cover success and auth error paths.
- [x] 1.1.2. Implement command-line parsing with `ortho-config` so flags,
  environment variables, and config files share a single schema; acceptance:
  CLI help documents all options, precedence order is tested, and defaults
  mirror config file values.
- [x] 1.1.3. Implement owner/repository discovery with paginated PR listing;
  acceptance: repository selection lists at least 50 PRs with pagination
  controls and asserts rate-limit headers are handled without panics.
- [x] 1.1.4. Add local repository discovery via git2 that maps remotes to
  GitHub origins; acceptance: running in a Git repo pre-fills owner/repo
  details and warns when remote origin is missing.

### 1.2. Persistence and configuration

- [x] 1.2.1. Define initial SQLite schema (repositories, pull requests,
  comments, sync checkpoints) using Diesel migrations; acceptance:
  `diesel migration run` succeeds and schema version is recorded in app
  telemetry.
- [x] 1.2.2. Implement local caching layer with coherent expiry policy;
  acceptance: cached PR metadata is reused across sessions and invalidates on
  ETag or Last-Modified changes detected via octocrab responses.
- [x] 1.2.3. Introduce configuration loading via ortho-config with environment
  and file sources; acceptance: configuration precedence is unit-tested and
  misconfiguration produces actionable error messages.

## 2. Review navigation and context

Deliver the core TUI experience for navigating reviews and understanding code
context.

### 2.1. Review listing and filtering

- [x] 2.1.1. Build review listing view with filters (all, unresolved, by file,
  by reviewer, by commit range); acceptance: filters execute without full
  reloads, retain cursor position, and are covered by state update tests.
- [x] 2.1.2. Implement incremental sync to keep review lists up to date;
  acceptance: background refresh merges new comments without losing selection
  state and logs sync latency metrics locally.

### 2.2. Contextual comment exploration

- [x] 2.2.1. Implement comment detail view with inline code context and syntax
  highlighting via syntect; acceptance: code blocks render with 80-column
  wrapping and fallback to plain text when highlighting fails.
- [x] 2.2.2. Provide full-screen diff context with jump-to-change navigation;
  acceptance: keyboard shortcuts move between hunks, and rendering remains
  under 100ms on the reference dataset measured in local profiling.
- [x] 2.2.3. Add time-travel navigation across PR history; acceptance:
  selecting a comment replays the relevant commit snapshot and verifies line
  mapping correctness against git2 diffs.
- [ ] 2.2.4. Make `TimeTravelParams` part of the public library API, including
  `from_comment` (or equivalent) to derive parameters from comment metadata.
  See `docs/frankie-design.md` §ADR-005.
- [ ] 2.2.5. Make `TimeTravelState` a stable public type (remove
  `#[doc(hidden)]` and publish required getters currently `pub(crate)`).
  Requires 2.2.4. See `docs/frankie-design.md` §ADR-005.
- [ ] 2.2.6. Replace the fixed internal commit history limit with configurable
  options (for example a `commit_history_limit` setting for time travel).
  Requires 2.2.4. See `docs/frankie-design.md` §ADR-005.
- [ ] 2.2.7. Extract time-travel orchestration out of TUI handlers into pure
  library services, keeping `bubbletea_rs::Cmd`, `spawn_blocking`, and any
  global `OnceLock` context in the TUI adapter layer only. Requires 2.2.5 and
  2.2.6. See `docs/frankie-design.md` §ADR-005.

### 2.3. Comment export pipeline

- [x] 2.3.1. Deliver structured comment export (location, code context, issue
  text) in Markdown and JSONL formats; acceptance: exports include stable
  ordering, pass schema validation, and are exercised in integration tests.
- [x] 2.3.2. Introduce template-driven export customization; acceptance:
  templates support placeholders for file, line, reviewer, and status, with
  unit tests covering substitution and escaping rules.

## 3. AI-assisted workflows

Integrate OpenAI Codex CLI workflows to assist and automate comment resolution.

### 3.1. Codex execution integration

- [x] 3.1.1. Wire comment exports into `codex app-server` with streaming
  progress and JSONL capture; acceptance: executions stream status updates,
  write transcripts to disk, and surface non-zero Codex exits to the TUI.
- [x] 3.1.2. Enable session resumption for interrupted Codex runs; acceptance:
  resuming reuses prior transcript and preserves approvals; the resumption code
  path is guarded by regression tests with at least 90% unit and integration
  coverage; CI demonstrates at least 99% successful resume outcomes for
  interrupted runs across five end-to-end scenarios.

### 3.2. Template and reply automation

- [x] 3.2.1. Provide template-based reply drafting with keyboard-driven
  insertion; acceptance: finish criteria are at least 90% automated test
  coverage for new reply-draft UI paths, p95 inline draft render latency under
  200ms, edit-before-send support for 100% of configured template slots, and
  length-limit enforcement for all draft mutations according to configured
  policy; in scope: template types, keyboard shortcuts, inline rendering,
  edit-before-send, and length enforcement; out of scope: live GitHub
  submission, AI rewording, and server-side workflow automation; prerequisites
  or dependencies: approved design mockups, available keyboard shortcut
  service, and a defined length-limit configuration schema. See
  `docs/frankie-design.md` §ADR-004.
- [x] 3.2.2. Add AI-powered comment expansion and rewording; acceptance:
  generated text is labelled as AI-originated, offers side-by-side diff
  preview, and falls back gracefully when the AI call fails; delivery includes
  reusable library APIs and both TUI and CLI access paths. See
  `docs/frankie-design.md` §ADR-006.
- [ ] 3.2.3. Extract reply templating out of TUI state into a top-level library
  module and re-export it as a public API (currently in `src/tui/state/`). See
  `docs/frankie-design.md` §ADR-005.
- [ ] 3.2.4. Make reply templating input a library data transfer object (DTO)
  (for example `ReplyTemplateContext`) instead of requiring `ReviewComment`
  directly. Requires 3.2.3. See `docs/frankie-design.md` §ADR-005.
- [ ] 3.2.5. Expose default reply templates as a public library API (they are
  currently crate-private). Requires 3.2.3. See `docs/frankie-design.md`
  §ADR-004.
- [ ] 3.2.6. Keep TUI-specific pieces as adapters only by updating the TUI and
  CLI surfaces to depend on the library reply templating APIs, not
  `crate::tui::state`. Requires 3.2.4 and 3.2.5. See `docs/frankie-design.md`
  §ADR-005.

### 3.3. Automated verification

- [ ] 3.3.1. Implement automated resolution verification by replaying diffs
  and checking comment conditions; acceptance: verification results annotate
  comments as verified or unverified and persist status in the local cache;
  delivery includes reusable library APIs and both TUI and CLI access paths.
- [ ] 3.3.2. Provide summary generation for PR-level discussions; acceptance:
  summaries group comments by file and severity, and include links back to TUI
  views; delivery includes reusable library APIs and both TUI and CLI access
  paths.

## 4. Resilience, security, and compliance

Harden the application for offline use, error transparency, and safe token
handling.

### 4.1. Offline and rate-limit resilience

- [ ] 4.1.1. Add offline mode with queued operations; acceptance: read-only
  features remain usable without network, and queued writes replay once
  connectivity returns, confirmed by integration tests with simulated outages;
  delivery includes reusable library APIs and both TUI and CLI access paths.
- [ ] 4.1.2. Implement GitHub rate-limit awareness and backoff; acceptance:
  requests respect `Retry-After` headers, backoff is logged, and unit tests
  cover limit exhaustion scenarios; delivery includes reusable library APIs and
  both TUI and CLI access paths.

### 4.2. Security and privacy controls

- [ ] 4.2.1. Secure token storage and redaction in logs; acceptance: tokens are
  never printed, storage uses OS keyring or encrypted file, and log scrubbing
  is validated with snapshot tests; delivery includes reusable library APIs and
  both TUI and CLI access paths where applicable.
- [ ] 4.2.2. Add data minimization for telemetry; acceptance: telemetry is
  opt-in by default, anonymizes identifiers, and writes to local files only;
  delivery includes reusable library APIs and both TUI and CLI access paths
  where applicable.

## 5. UX polish and release readiness

Deliver user-facing refinements, accessibility, documentation, and packaging.

### 5.1. Interaction polish

- [ ] 5.1.1. Expand keyboard shortcut coverage with in-app help overlay;
  acceptance: every view lists discoverable shortcuts, and help content matches
  actual bindings verified by UI state tests; shared behaviours must also
  remain available through reusable library APIs and CLI access where
  applicable.
- [ ] 5.1.2. Provide accessibility-friendly theming with monochrome fallback;
  acceptance: colour-blind safe palette is available, and contrast ratios meet
  Web Content Accessibility Guidelines (WCAG) AA for terminal defaults; shared
  behaviours must also remain available through reusable library APIs and CLI
  access where applicable.

### 5.2. Documentation and distribution

- [ ] 5.2.1. Publish user guide and troubleshooting documentation aligned with
  the design; acceptance: guides live in `docs/`, pass `make markdownlint`, and
  include screenshots or text equivalents for flows, including TUI, library,
  and CLI usage patterns.
- [ ] 5.2.2. Package binaries for major platforms with checksum generation;
  acceptance: release artefacts build via CI, include changelog entries, and
  are verified with signature or checksum validation in CI; packaging guidance
  documents integration expectations for TUI, library, and CLI consumers.
- [ ] 5.2.3. Re-export extracted core modules from `src/lib.rs` so the crate is
  usable as a library without the TUI runtime. Requires 2.2.7 and 3.2.6. See
  `docs/frankie-design.md` §ADR-005.
- [ ] 5.2.4. Gate TUI dependencies behind a `tui` feature in `Cargo.toml`,
  ensuring consumers can build and use the library without pulling in the TUI
  runtime. Requires 5.2.3. See `docs/frankie-design.md` §ADR-005.
