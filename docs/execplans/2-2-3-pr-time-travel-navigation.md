# Time-travel navigation across PR history

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

PLANS.md is not present in the repository root, so no additional plan
governance applies.

## Purpose / Big picture

Add time-travel navigation across PR history to the review text-based user
interface (TUI) so a reviewer can see the exact code state when a comment was
made, verify that line mappings remain correct, and navigate between commits in
the PR history. Success is visible when pressing `t` on a selected comment
shows the commit snapshot with file content at that commit, line mapping
verification displays correct/moved/deleted status, `h`/`l` navigate between
commits, and behavioural tests cover all scenarios.

## Constraints

- Keep the model-view-update (MVU) split intact: update logic stays in
  `src/tui/app/`, rendering stays in `src/tui/app/rendering.rs`, and components
  remain pure view helpers.
- Every new module begins with a `//!` module-level comment.
- No single file may exceed 400 lines; split into feature-focused modules if
  needed.
- Use `rstest` for unit tests and `rstest-bdd` v0.4.0 for behavioural tests.
- Use `git2` for Git operations (not shell commands).
- Use dependency injection (`GitOperations` trait) for testability.
- Any filesystem access must use `cap_std`, `cap_std::fs_utf8`, and `camino`.
- Avoid adding new dependencies beyond the existing stack; if unavoidable,
  escalate before proceeding.
- Documentation updates must follow the en-GB style guide, wrap at 80 columns,
  and pass `make markdownlint`, `make fmt`, and `make nixie`.
- Use Makefile targets for validation (`make check-fmt`, `make lint`,
  `make test`).

## Tolerances (exception triggers)

- Scope: if implementation needs more than 20 files or 1000 net new lines, stop
  and escalate.
- Interface: if any public API signature must change, stop and escalate.
- Dependencies: if a new external dependency is required beyond git2/mockall,
  stop and escalate.
- Tests: if tests still fail after two fix attempts, stop and escalate with the
  latest failure output.
- Ambiguity: if the line mapping algorithm choice materially affects accuracy,
  stop and ask for confirmation.

## Risks

- Risk: git2 operations slow on large repos. Severity: medium. Likelihood:
  medium. Mitigation: use `spawn_blocking`, cache results in state, show
  loading indicator.
- Risk: Comment's commit SHA not in local repo (force-push). Severity: medium.
  Likelihood: medium. Mitigation: validate commit exists, show clear error
  message.
- Risk: File renamed between commits breaks mapping. Severity: low. Likelihood:
  low. Mitigation: enable git2 rename detection.
- Risk: No local repository available. Severity: low. Likelihood: low.
  Mitigation: check repo_path before entering, show helpful error.

## Progress

- [x] Stage A: Git2 Foundation
- [x] Stage B: TUI State
- [x] Stage C: Messages and Input
- [x] Stage D: App Integration
- [x] Stage E: View Component
- [x] Stage F: BDD Testing
- [x] Stage G: Documentation and Close-out

## Surprises & discoveries

- git2 revwalk sorting by `TIME` alone produces non-deterministic ordering when
  commits have the same timestamp (common in tests). Fixed by combining
  `TOPOLOGICAL | TIME` sorting.
- BDD tests using rstest-bdd cannot await async operations within step handlers.
  Solved by simulating the async callback pattern: sending `EnterTimeTravel`
  followed by `TimeTravelLoaded` to replicate what the runtime does.
- The `#[expect(dead_code)]` attribute is unfulfilled when running tests because
  the code is actually used in tests. Fixed with
  `#[cfg_attr(not(test), expect(dead_code, ...))]` for conditional suppression.
- git2 `Repository` is not `Sync`, requiring `Mutex<Repository>` wrapper for
  use with `Arc<dyn GitOperations>` trait objects.

## Decision log

- Decision: Use `GitOperations` trait with `Arc<dyn GitOperations>` for
  dependency injection to enable testing without real Git repositories.
  Rationale: consistent with project patterns, enables mockall usage for BDD
  tests. Date/Author: 2026-01-19, plan author.
- Decision: Use `tokio::task::spawn_blocking` for git2 operations to keep the
  TUI responsive during potentially slow Git operations. Rationale: git2 is
  synchronous, blocking the async runtime would freeze the UI. Date/Author:
  2026-01-19, plan author.
- Decision: Add `ViewMode::TimeTravel` following the existing pattern from
  `ViewMode::DiffContext`. Rationale: consistent with existing architecture.
  Date/Author: 2026-01-19, plan author.
- Decision: Use `t` to enter time-travel mode, `h`/`l` to navigate between
  commits, and `Esc` to exit. Rationale: vim-like navigation keys, `t` for
  "time-travel" is mnemonic. Date/Author: 2026-01-19, plan author.
- Decision: Walk backwards from comment's `commit_sha` for commit history
  rather than fetching full PR commit list. Rationale: simpler implementation,
  uses data already available in ReviewComment. Date/Author: 2026-01-19, plan
  author.

## Outcomes & retrospective

**Completed:** 2026-01-19

All acceptance criteria met:

- Pressing `t` on a selected comment enters time-travel mode showing commit
  snapshot with file content at that commit.
- Line mapping verification displays correct/moved/deleted status via
  `LineMappingVerification` with `Exact`, `Moved`, `Deleted`, and `NotFound`
  variants.
- `h`/`l` navigate commits in history, `Esc` exits preserving selection.
- Graceful error messages when no local repo or commit not found.
- Unit tests (rstest) cover git operations and state navigation (286 tests
  pass).
- BDD tests (rstest-bdd) cover 7 scenarios for happy/unhappy paths.
- `make check-fmt`, `make lint`, and `make test` all succeed.
- Documentation updated in `docs/users-guide.md` with keyboard shortcuts and
  feature description.

**Lessons learned:**

- The bubbletea-rs MVU pattern works well but async operations require careful
  handling of loading states and callback messages.
- Dependency injection via traits (`GitOperations`) enabled clean BDD testing
  with mocks without needing the actual Git repository.
- Conditional lint attributes (`#[cfg_attr(not(test), ...)]`) are essential
  when test code exercises paths that are dead in production builds.

## Context and orientation

The review TUI lives under `src/tui/`. `ReviewApp` in `src/tui/app/mod.rs`
contains model-view-update (MVU) state and update logic, while
`src/tui/app/rendering.rs` builds strings for the terminal. Keyboard inputs are
mapped in `src/tui/input.rs` to `AppMsg` variants in `src/tui/messages.rs`. The
current UI renders a review list (`ReviewListComponent`) and comment detail
pane (`CommentDetailComponent`) with syntax highlighting via `CodeHighlighter`,
plus a full-screen diff context view (`DiffContextComponent`).

Review comments carry several relevant fields in `src/github/models/mod.rs`:

- `commit_sha: Option<String>` - the commit SHA when the comment was made
- `diff_hunk: Option<String>` - the diff context around the comment
- `line_number: Option<u32>` - current line number
- `original_line_number: Option<u32>` - original line number in the diff
- `file_path: Option<String>` - path to the file

Local repository discovery exists in `src/local/discovery.rs` using git2,
providing `LocalRepository` with repository path and remote URL mapping.
Behavioural tests live under `tests/` with Gherkin feature files in
`tests/features/`.

## Plan of work

Stage A: Git2 Foundation. Create types for commit snapshots and line mapping
results, extend error handling, implement `GitOperations` trait with git2
implementation for fetching commit content and computing diffs.

Stage B: TUI State. Create `TimeTravelState` to track current commit, file
content, line mapping verification, and navigation state.

Stage C: Messages and Input. Add message variants for entering/exiting
time-travel mode and navigating commits. Add keyboard bindings.

Stage D: App Integration. Add `ViewMode::TimeTravel`, integrate state and
handlers into `ReviewApp`, route messages appropriately.

Stage E: View Component. Create `TimeTravelViewComponent` to render commit
snapshot, file content, and line mapping status.

Stage F: BDD Testing. Create feature file and test harness with mock
`GitOperations` implementation.

Stage G: Documentation and Close-out. Update user guide, mark roadmap entry
done, run all validation gates.

## Concrete steps

### Stage A: Git2 Foundation

1. Create `src/local/commit.rs` with:
   - `CommitSnapshot` struct (sha, message, author, timestamp, file_content)
   - `LineMappingResult` enum (Exact, Moved, Deleted, NotFound)
   - `LineMappingVerification` struct (original_line, current_line, status)

2. Extend `src/local/error.rs` with:
   - `CommitAccessFailed` variant
   - `DiffComputationFailed` variant

3. Create `src/local/git_ops.rs` with:
   - `GitOperations` trait defining:
     - `get_commit_snapshot(sha, file_path) -> Result<CommitSnapshot>`
     - `get_file_at_commit(sha, file_path) -> Result<String>`
     - `verify_line_mapping(old_sha, new_sha, file_path, line) -> Result<LineMappingVerification>`
     - `get_parent_commits(sha, limit) -> Result<Vec<String>>`
   - `Git2Operations` struct implementing the trait

4. Update `src/local/mod.rs` to export new types.

5. Add unit tests for git operations (inline in `git_ops.rs`).

### Stage B: TUI State

1. Create `src/tui/state/time_travel.rs` with:
   - `TimeTravelState` struct (commit_sha, file_path, file_content,
     line_mapping, commit_history, current_index, loading)
   - Navigation methods (next_commit, previous_commit)
   - Factory method to create from ReviewComment

2. Update `src/tui/state/mod.rs` to export new types.

3. Add unit tests for state navigation.

### Stage C: Messages and Input

1. Add message variants to `src/tui/messages.rs`:
   - `EnterTimeTravel`
   - `ExitTimeTravel`
   - `TimeTravelLoaded(Result<TimeTravelState>)`
   - `NextCommit`
   - `PreviousCommit`

2. Add `is_time_travel()` classification method.

3. Add keybindings to `src/tui/input.rs`:
    - `t` in ReviewList → `EnterTimeTravel`
    - `h` in TimeTravel → `PreviousCommit`
    - `l` in TimeTravel → `NextCommit`
    - `Esc` in TimeTravel → `ExitTimeTravel`

### Stage D: App Integration

1. Add `ViewMode::TimeTravel` to `src/tui/app/mod.rs`.

2. Add fields to `ReviewApp`:
    - `time_travel_state: Option<TimeTravelState>`
    - `repo_path: Option<Utf8PathBuf>`
    - `git_ops: Option<Arc<dyn GitOperations>>`

3. Create `src/tui/app/time_travel_handlers.rs` with handler methods:
    - `handle_enter_time_travel()`
    - `handle_exit_time_travel()`
    - `handle_time_travel_loaded()`
    - `handle_next_commit()`
    - `handle_previous_commit()`

4. Update message routing in `handle_message()`.

### Stage E: View Component

1. Create `src/tui/components/time_travel_view.rs` with:
    - `TimeTravelViewComponent`
    - `TimeTravelViewContext` (state, dimensions)
    - Header rendering (commit info, navigation position)
    - File content rendering with line highlighting
    - Line mapping status indicator

2. Update `src/tui/components/mod.rs` to export.

3. Add rendering call in `view()` method for TimeTravel mode.

### Stage F: BDD Testing

1. Create `tests/features/time_travel.feature` with scenarios:
    - Enter time-travel mode from review list
    - Display commit snapshot with file content
    - Navigate between commits with h/l
    - Show line mapping verification status
    - Handle missing commit gracefully
    - Handle missing local repository gracefully
    - Exit time-travel mode with Esc

2. Create `tests/time_travel_bdd.rs` entry point.

3. Create `tests/time_travel_bdd/state.rs` with `ScenarioState`.

4. Create `tests/time_travel_bdd/mock_git_ops.rs` with mock implementation.

### Stage G: Documentation and Close-out

1. Update `docs/users-guide.md` with:
    - Time-travel mode keybindings
    - Feature description and workflow

2. Mark roadmap entry as done in `docs/roadmap.md`.

3. Run validation gates:

    ```bash
    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/frankie-check-fmt.log
    make lint 2>&1 | tee /tmp/frankie-lint.log
    make test 2>&1 | tee /tmp/frankie-test.log
    ```

4. Run documentation validators:

    ```bash
    set -o pipefail
    make markdownlint 2>&1 | tee /tmp/frankie-markdownlint.log
    make fmt 2>&1 | tee /tmp/frankie-docs-fmt.log
    make nixie 2>&1 | tee /tmp/frankie-nixie.log
    ```

## Validation and acceptance

Acceptance is satisfied when the following are true:

- Pressing `t` on a selected comment enters time-travel mode showing commit
  snapshot with file content at that commit.
- Line mapping verification displays correct/moved/deleted status.
- `h`/`l` navigate commits, `Esc` exits preserving selection.
- Graceful error when no local repo or commit not found.
- Unit tests (rstest) cover git operations and state navigation.
- BDD tests (rstest-bdd) cover happy/unhappy paths.
- `make check-fmt`, `make lint`, and `make test` succeed.
- Documentation updates pass `make markdownlint`, `make fmt`, and `make nixie`.

Quality criteria:

- Tests: rstest unit tests and rstest-bdd scenarios for the new behaviour.
- Lint/typecheck: `make lint` clean.
- Formatting: `make check-fmt` clean.

## Idempotence and recovery

All steps are re-runnable. If tests fail, inspect the log files under `/tmp/`,
apply fixes, and rerun the same commands. If git2 operations fail, check that
the test repository fixtures are properly set up.

## Artefacts and notes

Example time-travel header (illustrative):

```text
Commit: abc1234  "Fix login validation"
File: src/auth.rs  Line 42 → 45 (moved)
[h] Previous  [l] Next  [Esc] Exit
```

Example line mapping status indicators:

```text
✓ Line 42 → 42 (exact match)
→ Line 42 → 45 (moved +3 lines)
✗ Line 42 (deleted)
? Line 42 (not found in current commit)
```

## Interfaces and dependencies

- New module: `src/local/commit.rs` with `CommitSnapshot`, `LineMappingResult`
- New module: `src/local/git_ops.rs` with `GitOperations` trait
- New module: `src/tui/state/time_travel.rs` with `TimeTravelState`
- New module: `src/tui/app/time_travel_handlers.rs` with handler methods
- New module: `src/tui/components/time_travel_view.rs` with view component
- Modified: `src/local/error.rs` - new error variants
- Modified: `src/local/mod.rs` - export new types
- Modified: `src/tui/messages.rs` - new message variants
- Modified: `src/tui/input.rs` - new keybindings
- Modified: `src/tui/app/mod.rs` - ViewMode, state fields, routing
- Modified: `src/tui/state/mod.rs` - export new state
- Modified: `src/tui/components/mod.rs` - export new component
- Modified: `docs/users-guide.md` - feature documentation
- Modified: `docs/roadmap.md` - mark entry done
- New test: `tests/features/time_travel.feature`
- New test: `tests/time_travel_bdd.rs` and submodules

## Keyboard bindings

| Key   | Context    | Action                                      |
| ----- | ---------- | ------------------------------------------- |
| `t`   | ReviewList | Enter time-travel mode for selected comment |
| `h`   | TimeTravel | Previous commit in history                  |
| `l`   | TimeTravel | Next commit in history                      |
| `Esc` | TimeTravel | Exit time-travel mode                       |

## Revision note

Initial draft created to cover time-travel navigation, commit snapshot display,
line mapping verification, navigation, tests, and documentation updates.
