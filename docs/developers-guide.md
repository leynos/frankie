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

## Reply-template defaults

The canonical built-in reply templates live in `src/reply_template/defaults.rs`.
`DEFAULT_REPLY_TEMPLATES` is the source of truth and
`default_reply_templates()` derives its owned `Vec<String>` from that constant
so borrowed and owned callers cannot drift.

Configuration and TUI code consume `crate::reply_template` for these defaults.
Do not reintroduce copies under `src/config/` or `src/tui/`; those modules are
adapters that should depend inward on the reply-template domain module.

## Host-neutral summary references

Shared summary and navigation data transfer objects (DTOs) must use
host-neutral value objects. They must not expose CLI, TUI, Bubble Tea, or URI
rendering concerns in their core model fields or trait implementations.

PR-discussion summaries follow this convention through `ReviewViewRef` and
`ReviewView`. `DiscussionSummaryItem` stores the reference as structured data,
while `FrankieDeepLink` renders the current
`frankie://review-comment/<id>?view=detail` token for adapter surfaces that
need text output.

When adding a new summary or navigation target:

- put stable identity and logical destination data in the shared DTO;
- keep `Display` implementations that produce adapter text on presentation
  wrappers, not on the DTO itself;
- add serialization tests for the shared wire shape before changing CLI or TUI
  rendering.

## Spelling policy

Run `make spelling` to enforce en-GB-oxendict prose spelling. The generated
`typos.toml` starts from the shared estate dictionary, refreshes its untracked
local cache only when the authority is newer, and then applies the narrow
repository policy in `typos.local.toml`. Edit the local policy and regenerate
the configuration rather than changing generated entries by hand.

## Coverage ownership

The trunk owns both persistent coverage outputs. On a push to `main`,
`.github/workflows/coverage-main.yml` measures coverage, writes the ratchet
baseline, and uploads the report to CodeScene. Pull-request CI measures the
same selection only to compare it with that baseline: it archives no report,
never calls CodeScene, and never receives `CS_ACCESS_TOKEN`. The call is what
moves to the trunk, not the archive: the uploader pins the `cs-coverage`
archive by digest, but the client refuses to run whenever CodeScene's API
changes shape, and on the trunk such a change no longer fails every pull
request.

The publisher never binds the token in an `env` block, because the uploader is
a composite action that would pass a step's environment on to its nested steps.
A check step writes whether the secret is set, the upload runs only when it is
and only for `refs/heads/main`, and the token reaches the uploader solely as its
`access-token` input. Runs share one concurrency group per ref and are never
cancelled, so uploads land in commit order.

Two gaps are known and accepted. Merges made by the Dependabot automerge
workflow with `GITHUB_TOKEN` fire no push, so they reach the publisher only
through a dispatch or the next ordinary push. A dispatch that replaces a
pending push uploads the same or a newer commit, but the shared action writes
the baseline only on a push, so the baseline stays one push behind until the
next one.

`make workflow-contracts`, which pull-request CI runs, holds this shape in
`tests/workflow_contracts/`. It reads every workflow strictly (a repeated key
is an error) and follows local reusable-workflow calls transitively, so a
called workflow cannot reach CodeScene on a pull request's behalf.
