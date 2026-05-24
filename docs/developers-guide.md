# Developers guide

This guide indexes internal API and architecture notes for contributors. The
user-facing manual remains in [docs/users-guide.md](users-guide.md), while the
system design source of truth remains in
[docs/frankie-design.md](frankie-design.md).

## Time-travel service boundary

Time-travel orchestration lives in the shared library layer under
`src/time_travel/service.rs`. The TUI adapter starts asynchronous commands and
renders state, but it does not own the Git-backed navigation rules.

The public service surface is:

- `load_time_travel_state`, which materializes a `TimeTravelState` from
  `TimeTravelParams`, `GitOperations`, an optional head SHA, and a commit
  history limit.
- `navigate_time_travel_state`, which moves an already loaded
  `TimeTravelState` to the next newer or previous older commit.
- `TimeTravelNavigationDirection`, which names the navigation direction as
  `Next` or `Previous`.

`load_time_travel_state` clamps the commit history limit to at least one before
calling `GitOperations::get_parent_commits`. `navigate_time_travel_state`
returns `Ok(None)` when navigation is unavailable at a history boundary or
while the state is loading; it returns
`Result<Option<TimeTravelState>, GitOperationError>` so Git failures stay typed
at the library boundary.

TUI handlers in `src/tui/app/time_travel_handlers` delegate to these shared
functions from command closures, then translate successful or failed results
back into application messages. This keeps Bubble Tea, Tokio, and view-mode
state out of the service module.

Further detail:

- The library API overview is in
  [docs/users-guide.md](users-guide.md#library-api-time-travel-state).
- The architectural rationale and TUI adapter boundary are in
  [docs/frankie-design.md](frankie-design.md#227-extract-time-travel-orchestration-into-a-pure-library-service).
