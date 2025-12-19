# Frankie goes to Code Review roadmap

This roadmap translates the design in `docs/frankie-design.md` into phased,
measurable delivery slices. It focuses on outcomes, avoids time commitments,
and keeps tasks small enough to complete within a few weeks. Completion
criteria emphasise observable behaviours and tests rather than intent.

## Scope and principles

- Anchor delivery on GitHub pull request workflows, local-first execution, and
  AI-assisted resolution, as defined in the design.
- Sequence work to reduce risk: establish data and access layers before
  experience and automation layers.
- Keep each task independently testable, avoiding hidden coupling between
  phases.

## Phase 1: Access and data foundations

Establish reliable repository intake, authentication, and local persistence to
support later workflow features.

### Step: GitHub and repository intake

- [x] Provide PR URL intake using octocrab with token validation and explicit
      error surfacing; acceptance: opening a valid PR URL loads metadata and
      comments, invalid tokens return a user-readable failure message, and
      integration tests cover success and auth error paths.
- [x] Implement command-line parsing with `ortho-config` so flags, environment
      variables, and config files share a single schema; acceptance: CLI help
      documents all options, precedence order is tested, and defaults mirror
      config file values.
- [x] Implement owner/repository discovery with paginated PR listing;
      acceptance: repository selection lists at least 50 PRs with pagination
      controls and asserts rate-limit headers are handled without panics.
- [ ] Add local repository discovery via git2 that maps remotes to GitHub
      origins; acceptance: running in a Git repo pre-fills owner/repo details
      and warns when remote origin is missing.

### Step: Persistence and configuration

- [x] Define initial SQLite schema (repositories, pull requests, comments,
      sync checkpoints) using Diesel migrations; acceptance: `diesel migration
      run` succeeds and schema version is recorded in app telemetry.
- [ ] Implement local caching layer with coherent expiry policy; acceptance:
      cached PR metadata is reused across sessions and invalidates on ETag or
      Last-Modified changes detected via octocrab responses.
- [x] Introduce configuration loading via ortho-config with environment and
      file sources; acceptance: configuration precedence is unit-tested and
      misconfiguration produces actionable error messages.

## Phase 2: Review navigation and context

Deliver the core TUI experience for navigating reviews and understanding code
context.

### Step: Review listing and filtering

- [ ] Build review listing view with filters (all, unresolved, by file, by
      reviewer, by commit range); acceptance: filters execute without full
      reloads, retain cursor position, and are covered by state update tests.
- [ ] Implement incremental sync to keep review lists up to date; acceptance:
      background refresh merges new comments without losing selection state and
      logs sync latency metrics locally.

### Step: Contextual comment exploration

- [ ] Implement comment detail view with inline code context and syntax
      highlighting via syntect; acceptance: code blocks render with 80-column
      wrapping and fallback to plain text when highlighting fails.
- [ ] Provide full-screen diff context with jump-to-change navigation; accept-
      ance: keyboard shortcuts move between hunks, and rendering remains under
      100ms on the reference dataset measured in local profiling.
- [ ] Add time-travel navigation across PR history; acceptance: selecting a
      comment replays the relevant commit snapshot and verifies line mapping
      correctness against git2 diffs.

### Step: Comment export pipeline

- [ ] Deliver structured comment export (location, code context, issue text) in
      Markdown and JSONL formats; acceptance: exports include stable ordering,
      pass schema validation, and are exercised in integration tests.
- [ ] Introduce template-driven export customisation; acceptance: templates
      support placeholders for file, line, reviewer, and status, with unit
      tests covering substitution and escaping rules.

## Phase 3: AI-assisted workflows

Integrate OpenAI Codex CLI workflows to assist and automate comment resolution.

### Step: Codex execution integration

- [ ] Wire comment exports into `codex exec` with streaming progress and JSONL
      capture; acceptance: executions stream status updates, write transcripts
      to disk, and surface non-zero Codex exits to the TUI.
- [ ] Enable session resumption for interrupted Codex runs; acceptance:
      resuming reuses prior transcript, preserves approvals, and is covered by
      regression tests for interrupted runs.

### Step: Template and reply automation

- [ ] Provide template-based reply drafting with keyboard-driven insertion;
      acceptance: replies render inline, support edit-before-send, and enforce
      configured length limits.
- [ ] Add AI-powered comment expansion and rewording; acceptance: generated
      text is labelled as AI-originated, offers side-by-side diff preview, and
      falls back gracefully when the AI call fails.

### Step: Automated verification

- [ ] Implement automated resolution verification by replaying diffs and
      checking comment conditions; acceptance: verification results annotate
      comments as verified/unverified and persist status in the local cache.
- [ ] Provide summary generation for PR-level discussions; acceptance: summaries
      group comments by file and severity, and include links back to TUI views.

## Phase 4: Resilience, security, and compliance

Harden the application for offline use, error transparency, and safe token
handling.

### Step: Offline and rate-limit resilience

- [ ] Add offline mode with queued operations; acceptance: read-only features
      remain usable without network, and queued writes replay once connectivity
      returns, confirmed by integration tests with simulated outages.
- [ ] Implement GitHub rate-limit awareness and backoff; acceptance: requests
      respect `Retry-After` headers, backoff is logged, and unit tests cover
      limit exhaustion scenarios.

### Step: Security and privacy controls

- [ ] Secure token storage and redaction in logs; acceptance: tokens are never
      printed, storage uses OS keyring or encrypted file, and log scrubbing is
      validated with snapshot tests.
- [ ] Add data minimisation for telemetry; acceptance: telemetry is opt-in by
      default, anonymises identifiers, and writes to local files only.

## Phase 5: UX polish and release readiness

Deliver user-facing refinements, accessibility, documentation, and packaging.

### Step: Interaction polish

- [ ] Expand keyboard shortcut coverage with in-app help overlay; acceptance:
      every view lists discoverable shortcuts, and help content matches actual
      bindings verified by UI state tests.
- [ ] Provide accessibility-friendly theming with monochrome fallback; accept-
      ance: colour-blind safe palette is available, and contrast ratios meet
      WCAG AA for terminal defaults.

### Step: Documentation and distribution

- [ ] Publish user guide and troubleshooting documentation aligned with the
      design; acceptance: guides live in `docs/`, pass `make markdownlint`, and
      include screenshots or text equivalents for flows.
- [ ] Package binaries for major platforms with checksum generation; accept-
      ance: release artefacts build via CI, include changelog entries, and are
      verified with signature or checksum validation in CI.
