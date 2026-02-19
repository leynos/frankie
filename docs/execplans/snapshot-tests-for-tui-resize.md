# Exec plan: fix TUI terminal sizing and resize responsiveness for issue #37

## Status

COMPLETED.

End-to-end pseudo-terminal (PTY) snapshot coverage is implemented and passes
for the targeted `interactive_tui_resize_snapshots` integration suite. The
harness was adjusted to work within `ratatui_testlib` writer constraints while
still validating both startup and dynamic resize behaviour.

## Purpose

- Ensure terminal UI height is derived from actual terminal dimensions on
  startup.
- Expand list and detail rendering to use the full available height.
- Recalculate layout dynamically on terminal resize.
- Validate behaviour using ratatui-testlib with Insta snapshot tests.

## Core context

- Issue: `#37` reports the TUI ignores terminal height and hard-caps at around
  25 lines.
- Relevant implementation files currently used by the app include:
  - `src/cli/review_tui.rs`
  - `src/tui/mod.rs`
  - `src/tui/app/mod.rs`
  - `src/tui/app/model_impl.rs`
  - `src/tui/components/comment_detail.rs`
  - `src/tui/app/tests.rs`
- Existing behavioural coverage is test-driven unit/state oriented and does not
  include PTY snapshots.
- Relevant guidance docs requested by the issue:
  - `docs/snapshot-testing-bubbletea-terminal-uis-with-insta.md`
  - `docs/building-idiomatic-terminal-uis-with-bubbletea-rs.md`
  - `docs/rust-testing-with-rstest-fixtures.md`

## Constraints

- Keep modifications to terminal behaviour and tests narrowly scoped to avoid
  functional churn.
- Preserve current navigation, filtering, and sync semantics.
- Do not introduce breaking layout dependencies for existing snapshot tests
  that do not involve terminal size.
- Use repository conventions for docs style and test structure.

## Tolerances and quality bars

- No hardcoded total-height caps should remain in layout calculations.
- Detail and list panes must still have minimum usable heights (non-zero where
  content is shown).
- Scrolling and cursor movement must remain bounded and stable after resizes.
- Snapshot assertions should allow for ANSI normalization to avoid brittle
  control-sequence diffs.

## Risks

- The detail component currently has explicit height assumptions and may
  require refactoring into a dynamic cap.
- Existing resize message handling may already work, but only for model-level
  tests, not full PTY integration.
- Aggressive refactor of layout maths can introduce index drift in list
  selection and scroll calculations.
- Snapshot maintenance can become noisy across terminals if normalization is
  incomplete.

## Progress log

- [x] Inspected current issue context and code paths.
- [x] Confirmed existing terminal size and resize plumbing exists and needs
      hardening.
- [x] Implement layout and detail-height fix to make list/detail height dynamic.
- [x] Implement PTY/integration snapshot coverage for startup and resize using
      bounded read windows to avoid blocking.
- [x] Add snapshot fixtures and assertions in
      `tests/interactive_tui_resize_snapshots.rs` (3 snapshots + resize
      transition captures).
- [x] Validate the issue with dedicated PTY + `insta` integration tests.
- [x] Run workspace-wide required gates for the touched area, or capture
      explicit approval to defer.
- [x] Address post-merge review feedback covering ANSI-safe truncation,
      timeout failure semantics in the PTY fixture, and documentation fixes for
      quote escaping and style guidance.

## Decision log

- Use existing `WindowSizeMsg` message path, not a new event channel, to
  preserve app architecture.
- Keep fixed chrome row heights where meaningful, but compute variable panel
  heights from terminal bounds rather than fixed maxima.
- Add explicit snapshot-driven tests for resize transitions because this is
  terminal-dependent behaviour.

## Plan

1. Verify size source-of-truth and startup behaviour
   - In `src/cli/review_tui.rs`, confirm terminal dimensions are queried before
     UI creation.
   - Ensure failures in size detection fall back to a safe deterministic
     default and do not reuse stale values.
   - Ensure the queried values are always passed into `ReviewApp`
     initialization consistently.

2. Remove height cap and make layout adaptive
   - In `src/tui/app/mod.rs`:
     - Keep terminal chrome constants only where fixed by UI semantics.
     - Recompute list/detail allocation from full `height` every render cycle.
     - Clamp values to minimums and avoid negative/zero rows.
   - In `src/tui/components/comment_detail.rs`:
     - Replace rigid detail height cap usage with parameterized or derived max
       height.
     - Keep content truncation behaviour where required for safety.
   - Keep `handle_resize` as the single code path for updating viewport
     dimensions.

3. Preserve cursor and scroll invariants on resize
   - In `src/tui/app/mod.rs`, validate `set_cursor_visible_rows` and list
     scroll bounds after `handle_resize`.
   - Add/update unit tests asserting cursor and top indices remain valid at
     minimum and large heights.
   - Explicitly test edge sizes and one-step larger/smaller transitions.

4. Wire runtime resize handling confidence
   - In `src/tui/app/model_impl.rs`, confirm translation from framework resize
     events maps to `AppMsg::WindowResized { width, height }`.
   - Verify terminal event subscription is still active for interactive mode
     and that the app receives updates.

5. Add ratatui-testlib + insta PTY tests
   - Create plan-aligned tests in `tests/`:
     - `tests/interactive_tui_resize_snapshots.rs`
   - Use `ratatui_testlib` to boot the app in controlled terminal sizes.
   - Capture frames using bounded `TestTerminal` read attempts with synthetic
     redraw probes.
   - Add `insta` snapshot fixtures:
     - startup at small height (for minimum clamp behaviour),
     - startup at large height (no cap),
     - resize sequence small → large → small.
   - Use `rstest` fixtures to generate parameterized size cases and avoid
     duplicated setup.
   - Normalize/strip variable ANSI cursor state before snapshot assertions.

6. Dependency and build config updates
   - If not already present, add `ratatui-testlib` and `insta` as
     dev-dependencies in `Cargo.toml`.
   - Keep versions as normalized workspace dependency style and match lockfile
     on commit.

7. Validation and completion
   - Run quality commands after changes:

     ```sh
     branch_name=$(git branch --show)
     make check-fmt | tee /tmp/check-fmt-frankie-${branch_name}.out
     make lint | tee /tmp/lint-frankie-${branch_name}.out
     make test | tee /tmp/test-frankie-${branch_name}.out
     ```

   - Include docs checks only if markdown changes are made.
   - Capture and inspect any snapshot diffs before commit.
   - Record test evidence and remaining risk in the final handoff.

## Surprises & discoveries

- The existing `calculate_list_height` function was already partly dynamic, but
  the startup path still depended on an explicit default of `(80, 24)` when
  `INITIAL_TERMINAL_SIZE` had not been populated.
- The PTY helper in `ratatui-testlib` allows only one `write_all` call per
  `TestTerminal` lifetime, so probe input is sent once per fixture instance and
  subsequent snapshots use resize-triggered redraws.

## Final decision log update

- Decision: update `get_initial_terminal_size` to query
  `crossterm::terminal::size()` when explicit initial dimensions are not
  available, so startup can still react to the real terminal in non-CLI
  entrypoints (e.g., tests and dedicated fixtures).

## Acceptance criteria

- Launch at terminal height above the current fixed cap displays more than 25
  rows where content exists.
- Layout expands/shrinks after terminal resize without stale row clipping.
- List and detail panes remain bounded and navigable at minimum terminal
  heights.
- Resize bug and regressions are covered with at least two Insta snapshot
  fixtures and one resize-step fixture.
- Required gates succeed (or failures are triaged and accepted only with
  explicit user approval).

## Revision note

- Updated after test execution to record successful PTY snapshot validation and
  to capture the `ratatui_testlib` one-write constraint in the fixture strategy.
- Updated after review remediation to record consolidation of line truncation
  helpers, timeout-failure hardening in `tui_resize_snapshot_fixture`, and a
  fresh full-gate pass.
