# Replace the fixed internal commit history limit with configurable options

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS

`PLANS.md` is not present in the repository root, so no additional
plan-governance document applies.

## Purpose / big picture

Roadmap item 2.2.6 replaces the hardcoded `COMMIT_HISTORY_LIMIT` constant
(currently `50`, defined at `src/tui/app/time_travel_handlers/mod.rs` line 21)
with a user-configurable setting. After this change a caller can control how
many commits the time-travel feature loads by setting `commit_history_limit`
via the configuration file (`.frankie.toml`), the
`FRANKIE_COMMIT_HISTORY_LIMIT` environment variable, or the
`--commit-history-limit` command-line flag. The default remains `50`,
preserving existing behaviour for users who do not override it.

Success is observable in four ways. First, setting `commit_history_limit = 10`
in `.frankie.toml` causes the time-travel commit history to contain at most 10
entries. Second, running with the default configuration continues to load up to
50 commits exactly as before. Third, the public library API exposes the limit
so that embedded hosts can read or pass it without depending on `crate::tui`.
Fourth, unit and behavioural tests exercise both the default and overridden
limits, confirming that the value flows from configuration through to loaded
history length.

This slice is intentionally narrow. It does not extract time-travel
orchestration out of TUI handlers (that is roadmap item 2.2.7). It adds a
single configuration field, threads it to the one call site that consumes the
limit, and proves the change with tests.

## Constraints

- The default limit must remain `50`. Existing users who have no configuration
  override must see identical behaviour.
- The new field must follow the same layered configuration pattern used by
  every other `FrankieConfig` field: defaults, `.frankie.toml`, environment
  variable (`FRANKIE_COMMIT_HISTORY_LIMIT`), CLI flag
  (`--commit-history-limit`).
- The constant `COMMIT_HISTORY_LIMIT` in
  `src/tui/app/time_travel_handlers/mod.rs` must be removed and replaced by the
  value read from configuration. No second copy of the default may exist
  outside `src/config/mod.rs`.
- The limit must be available as a public library value so that roadmap item
  2.2.7 (orchestration extraction) and external hosts can use it. Per
  `docs/adr-005-cross-surface-library-first-delivery.md`, core behaviour must
  live in shared library modules, not solely in TUI code.
- Every new Rust module must begin with a `//!` module comment.
- No single source file may exceed 400 lines.
- Unit tests must use `rstest`.
- Behavioural tests must use `rstest-bdd` v0.5.0 and
  `#[scenario(path = ...)]`.
- Behavioural helpers returning `Result` must not use `assert!`; return
  explicit step errors to satisfy `clippy::panic_in_result_fn` under
  `-D warnings`.
- Documentation must use en-GB-oxendict spelling and follow the style guide at
  `docs/documentation-style-guide.md`.
- Do not mark roadmap item 2.2.6 as done until code, tests, design docs, user
  docs, and validation gates have all passed.
- `FrankieConfig::VALUE_FLAGS` must be updated to include
  `--commit-history-limit` since it is a value-bearing flag.
- A zero or unreasonably low limit must not be silently accepted. A minimum of
  `1` must be enforced — a limit of zero commits would render time travel
  non-functional.

## Tolerances (exception triggers)

- Scope: if the implementation needs more than 15 files or more than 500 net
  new lines, stop and escalate with a narrower staging proposal.
- Interface: if making the limit configurable requires changing the public
  shape of `GitOperations::get_parent_commits`, `TimeTravelState`,
  `TimeTravelInitParams`, or `TimeTravelParams`, stop and escalate.
- Dependencies: if a new external dependency is required, stop and escalate.
- Validation: if `make check-fmt`, `make lint`, or `make test` fail after
  three fix cycles, stop and escalate with the logs and current diff.

## Risks

- Risk: the `OnceLock`-based global storage pattern used to pass configuration
  from the CLI layer to the TUI `Model::init()` callback makes it difficult to
  thread the new limit value without adding yet another global. Severity: low.
  Likelihood: medium. Mitigation: the limit is a single `usize` that can be
  stored alongside the existing `GIT_OPS_CONTEXT` global or as a dedicated
  lightweight `OnceLock<usize>`. Both approaches keep the blast radius small.
  The preferred approach is to add the limit to the existing `GIT_OPS_CONTEXT`
  tuple or store it alongside `ReviewApp`'s existing builder-pattern fields so
  it flows naturally to the handler. Alternatively, we can store the
  `commit_history_limit` directly on `ReviewApp` via a new
  `.with_commit_history_limit()` builder method, consistent with the existing
  pattern for `reply_draft_config`, `codex_poll_interval`, etc.

- Risk: the `ortho_config` macro may not support a `usize` field for CLI
  parsing without extra configuration. Severity: low. Likelihood: low.
  Mitigation: `FrankieConfig` already contains `reply_max_length: usize` and
  `pr_metadata_cache_ttl_seconds: u64`, both of which work with `ortho_config`.
  The new field uses the same pattern.

- Risk: existing BDD tests in `tests/time_travel_bdd.rs` use
  `MockGitOperations` with `get_parent_commits` expectations that return a
  fixed vector. The tests do not assert on the `limit` argument. If we change
  the call site to pass a different limit, these tests will still pass, which
  is correct — but they do not prove the limit flows correctly. Severity: low.
  Likelihood: high. Mitigation: add dedicated tests that verify the limit
  parameter reaches the `get_parent_commits` call.

- Risk: design documentation drifts by implying this item delivered more than
  it actually did. Severity: low. Likelihood: medium. Mitigation: document the
  exact boundary — configurable limit, not orchestration extraction — in
  `docs/frankie-design.md`.

## Progress

- [ ] Read and internalise current codebase state, roadmap, and referenced
      architecture decision records.
- [ ] Draft this ExecPlan.
- [ ] Stage A: add `commit_history_limit` field to `FrankieConfig` with
      default `50`, update `Default` impl, and update `VALUE_FLAGS`.
- [ ] Stage B: thread the limit through TUI storage and `ReviewApp` to the
      time-travel handler, removing the hardcoded constant.
- [ ] Stage C: add unit tests for the new config field and the limit threading.
- [ ] Stage D: add BDD tests proving default and overridden limits.
- [ ] Stage E: update `docs/frankie-design.md`, `docs/users-guide.md`,
      `docs/roadmap.md`, and run all validation gates.

## Surprises & discoveries

(None yet.)

## Decision log

(None yet.)

## Outcomes & retrospective

(Not yet started.)

## Context and orientation

Frankie is a Rust-based code review platform. Its time-travel feature lets
users view the exact code state when a review comment was made and navigate
through commit history. The feature was implemented in roadmap item 2.2.3 and
progressively extracted into the public library API in items 2.2.4 (parameters)
and 2.2.5 (state).

### Current commit history limit

The constant `COMMIT_HISTORY_LIMIT` is defined at
`src/tui/app/time_travel_handlers/mod.rs` line 21:

```rust
const COMMIT_HISTORY_LIMIT: usize = 50;
```

It is consumed at line 308 in `load_time_travel_state()`:

```rust
let commit_history = git_ops.get_parent_commits(params.commit_sha(), COMMIT_HISTORY_LIMIT)?;
```

The `GitOperations` trait method `get_parent_commits(sha, limit)` (defined in
`src/local/git_ops/mod.rs`) already accepts a `limit: usize` parameter, so no
trait changes are needed — the limit simply needs to flow from configuration
rather than from a constant.

### Configuration system

Configuration is managed by the `FrankieConfig` struct in `src/config/mod.rs`,
which uses the `ortho_config` procedural macro. Fields support layered
precedence: built-in defaults, `.frankie.toml` file, `FRANKIE_*` environment
variables, and CLI flags. Existing numeric fields (`reply_max_length: usize`,
`pr_metadata_cache_ttl_seconds: u64`, `ai_timeout_seconds: u64`) demonstrate
the pattern for adding a new numeric field.

### TUI configuration flow

Configuration flows from `FrankieConfig` to the TUI in two phases:

1. The CLI entrypoint (`src/cli/review_tui.rs`) reads `FrankieConfig` and
   stores values in `OnceLock`-based global storage (`src/tui/storage.rs`),
   then calls the bubbletea-rs program.
2. `ReviewApp::init()` (`src/tui/app/model_impl.rs`) reads from global
   storage and wires up the app via builder methods such as `.with_git_ops()`,
   `.with_reply_draft_config()`, etc.

The commit history limit will follow this existing pattern: the CLI layer reads
the value from `FrankieConfig`, passes it to `ReviewApp` via a new builder
method, and the time-travel handler reads it from `ReviewApp` instead of using
a constant.

### Key files

- `src/config/mod.rs` — `FrankieConfig` struct and defaults.
- `src/tui/app/time_travel_handlers/mod.rs` — handlers containing the
  `COMMIT_HISTORY_LIMIT` constant and `load_time_travel_state()`.
- `src/tui/app/mod.rs` — `ReviewApp` struct and builder methods.
- `src/tui/app/model_impl.rs` — `Model::init()` wiring.
- `src/cli/review_tui.rs` — CLI-to-TUI bridge.
- `src/tui/storage.rs` — `OnceLock`-based global storage.
- `src/local/git_ops/mod.rs` — `GitOperations` trait with
  `get_parent_commits(sha, limit)`.
- `src/time_travel/mod.rs` — public library module for time-travel types.
- `tests/time_travel_bdd.rs` — existing TUI BDD tests.
- `tests/features/time_travel.feature` — existing Gherkin feature file.

### Types involved

`FrankieConfig` gains a new `commit_history_limit: usize` field with default
`50`. The field is public, matching the existing convention where all
`FrankieConfig` fields are `pub`.

`ReviewApp` gains a new `commit_history_limit: usize` field (defaulting to the
library constant) and a `.with_commit_history_limit(limit)` builder method.

No changes to `GitOperations`, `TimeTravelState`, `TimeTravelInitParams`, or
`TimeTravelParams` are required.

## Plan of work

### Stage A: add the configuration field

Add a `commit_history_limit: usize` field to `FrankieConfig` in
`src/config/mod.rs`. Provide a `DEFAULT_COMMIT_HISTORY_LIMIT` constant set to
`50` and use it in the `Default` implementation. Add Rustdoc explaining the
field's purpose, the sources it can be provided from (CLI, environment, config
file), and the default value. Add `--commit-history-limit` to the `VALUE_FLAGS`
array.

Also expose the default constant publicly from the `config` module so the
library API and test code can reference it, following the pattern of
`DEFAULT_REPLY_MAX_LENGTH`. Re-export it from `src/lib.rs` if needed for test
accessibility.

Stage A validation: run `cargo check --workspace` to confirm the new field
compiles and does not break existing code.

### Stage B: thread the limit to the handler

This stage connects the configuration value to the call site that previously
used the hardcoded constant.

In `src/tui/app/mod.rs`, add a `commit_history_limit: usize` field to
`ReviewApp` with the default from the config constant. Add a
`.with_commit_history_limit(limit: usize)` builder method following the
existing pattern (for example `.with_codex_poll_interval()`).

In `src/cli/review_tui.rs`, read `config.commit_history_limit` and pass it to
`ReviewApp` via the new builder method. This follows the existing pattern where
other config values (for example `reply_max_length`, `ai_timeout_seconds`) are
read from `FrankieConfig` and passed to dedicated service or app fields.

In `src/tui/app/time_travel_handlers/mod.rs`, remove the
`const COMMIT_HISTORY_LIMIT: usize = 50;` line. Update
`load_time_travel_state()` to accept the limit as a parameter. Update the call
sites in `handle_enter_time_travel()` and related handler code to pass
`self.commit_history_limit` when calling the load function. Since
`load_time_travel_state` is a free function called from
`spawn_time_travel_load`, the limit will need to be captured and passed through
the async closure chain, following the same pattern used for `head_sha` and
`params`.

Stage B validation: run `make check-fmt`, `make lint`, and `make test`. All
existing tests must continue to pass.

### Stage C: add unit tests for the configuration field

Add unit tests in the existing config test module structure (under
`src/config/tests/`) verifying:

1. The default value of `commit_history_limit` is `50`.
2. The value can be overridden (for example via the figment/composer test
   pattern used by existing config tests).

Add unit tests in `src/tui/app/time_travel_handlers/tests.rs` verifying that
the limit parameter reaches the `get_parent_commits` call. Use `mockall`
expectations on the `MockGitOperations` to assert the exact limit value passed.

Stage C validation: run `make test` and confirm the new tests pass.

### Stage D: add BDD tests

Create `tests/features/commit_history_limit.feature` with scenarios covering:

1. Default commit history limit — when no override is configured, the
   time-travel history contains up to the default number of commits.
2. Overridden commit history limit — when a custom limit is provided, the
   loaded history respects that limit.
3. Minimum limit enforcement — a limit of zero is rejected or clamped to a
   minimum of `1`.

Create `tests/commit_history_limit_bdd.rs` with step definitions following the
pattern established in `tests/time_travel_params_bdd.rs` and
`tests/time_travel_state_bdd.rs`. The BDD state struct should derive
`ScenarioState` and `Default`, use `Slot<T>` for state, and follow the
`StepError`/`StepResult` pattern. Helpers returning `Result` must use explicit
error returns rather than `assert!`.

The tests should exercise the configuration-to-history-length path at the
library level, using `MockGitOperations` to verify that the correct limit is
passed through, and using constructed `TimeTravelState` values to verify
history lengths.

Stage D validation: run `cargo test --test commit_history_limit_bdd` to confirm
the new suite passes, then run `make test` to confirm no regressions.

### Stage E: update documentation and mark roadmap item done

Update `docs/frankie-design.md` to record the design decision. Add a paragraph
under Feature F-007 (after the existing "Library API (roadmap 2.2.5)"
subsection) with a heading "##### Library API (roadmap 2.2.6)". Document that
the commit history limit is now configurable via
`FrankieConfig::commit_history_limit`, the default is `50`, and no CLI surface
beyond the configuration flag is added for this slice.

Update `docs/users-guide.md` to document the new configuration option in the
appropriate sections:

- Add `commit_history_limit` to the configuration file example under
  `.frankie.toml`.
- Add `FRANKIE_COMMIT_HISTORY_LIMIT` to the environment variables table.
- Add `--commit-history-limit` to the CLI flags documentation.
- Add a note in the time-travel mode section explaining that the number of
  commits loaded is configurable.

After implementation and validation are complete, update `docs/roadmap.md` to
mark item 2.2.6 as done (change `- [ ]` to `- [x]`). Do not change the roadmap
checkbox during the draft phase.

Stage E validation: run the full validation suite:

```bash
set -o pipefail && make fmt 2>&1 | tee /tmp/2-2-6-fmt.log
set -o pipefail && MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint 2>&1 | tee /tmp/2-2-6-markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/2-2-6-nixie.log
set -o pipefail && make check-fmt 2>&1 | tee /tmp/2-2-6-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/2-2-6-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/2-2-6-test.log
```

## Concrete steps

All commands run from the repository root (`/home/user/project`).

Format docs and source:

```bash
set -o pipefail && make fmt 2>&1 | tee /tmp/2-2-6-fmt.log
```

Validate Markdown:

```bash
set -o pipefail && MDLINT=/root/.bun/bin/markdownlint-cli2 make markdownlint 2>&1 | tee /tmp/2-2-6-markdownlint.log
```

Validate Mermaid diagrams:

```bash
set -o pipefail && make nixie 2>&1 | tee /tmp/2-2-6-nixie.log
```

Verify formatting:

```bash
set -o pipefail && make check-fmt 2>&1 | tee /tmp/2-2-6-check-fmt.log
```

Run Clippy and Rustdoc checks:

```bash
set -o pipefail && make lint 2>&1 | tee /tmp/2-2-6-lint.log
```

Run the full test suite:

```bash
set -o pipefail && make test 2>&1 | tee /tmp/2-2-6-test.log
```

Confirm the new behavioural suite is part of the green run:

```bash
set -o pipefail && cargo test --test commit_history_limit_bdd 2>&1 | tee /tmp/2-2-6-bdd.log
```

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes. The new BDD test
  `commit_history_limit_bdd` passes. All existing tests in the workspace
  continue to pass.
- Lint/typecheck: `make check-fmt` and `make lint` both pass with zero
  warnings.
- Documentation: `docs/users-guide.md` documents the new
  `commit_history_limit` option. `docs/frankie-design.md` records the design
  decision under F-007.

Quality method (how to verify):

- Run the commands listed in "Concrete steps" above and compare exit codes
  against zero.
- Verify that `FrankieConfig::default().commit_history_limit` equals `50`.
- Verify that the constant `COMMIT_HISTORY_LIMIT` no longer exists in
  `src/tui/app/time_travel_handlers/mod.rs`.
- Verify that `tests/commit_history_limit_bdd.rs` exercises both the default
  and overridden limits.

Success criteria for close-out:

- `FrankieConfig` exposes a `commit_history_limit: usize` field defaulting to
  `50`.
- The field is configurable via `.frankie.toml`, `FRANKIE_COMMIT_HISTORY_LIMIT`,
  and `--commit-history-limit`.
- The TUI time-travel handler reads the limit from `ReviewApp` rather than a
  hardcoded constant.
- The hardcoded `COMMIT_HISTORY_LIMIT` constant is removed.
- Unit tests verify the default value and configuration override.
- BDD tests verify that both default and overridden limits affect loaded
  history length.
- `docs/frankie-design.md` reflects the configurable limit under F-007.
- `docs/users-guide.md` documents all three configuration sources.
- `docs/roadmap.md` marks item 2.2.6 done only after the full validation
  suite passes.
- All of `make check-fmt`, `make lint`, and `make test` pass.

## Idempotence and recovery

Every stage can be re-run safely. The changes add a new configuration field and
thread it through existing code paths. No persistent state (database schema,
user data, configuration files) is modified destructively. If a stage fails
midway, reverting the changed files via `git checkout -- <file>` and retrying
is safe.

## Artefacts and notes

The following files are created by this plan:

- `tests/commit_history_limit_bdd.rs` — new BDD step definitions (~120
  lines).
- `tests/features/commit_history_limit.feature` — new Gherkin feature file
  (~30 lines).

The following files are modified:

- `src/config/mod.rs` — add `commit_history_limit` field, default constant,
  and `VALUE_FLAGS` entry.
- `src/tui/app/mod.rs` — add `commit_history_limit` field and builder method
  to `ReviewApp`.
- `src/tui/app/time_travel_handlers/mod.rs` — remove hardcoded constant,
  thread limit parameter through load functions.
- `src/tui/app/model_impl.rs` — wire the limit from storage/config to
  `ReviewApp`.
- `src/cli/review_tui.rs` — read the config value and pass it through.
- `src/tui/storage.rs` — store the commit history limit if needed (or pass
  directly via builder).
- `docs/frankie-design.md` — document the design decision.
- `docs/users-guide.md` — document the new configuration option.
- `docs/roadmap.md` — mark 2.2.6 as done (final step only).

Expected net change: approximately 300 new lines (BDD tests, config field,
documentation) plus small modifications to existing files, well within the
15-file / 500-line tolerance.

## Interfaces and dependencies

No new external dependencies are required. All changes use existing crate
infrastructure.

The public interface after this change adds one field to `FrankieConfig`:

```rust
/// Maximum number of commits to load in time-travel history.
///
/// Controls how many parent commits are retrieved when entering
/// time-travel mode. Larger values provide more navigation depth
/// at the cost of increased load time for repositories with long
/// histories.
///
/// Can be provided via:
/// - CLI: `--commit-history-limit <COUNT>`
/// - Environment: `FRANKIE_COMMIT_HISTORY_LIMIT`
/// - Config file: `commit_history_limit = 50`
#[ortho_config()]
pub commit_history_limit: usize,
```

The default constant:

```rust
pub const DEFAULT_COMMIT_HISTORY_LIMIT: usize = 50;
```

The `ReviewApp` builder addition:

```rust
/// Sets the maximum number of commits to load in time-travel history.
#[must_use]
pub fn with_commit_history_limit(mut self, limit: usize) -> Self {
    self.commit_history_limit = limit;
    self
}
```

The modified `load_time_travel_state` signature:

```rust
fn load_time_travel_state(
    git_ops: &dyn GitOperations,
    params: &TimeTravelParams,
    head_sha: Option<&CommitSha>,
    commit_history_limit: usize,
) -> Result<TimeTravelState, GitOperationError>
```

## Approval gate

This plan is in DRAFT status and awaits explicit approval before implementation
begins.
