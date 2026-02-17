# Snapshot Testing Bubbletea Terminal UIs with Insta

## Introduction to Snapshot Testing TUIs

Snapshot testing is a powerful technique to catch unintended UI changes by
comparing the current output of a program to a stored reference (the
“snapshot”). In the context of terminal UIs (TUIs) built with **bubbletea-rs**,
snapshots can ensure that refactors or feature changes don’t unintentionally
alter the interface. Bubbletea’s architecture follows an Elm-like
Model-View-Update pattern, so the UI is a deterministic function of the state.
This determinism makes it feasible to capture the rendered screen content and
use it as a golden reference. Instead of writing fragile assertions against
specific text or layout, the whole TUI output is captured once and then checked
to ensure future test runs produce the same output. This guide will walk
through setting up snapshot tests for bubbletea-rs using the **insta** crate,
including simulating user input (keypresses), structuring tests with **rstest**
and **rstest-bdd**, and dealing with common challenges like dynamic output and
terminal sizing.

**Why snapshot-test a TUI?**  Traditional assertions are tedious for TUIs –
every piece of text, whitespace, or colour would need to be checked manually.
Snapshot tests (also called golden file tests) capture the entire screen output
and allow differences to be reviewed when they occur. This is invaluable for
catching regressions: a small change in the `View` implementation (say, a
missing character or a layout shift) will cause the snapshot to differ,
prompting a closer look. For complex interactive TUIs, snapshot tests provide
broad coverage with minimal boilerplate. The trade-off is that *any*
intentional UI change will also break the test – the snapshot files must be
updated when those changes are accepted. As a result, snapshot tests are most
useful when the UI layout is relatively stable or when large refactors are in
progress and output stability matters.

## Test Strategy: Model vs. End-to-End

Multiple levels exist at which a Bubbletea TUI can be tested. In increasing
order of scope: (1) drive only the update logic (simulate messages and inspect
model state), (2) mix update calls with some direct model manipulation, (3)
test just the view output given a known model state, or (4) treat the entire
app as a black-box and simulate real terminal input and output. Fully
end-to-end tests (approach 4) treat the TUI like a user would – spawning the
program, sending actual keystrokes, and capturing screen bytes – but this can
be complex and flaky (timing issues, external terminal dependencies, etc.).
Instead, a pragmatic approach is to **simulate user interactions at the message
level and snapshot the view’s output**. This corresponds to a middle ground
between integration and unit testing. The Bubbletea **update loop** applies
messages to the model (just as it would at runtime), but a real terminal UI or
asynchronous event loop is not executed. Exercising the update logic and then
calling the model’s `view()` method yields a deterministic snapshot of the TUI
after a sequence of inputs. This approach provides confidence that the UI
reacts correctly to events (almost like an end-to-end test) while still running
fully in-memory and under controlled conditions, making it easier to enforce
determinism and test isolation.

**Tip:** If the Bubbletea app has heavy side effects or asynchronous commands
in its update logic, those side effects should be stubbed out or satisfied via
model injection to avoid nondeterminism. For example, if an update returns a
command that fetches network data or the current time, a pure snapshot test
should not actually perform that action. One approach is to design the model so
that such commands are optional or can be replaced with test doubles. This
guide focuses on the **synchronous model update and view rendering**, assuming
that any asynchronous commands are either disabled or their resulting messages
are simulated directly in tests.

## Recommended Layering: MVU Snapshots First, PTY Tests Second

The sweet spot for bubbletea-rs testing is usually **MVU snapshots**: drive the model with a sequence of messages, then snapshot `view()`. The framework deliberately makes this cheap: `Msg` is just `Box<dyn Any + Send>`, so tests can inject events without going anywhere near a real terminal.[^3] This gives you fast, deterministic tests that run well in CI.

When your `update` returns commands (`Cmd` futures), you can keep things deterministic in two ways. You can inject the “command result” message directly (treat commands as an implementation detail), or you can execute the command under a mocked dependency layer and feed any produced message back into `update`. The latter gives you higher confidence without going full black-box.

Finally, reserve a small number of **PTY-backed smoke tests** for the bits MVU tests cannot cover: raw mode, alternate screen transitions, key-encoding quirks, resize negotiation, and the very practical question of “does the compiled binary behave like a proper terminal program”. `ratatui_testlib` runs your real process inside a pseudo-terminal and lets you snapshot the emulated screen contents for visual regression testing.[^5]

Bubbletea-rs also ships a `DummyTerminal`, which is handy if you want to run the framework’s `Program` loop headlessly (for example, to exercise the built-in command scheduling) without touching a real terminal.[^4]

## Setting Up Insta for Bubbletea Snapshot Tests

First, add `insta` to the development dependencies, and install the companion
CLI tool for reviewing snapshots:

```toml
[dev-dependencies]
insta = { version = "1", features = ["filters"] }
rstest = "0.26"
rstest-bdd = "0.1.0-alpha4"

# Optional: PTY-backed black-box tests (runs your real binary in a pseudo-terminal)
ratatui-testlib = { version = "0.1", features = ["snapshot-insta"] }
```

```bash
cargo install cargo-insta
```

Ensure the `"filters"` feature for insta is enabled, as regex filters will be
used to redact dynamic text in snapshots. With these in place, tests can call
`insta::assert_snapshot!` on strings or debug representations. By default,
snapshot files (with extension `.snap`) are saved under a `snapshots/`
directory next to the test file.

Because bubbletea-rs UIs often include ANSI colour/style codes (via the
Lipgloss styling library), a conscious choice is needed for how they are
handled in snapshots. The insta crate captures the raw string including any
escape sequences. This is fine (it actually helps detect styling changes), but
it can reduce snapshot legibility. To strip ANSI codes for clarity, insta’s
filter feature can be used to remove them. For example, before asserting, add a
filter to delete ANSI escape sequences:

```rust
use insta::Settings;

let mut settings = Settings::new();
settings.add_filter(r"\x1B\[[0-9;]*[A-Za-z]", ""); // regex to match ANSI codes
settings.bind(|| {
    insta::assert_snapshot!(cleaned_output);
});
```

In practice, many developers leave colour codes in the snapshot, unless the
output becomes too noisy. The Ratatui project notes that its built-in snapshot
support doesn’t yet handle colour, effectively ignoring colour in
comparisons.[^1] If colours are not critical to the test, the view can also be
run with a “no-colour” setting (e.g., set the `NO_COLOR` env variable or
configure Lipgloss to disable colour) so that the output is plain text. This is
optional – just be consistent with whatever choice is made so snapshots remain
comparable.

Finally, when writing snapshot tests for TUIs, it is crucial to **fix the
terminal dimensions** during tests. The rendered output of many TUIs depends on
the terminal size (for wrapping text, layout, etc.). Without a specified size,
the program might default to whichever console size it last observed. For
reliable tests, simulate a known size (e.g. 80×24). In a Bubbletea app, this
typically means sending a `WindowSizeMsg` to the update function before
capturing the view. An example of this appears below. Using a consistent size
ensures reproducible results across different environments.

> **Note:** If tests are in an async context (for example, using
> `#[tokio::test]` because the update requires a runtime), enable the
> appropriate feature in rstest-bdd (such as `rstest-bdd-macros/tokio`). This
> allows scenario tests to run in a Tokio context as needed. Many Bubbletea
> commands are async, but when the `Program` itself is not executed, a full
> async test is often unnecessary – the model’s update and view can usually be
> called in a synchronous test because they just return data (commands that
> would have been awaited are returned as objects and can be ignored or
> executed manually if needed).

## Capturing Bubbletea TUI Output

In bubbletea-rs, a model’s `view()` method returns a `String` representing the
entire screen contents (including newlines and any ANSI styling). This makes
capturing output straightforward: calling `model.view()` directly in a test to
get the draw output. The key is to ensure the model is in the desired state
first. A typical snapshot test will look like:

```rust
use bubbletea_rs::{Model, WindowSizeMsg};

#[test]
fn main_menu_initial_render() {
    let (mut model, _cmd) = MyAppModel::init();

    // Simulate a terminal size of 80×24 (bubbletea-rs expects messages boxed as `Msg`)
    model.update(Box::new(WindowSizeMsg { width: 80, height: 24 }));

    insta::assert_snapshot!(model.view());
}
```

The steps are:

- **Initialize the model:** Construct a Bubbletea model in a known state. This
  might involve calling the model’s constructor or `init()` function. If
  `Model::init()` returns a `Cmd` (command) that kicks off background work, it
  may be preferable to avoid running that command in the test. Often, it is
  enough to ignore it. For example, if `MyAppModel::init()` returns something
  like a subscription or an HTTP fetch command, it can be dropped or replaced
  with a no-op in test configuration. The focus is on the visible state/output.

- **Set terminal size:** Bubbletea automatically sends a `WindowSizeMsg` to the
  program on startup to report terminal dimensions. In tests, this should be
  done manually. Construct a `WindowSizeMsg` with fixed dimensions and pass it
  to the model’s `update`. If `WindowSizeMsg` is not publicly exported, an
  equivalent message in the application can be used. If the model reads size
  from the environment, configure that source instead. The simplest approach is
  often to store width/height in the model upon receiving `WindowSizeMsg`, and
  supply one in tests. Using a fixed size (like 80×24) prevents snapshots from
  changing across different terminals or CI systems.

- **Render the view:** Call `model.view()` to obtain the screen content. This
  should be a pure function of the model’s state (in Bubbletea, view should not
  have side effects). The returned string contains everything the program would
  draw to the terminal at that moment. It may include multiple lines,
  box-drawing characters, etc., exactly as a user would see. If the application
  uses multiple frames (for animation) or alternate screens, note that `view()`
  is usually just the latest frame. Typically, a single snapshot is written for
  each test scenario, so choose the point in the interaction to capture (often
  the end state or an important intermediate state).

- **Assert snapshot:** Use `insta::assert_snapshot!` to compare the output
  against a saved snapshot. On the first run, this will create a new snapshot
  file (e.g. `my_app__main_menu_initial_render.snap`). On subsequent runs,
  insta will diff the current output with the file. If they differ, the test
  fails, and `cargo insta review` can be run to see the diff and decide whether
  to accept the changes. The snapshot file will contain the text exactly as
  printed by the TUI, line by line.[^2] It is a good idea to commit these
  `.snap` files to version control, as they represent the expected output.

**Tip:** If the model’s view output contains non-deterministic elements (for
example, a timestamp, a random number, or an ID that changes each run), those
elements must be **stabilized** for the snapshot to be useful. Several
approaches help achieve this:

- **Redactions/Filters:** Insta allows regex filters over the final string to
  replace volatile parts with placeholders. For instance, if the UI prints
  `Last updated at 2025-12-05 19:51`, a filter can replace the date/time with a
  fixed token like `<TIMESTAMP>`. This keeps the snapshot stable while still
  showing the surrounding content. Use
  `settings.add_filter(pattern, "replacement")` as shown earlier. Keep the
  patterns specific enough to avoid accidental matches of real text.

- **Deterministic seeding:** If randomness is involved (say the view shows a
  random quote or a spinner that picks a random frame), adjust the code for
  tests to use a fixed seed or sequence. For example, when using `rand`, seed
  the RNG with a constant in test mode. If the spinner rotates on each tick,
  the code can simulate exactly N tick updates so the displayed frame is known.
  The goal is to eliminate unpredictability. This may require adding test hooks
  or dependency-injecting an RNG. Many snapshot testing veterans create small
  helpers such as `fn now() -> Instant` that are overridden in tests to return
  a fixed time.

- **Ignore ephemeral UI elements:** Sometimes the easiest path is to omit
  certain dynamic elements from the snapshot. For example, if the TUI displays
  a live clock or progress percentage, that portion can be suppressed for
  tests. The view can be designed to omit or zero-out such information when a
  debug/test flag is set. This is more of a last resort, as it changes the
  behaviour under test — but it can be acceptable if those elements do not
  affect the rest of the layout and are verified via other means.

Preparing the model state carefully and cleaning any dynamic data keeps
snapshot comparisons meaningful and stable. As an illustration, the maintainers
of Bubble Tea’s Go version have a testing tool (`teatest`) that works
similarly: it feeds the program events and then supports golden-file
comparisons of the full output. In the Rust context here, insta plays the same
role – capturing the **entire TUI screen** for verification.

## Simulating User Inputs (Key Presses)

A snapshot test becomes much more powerful when sequences of user input are
simulated to drive the UI into various states. In bubbletea-rs, user
interactions (keypresses, mouse events, etc.) are delivered to the `update`
method as message types. Specifically, keystrokes arrive as `KeyMsg` messages
(which carry a `crossterm::event::KeyCode` plus any active `KeyModifiers`). To
simulate a key press in a test, create a `KeyMsg` and call
`model.update(Box::new(KeyMsg { ... }))`.

**Example:** Suppose pressing **“q”** in an app triggers a quit confirmation
dialog. In a test, the following applies:

```rust
use bubbletea_rs::{KeyMsg, WindowSizeMsg};
use crossterm::event::{KeyCode, KeyModifiers};

// ... inside a test (assume `model` already exists) ...
model.update(Box::new(WindowSizeMsg { width: 80, height: 24 })); // initial size
model.update(Box::new(KeyMsg {
    key: KeyCode::Char('q'),
    modifiers: KeyModifiers::NONE,
}));

insta::assert_snapshot!(model.view());
```

After the `KeyMsg` update, the model should have transitioned to the “quit
confirmation” state, and the view output will reflect that (e.g. showing
`"Quit? (y/N)"`). Snapshotting the output verifies that the correct prompt
appears on the screen after pressing "q". Multiple inputs can be chained in one
test to simulate a longer interaction.

For instance, consider testing a simple flow: open a menu, navigate, and select
an item. Arrow key presses and the Enter key can be simulated:

```rust
use bubbletea_rs::KeyMsg;
use crossterm::event::{KeyCode, KeyModifiers};

model.update(Box::new(KeyMsg { key: KeyCode::Down, modifiers: KeyModifiers::NONE }));
model.update(Box::new(KeyMsg { key: KeyCode::Down, modifiers: KeyModifiers::NONE }));
model.update(Box::new(KeyMsg { key: KeyCode::Enter, modifiers: KeyModifiers::NONE }));

insta::assert_snapshot!(model.view());
```

Each call to `update` feeds one input to the model, just as if the user pressed
a key. Use the correct `KeyCode` variants from `crossterm::event::KeyCode` for
special keys (e.g. `KeyCode::Up`, `KeyCode::Esc`, `KeyCode::Backspace`, etc.).
For keys with modifiers (Ctrl+C, etc.), include the `KeyModifiers`.
Bubbletea-rs keeps `KeyMsg` as a plain struct, so constructing key events is
straightforward. After simulating the sequence, call `view()` to get the final
screen.

If the application logic sends its own custom messages (for example, a message
indicating a task was added), those can also be simulated by calling `update`
with that message. Essentially, any `Msg` that the update can handle can be
injected in tests. This includes timer ticks or external events – create the
corresponding message struct and pass it to `update`. By driving the state
purely with messages, the test mirrors how the real program runs without
needing to spin up the full runtime. As one Bubble Tea testing article notes,
*“the test emulates the user pressing keys and checking that the program
responds in kind”* – the approach here does the same with bubbletea-rs.

A practical tip for simulating text input: If a TUI has a text field (e.g.
using the `TextInput` component from bubbletea-widgets) and the goal is to
simulate typing a word, each character needs to be sent as a separate `KeyMsg`.
A small helper can reduce repetition, such as:

```rust
fn send_text(model: &mut MyAppModel, text: &str) {
    for ch in text.chars() {
        let kc = KeyCode::Char(ch);
        model.update(Box::new(KeyMsg { key: kc, modifiers: crossterm::event::KeyModifiers::NONE }));
    }
}
```

In a test,
`send_text(&mut model, "hello"); model.update(Box::new(KeyMsg { key: KeyCode::Enter, modifiers: crossterm::event::KeyModifiers::NONE }));`
would simulate typing “hello” and pressing Enter. Snapshot the output to verify
that the input was handled (for example, the new item “hello” appears in a
list). Remember to simulate special keys like Enter or Tab as needed by the UI
flow.

By combining sequences of inputs, test code can script any user journey and the
resulting screen. If intermediate screens also matter, take snapshots at
multiple points – though that often means splitting into multiple tests (one
per significant step) or using multiple assertions in one test with distinct
names. Insta allows multiple snapshots in one test function if each assertion
is given a name, e.g. `assert_snapshot!("after_two_downs", model.view());` and
after another input, `assert_snapshot!("after_selection", model.view());`. Each
produces a separate `.snap` file. However, distinct test cases for different
end states are usually clearer unless the intermediate state is needed for
context.

One more consideration: Bubbletea’s update returns an `Option<Cmd>`. If the
update logic schedules asynchronous commands (like `Cmd::spawn` to do something
later), those commands will not run automatically in a unit test (because the
full program loop is not running). If the output of the view *depends* on a
command’s result, either invoke the command manually and then call `update`
with its resulting message, or refactor the logic so that the view reflects
only the model state and not immediate async results. In many cases, the
returned `Cmd` can be ignored in tests. But if pressing a key triggers a `Cmd`
that after 1 second sends a `TickMsg` which changes the UI, that tick can be
simulated by directly calling `update(Box::new(TickMsg))` in the test (instead of waiting
one second). This provides fine-grained control to advance the app state in a
deterministic way. The goal is to avoid real-time delays in tests – simulate
the passage of time or the completion of async tasks by injecting the
corresponding message.

## Injecting Mocks and Testing Commands Deterministically

Snapshot tests fall over when your model reaches out to the world (clock, RNG, filesystem, network). The easiest fix is architectural: make “the world” an explicit dependency, and keep `view()` a pure function of model state.

In bubbletea-rs, commands are just boxed futures (`Cmd`) that yield an optional `Msg`.[^6] That gives you a clean seam for mocking: construct your model with fake dependencies, trigger an update that returns a command, `await` it in the test, and feed any resulting message back into `update`.

A practical pattern is “ports and adapters”: define tiny traits for side-effect boundaries and inject them into the model via a constructor used by tests (while `Model::init()` can keep using production defaults).

```rust
use std::{future::Future, pin::Pin, sync::Arc};

use bubbletea_rs::{Cmd, KeyMsg, Model, Msg, WindowSizeMsg};
use crossterm::event::{KeyCode, KeyModifiers};

#[derive(Clone, Debug)]
struct ItemsLoaded(Vec<String>);

trait ItemRepo: Send + Sync {
    fn list(&self) -> Pin<Box<dyn Future<Output = Vec<String>> + Send>>;
}

fn fetch_items(repo: Arc<dyn ItemRepo>) -> Cmd {
    Box::pin(async move {
        let items = repo.list().await;
        Some(Box::new(ItemsLoaded(items)) as Msg)
    })
}

struct MyAppModel {
    repo: Arc<dyn ItemRepo>,
    items: Vec<String>,
}

impl MyAppModel {
    // Test-friendly constructor: inject mocks here.
    fn new(repo: Arc<dyn ItemRepo>) -> Self {
        Self { repo, items: vec![] }
    }
}

impl Model for MyAppModel {
    fn init() -> (Self, Option<Cmd>) {
        // Production default wiring (real repo, real clock, etc.)
        // Tests can bypass this and call `MyAppModel::new(...)`.
        (Self::new(Arc::new(/* RealRepo */ unimplemented!())), None)
    }

    fn update(&mut self, msg: Msg) -> Option<Cmd> {
        if let Some(key) = msg.downcast_ref::<KeyMsg>() {
            if key.key == KeyCode::Char('r') {
                return Some(fetch_items(self.repo.clone()));
            }
        }
        if let Some(ItemsLoaded(items)) = msg.downcast_ref::<ItemsLoaded>() {
            self.items = items.clone();
        }
        None
    }

    fn view(&self) -> String {
        format!("Items:\n{}", self.items.join("\n"))
    }
}

struct FakeRepo {
    items: Vec<String>,
}

impl ItemRepo for FakeRepo {
    fn list(&self) -> Pin<Box<dyn Future<Output = Vec<String>> + Send>> {
        let items = self.items.clone();
        Box::pin(async move { items })
    }
}

#[tokio::test]
async fn refresh_renders_loaded_items() {
    let repo = Arc::new(FakeRepo {
        items: vec!["alpha".into(), "beta".into()],
    });

    let mut model = MyAppModel::new(repo);
    model.update(Box::new(WindowSizeMsg { width: 80, height: 24 }));

    // Trigger refresh (returns a command)
    let cmd = model.update(Box::new(KeyMsg {
        key: KeyCode::Char('r'),
        modifiers: KeyModifiers::NONE,
    }));

    // Execute the command deterministically and feed its message back in
    if let Some(cmd) = cmd {
        if let Some(msg) = cmd.await {
            model.update(msg);
        }
    }

    insta::assert_snapshot!(model.view());
}
```

This looks like a lot of code in a guide, but the pay-off is huge: once you extract the “ports”, you can write MVU snapshots that cover real flows (including async commands) without touching the network, wall clock, or filesystem.

For **fully black-box PTY tests**, you can’t inject Rust trait objects directly, so give your app a configuration seam instead: flags/env like `--data-dir`, `--api-base-url`, `NO_COLOR=1`, `MYAPP_TEST_SEED=…`, or a `--clock=fixed` switch that disables live timestamps. That way the process-level test harness can still run deterministically.

## Structuring Tests with Rstest and BDD Scenarios

Using **rstest** fixtures and **rstest-bdd** can greatly improve the clarity
and reusability of test code. Rstest allows parameterized tests and reusable
fixtures, while rstest-bdd introduces a Given-When-Then style API that maps
well to describing user interactions. Here’s how they fit into this guide:

**Fixtures for Reusable Setup:** Define a fixture for a Bubbletea model that
handles common setup, such as initializing the model and applying a window
size. For example:

```rust
use rstest::fixture;
use bubbletea_rs::{Model, WindowSizeMsg};

#[fixture]
fn model() -> MyAppModel {
    let (mut model, _cmd) = MyAppModel::init();

    // Simulate an 80×24 terminal (avoid coupling snapshots to the developer’s terminal size)
    model.update(Box::new(WindowSizeMsg { width: 80, height: 24 }));

    model
}
```

Now any test that takes `model: MyAppModel` as an argument will get a fresh
initialized model with a known terminal size. This ensures test isolation (each
test gets its own state) and DRY setup.

**Parameterized Tests:** If similar scenarios differ only slightly (e.g.,
different input sequences or initial states), `#[rstest]` can parametrize them.
For instance, to test that pressing “h”, “j”, “k”, “l” in normal mode triggers
the same action as arrow keys (a Vim-style keybinding), write:

```rust
#[rstest]
#[case(KeyCode::Left, "left_arrow_output")]
#[case(KeyCode::Char('h'), "left_h_output")]
fn left_keybinds(mut model: MyAppModel, #[case] key: KeyCode, #[case] snapshot_name: &str) {
    model.update(Box::new(KeyMsg { key, modifiers: crossterm::event::KeyModifiers::NONE }));
    let output = model.view();
    insta::assert_snapshot!(snapshot_name, output);
}
```

This example uses `#[case]` to feed in different keys and an identifier to use
in the snapshot name. `insta::assert_snapshot!` allows specifying a manual name
for the snapshot – this is useful to avoid name collisions when one test
function is used for multiple cases. In this example, it will produce files
like `left_keybinds__left_arrow_output.snap` and
`left_keybinds__left_h_output.snap`, each containing the UI after pressing the
respective key. This pattern keeps the test code concise while covering
multiple inputs.

**Behaviour-Driven (Given-When-Then) Scenarios:** Rstest-bdd builds on fixtures
and enables more narrative tests. Under the hood, it uses Gherkin-style
*.feature* files and binds steps to Rust functions. A separate feature file is
optional; the macros can define steps directly. For example, consider a feature
file `tests/features/quit.feature`:

```gherkin
Feature: Quitting the app
  Scenario: User quits from main screen
    Given the app is at the main screen
    When the user presses "q"
    Then a quit confirmation dialog is shown
    And the dialog asks "Quit? (y/N)"
```

These steps can be implemented in Rust:

```rust
use rstest::fixture;
use rstest_bdd::{given, when, then, scenario};

use bubbletea_rs::{KeyMsg, Model, WindowSizeMsg};
use crossterm::event::{KeyCode, KeyModifiers};

#[fixture]
fn model() -> MyAppModel {
    let (mut m, _cmd) = MyAppModel::init();
    m.update(Box::new(WindowSizeMsg { width: 80, height: 24 }));
    m
}

#[given("the app is at the main screen")]
fn app_at_main_screen(model: MyAppModel) -> MyAppModel {
    // The fixture already provides an initialized model at the main screen.
    model
}

#[when("the user presses "q"")]
fn user_presses_q(model: &mut MyAppModel) {
    model.update(Box::new(KeyMsg {
        key: KeyCode::Char('q'),
        modifiers: crossterm::event::KeyModifiers::NONE,
    }));
}

#[then("a quit confirmation dialog is shown")]
fn quit_dialog_shown(model: &MyAppModel) {
    let output = model.view();
    assert!(output.contains("Quit?"), "Dialog text not found in output");
}

#[then("the dialog asks "Quit? (y/N)"")]
fn quit_dialog_correct(model: &MyAppModel) {
    insta::assert_snapshot!(model.view());
}

#[scenario(path = "tests/features/quit.feature", name = "User quits from main screen")]
fn quit_feature(model: MyAppModel) {
    // The scenario macro runs the bound steps; the body can stay empty.
}
```

Finally, bind the scenario to the feature:

```rust
#[scenario(path = "tests/features/quit.feature", name = "User quits from main screen")]
fn quit_feature(model: MyAppModel) {
    // The scenario macro will automatically run the given/when/then in order
    // using the step definitions above. The `model` fixture is shared across steps.
}
```

In the above, the `model: MyAppModel` fixture is passed into the scenario, and
rstest-bdd ensures the same instance flows through the Given, When, Then. Each
step function either takes `&mut MyAppModel` (if it modifies state) or
`&MyAppModel` (if it is just checking). A mix of assertions is used: a basic
`assert!(contains)` for one Then step and a full `assert_snapshot!` for the
final state. Only snapshots could be used (especially if the dialog output
spans multiple lines or has colours that need verification), or finer-grained
asserts can cover specific elements with snapshot reserved for the full screen.
Both approaches are valid and can complement each other.

**Note:** The scenario’s test function (annotated with `#[scenario]`) runs
after the steps, meaning by the time its body executes, all Given/When/Then
have completed. In the example, the body is empty because the checks were in
the Then steps. Additional verification can be performed in the scenario body
if needed. Each scenario appears as a separate test in `cargo test` output,
named after the scenario. The insta snapshot within will be named accordingly
(often including the scenario name or test function name – the name can always
be overridden in the macro if needed).

The advantage of using rstest-bdd is clarity: anyone reading the test can see
the narrative of the user interaction. It also encourages reusing fixtures (the
`model` in this case) and separating the action from the verification. We
could, for instance, have multiple scenarios reuse the same `when` step for
pressing "q" if they start from different states.

**Isolation:** Each scenario gets its own fresh fixture instances, so one
scenario’s state changes won’t leak into another. This is critical for snapshot
tests – if a previous test left the model in some mutated state or didn’t reset
a global, the next test’s snapshot might be inconsistent. Use fixtures to
manage setup/teardown if needed. For example, if a TUI writes to a file or uses
a global config, reset or stub those in a fixture. Snapshot tests should be
deterministic and independent.

## PTY-Based Black-Box Snapshot Testing with `ratatui_testlib`

MVU snapshots give you precision and speed, but they intentionally skip the terminal. When you need confidence in *terminal behaviour* (TTY detection, raw mode, alternate screen, resize negotiation, escape-sequence correctness), add a thin layer of PTY-backed integration tests.

`ratatui_testlib` provides a pseudo-terminal (PTY) plus a VT-style terminal emulator and an ergonomic harness API. It works even if your app is not built with Ratatui; the PTY/escape-sequence problem is framework-agnostic.[^5]

A good pattern is: keep most tests as MVU snapshots, then write a handful of PTY tests as “smoke tests” for critical flows.

```rust
use ratatui_testlib::{CommandBuilder, KeyCode, Modifiers, Result, TuiTestHarness};

#[test]
fn quit_flow_smoke_test() -> Result<()> {
    let mut harness = TuiTestHarness::builder()
        .with_size(80, 24)
        .build()?;

    // In an integration test (tests/...), Cargo exposes the built binary via env!().
    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_my_app"));
    cmd.env("NO_COLOR", "1");          // keep snapshots readable
    cmd.env("MYAPP_TEST_SEED", "1");   // your own determinism hook (recommended)
    harness.spawn(cmd)?;

    harness.wait_for_text("Main Menu")?;
    insta::assert_snapshot!("main_menu", harness.screen_contents());

    // Press "q" to open quit dialog
    harness.send_key(KeyCode::Char('q'))?;
    harness.wait_for_text("Quit?")?;
    insta::assert_snapshot!("quit_dialog", harness.screen_contents());

    // Press Enter (or "y", etc.) to exit
    harness.send_key(KeyCode::Enter)?;
    harness.wait_exit()?;

    Ok(())
}
```

A few practical tips that make these tests much less flaky:

- Fix the terminal size (the harness does this) and avoid relying on the developer’s real terminal dimensions.
- Prefer `wait_for_text(...)` / `wait_for(...)` over sleeps; let the harness poll until the UI reaches the expected state.
- Keep live clocks, spinners, and progress animations behind a test-mode switch (env/flag), or snapshot at a well-defined tick count.
- Use configuration seams for black-box mocking: point the binary at a temp data dir, a local stub server, or a “fixture file” so the test controls external state.

If you need modifier keys (Ctrl+C etc.), `send_key_with_modifiers` exists:

```rust
harness.send_key_with_modifiers(KeyCode::Char('c'), Modifiers::CTRL)?;
```

That lets you test the *real* key encoding and handling path, which MVU tests deliberately bypass.

## Using Insta Effectively (Redactions, Filters, Snapshot Organization)

With insta, beyond the basics of `assert_snapshot!`, several features help when
testing TUIs:

- **Snapshot names and organization:** By default, insta names the snapshot file
  based on the test function name (and scenario/parameter, if applicable). The
  name can be overridden by passing a string as the first argument to
  `assert_snapshot!`. For example, in a single test that interacts with and
  snapshots multiple screens:

```rust
assert_snapshot!("screen1_main_menu", output1);
// ... perform some actions ...
assert_snapshot!("screen2_after_delete", output2);
```

This will produce files like `my_test__screen1_main_menu.snap` and
`my_test__screen2_after_delete.snap`. Use descriptive names to identify what
each snapshot represents. In a BDD scenario, to incorporate scenario details,
include a placeholder in the feature and pass it as an argument to the Then
step (e.g., `Then the screen should match snapshot "after_delete"` and use that
string in the `assert_snapshot!`). Otherwise, the snapshot will likely use the
test name and a counter.

- **Redacting sensitive or irrelevant data:** Filters were covered earlier.
  Insta also supports structured **redactions** for serde-serializable data,
  but since the output here is a plain string, regex filters are the way to go.
  Common use cases:

- Redacting timestamps, as mentioned.

- Redacting random IDs or memory addresses if any appear in the UI.

- Normalizing whitespace if needed (though generally exact whitespace should be
  preserved in a TUI snapshot). For instance, if the UI draws a progress bar
  with changing lengths, the numeric percentage might be replaced with
  `[progress]` if verifying the exact percentage is not important for the test.

Filters can be defined globally for all tests by calling `Settings::add_filter`
at the beginning of the test module (or using `with_settings!` macro in insta
to wrap a block of snapshot assertions with certain settings). For example:

```rust
use insta::{with_settings, Settings};

#[test]
fn test_output() {
    let mut settings = Settings::new();
    settings.add_filter(r"\d\d\d\d-\d\d-\d\d \d\d:\d\d:\d\d", "<TIMESTAMP>");
    with_settings! { settings => {
        insta::assert_snapshot!(model.view());
    }}
}
```

This replaces any string that looks like a datetime `YYYY-MM-DD HH:MM:SS` with
`<TIMESTAMP>` before comparing or writing the snapshot. The filters prevent the
need to manually post-process the string in test code; the adjustment is
handled during the snapshot assertion.

- **Snapshot file location:** By default, insta creates a `snapshots`
  directory in the same folder as the test file (for a unit test in the main
  crate) or in the crate’s root for integration tests, with filenames derived
  from the test name. To keep all snapshots in a single place or adjust the
  path (say, when running in a workspace with multiple crates), use
  `Settings::set_snapshot_path`. For instance:

```rust
Settings::clone_current()
    .set_snapshot_path("../ui_snapshots")
    .bind(|| {
        assert_snapshot!(output);
    });
```

This tells insta to look in a directory relative to the current one. In most
cases, the default is fine. Organise tests so that each component or feature
has its own test module, which will naturally group the snapshots.

- **Reviewing and updating snapshots:** When a snapshot test fails (because the
  output changed from the saved snapshot), run `cargo insta review` to
  interactively review differences. This shows a coloured diff of expected vs
  actual output. For TUIs, this can highlight even subtle alignment changes. If
  the changes are intentional, approve them to update the `.snap` file. If not,
  investigate which commit/code caused the difference. Running
  `cargo insta review` as part of regular UI changes keeps snapshots accurate.
  When a lot of churn is anticipated (for example, after restyling the whole
  app), `cargo insta accept --all` can update snapshots quickly; still skim the
  diffs to ensure everything looks correct. Remember that **any** change in the
  output, even whitespace or ANSI codes, will appear in the diff. This
  strictness is what gives snapshot tests their power.

One caveat to bear in mind: because snapshot tests assert the entire screen
content, if the UI changes frequently (e.g., dynamic layouts), snapshots may be
updated often. The Ratatui documentation suggests reviewing snapshots only
after significant updates to avoid CI noise. In practical terms, strike a
balance – do not disable the tests, but group minor cosmetic changes so
multiple snapshots are updated in one go. Also, consider writing more focused
tests for critical logic (like “pressing X increases the counter by 1” as a
unit test on the model state) and reserve snapshot tests for verifying the
*presentation* of that state.

## Handling Non-Deterministic Elements and Warnings

Snapshot testing Bubbletea UIs does come with some challenges, but they can be
managed:

- **Timing-dependent behaviour:** If the UI has animations or time-driven
  changes (spinners, clocks, auto-refresh lists, etc.), ensure the test either
  freezes time or captures a specific moment. For example, if an animated
  spinner advances every 200ms via a tick `Msg`, decide whether to test the
  initial state (no ticks applied) or after a certain number of ticks.
  “Fast-forwarding” can be simulated by calling update with a tick message
  multiple times. Alternatively, for things like an ASCII spinner, simply
  exclude it from test verification if it is not important (or assert that it
  is one of the known spinner frames, rather than performing a snapshot on it).
  The key is to avoid races or sleep calls in tests. All inputs and events
  should be fed synchronously.

- **Randomness:** Seeding random number generators was covered earlier. Another
  pattern is to use dependency injection for any random or external data. If
  the view calls a function to get a random quote, tests can override that
  function (perhaps by configuring the model with a predictable quote
  provider). The fewer unpredictable sources, the better. It is acceptable to
  use insta’s redactions to blank out truly random strings and just confirm the
  rest of the layout. Some test rigour is lost (the exact random content is not
  checked), but layout validation is preserved.

- **External resources:** If a TUI prints data fetched from a server or file,
  the test should not rely on the real resource. Use test doubles or sample
  data. For example, if on startup the app loads a config file and displays
  some values, the model in tests can use a temp file or dummy config instead.
  The snapshot then contains that dummy data. The snapshot essentially asserts
  that whatever data is present is correctly rendered – so as long as the
  structure is the same, using fake data is fine.

- **Terminal quirks:** Bubbletea (like many TUIs) uses special control codes for
  tasks such as hiding the cursor, clearing the screen, or switching to an
  alternate screen. When calling `model.view()` directly, the returned string
  is typically just the content, not those setup/teardown codes (since those
  are handled by the `Program` when running for real). If stray characters
  appear in a snapshot that correspond to such codes, they can be filtered out.
  In most cases they do not appear because `view()` returns only what is drawn
  (e.g., via Lipgloss or text strings). The **bubbletea-rs** `Program` handles
  terminal initialization (entering alternate screen, etc.), which is bypassed
  in these tests. That is beneficial because the snapshots focus purely on UI
  content.

- **Platform differences:** Aim for identical output across platforms. If
  characters might not render the same (for example, Windows console might not
  handle certain Unicode), the snapshot will reflect whichever environment ran
  the test. Using standard UTF-8 characters and ANSI escapes is usually fine
  across OSes when a consistent C locale is used. If CI and local environments
  differ (say, line-ending differences or locale issues that change unicode
  icons), normalization may be needed (for instance, always output `\n` as line
  separator, and open files in text mode accordingly). This is generally not a
  problem, but it is worth remembering if a snapshot passes locally but fails
  on CI because of an encoding issue.

- **Updating snapshots vs. test assertions:** A golden-file test alerts to
  *any* change, but does not judge whether that change is desirable – that
  decision sits with the reviewer. For experienced developers, it is often
  clear when a diff is expected (e.g., an intentional label change) versus a
  regression. Be disciplined: if a diff appears unexpectedly, investigate the
  code because something subtle might have broken. This is where snapshot tests
  shine: they can catch UI regressions that wouldn’t crash the program but
  would degrade user experience. For example, a refactor might accidentally
  remove a highlight or misalign text. A snapshot test failure shows the
  before/after of the UI, prompting attention to the issue.

One concrete example: a developer retrofitting snapshot tests for a Bubbletea
app noted that the process forced deeper thought about the app’s architecture
and state handling. That process often reveals where side effects should be
separated from pure updates, or where code can be reorganized to be more
testable. Embracing those improvements leaves the TUI code cleaner and more
maintainable.

Finally, keep in mind that snapshot tests complement other testing methods;
they shouldn’t be the only tests. They cover “did the UI look as expected” very
well, but they don’t directly tell why a change happened. If a snapshot test
fails, a quick unit test or debugging of the model’s state transitions can
pinpoint the bug. For logic-heavy components, traditional assertions on the
model state can be simpler and more robust. Use snapshot tests when verifying
the drawn output is important – layout, text content, etc., especially in
combination with multiple inputs where writing individual assertions would be
laborious.

## Running the Tests and Interpreting Results

Once snapshot tests have been written, run them with `cargo test`. The first
run (or whenever new tests are added) will create initial `.snap` files. The
files should be inspected to verify the captured screen content. If a test
fails due to a snapshot mismatch, run `cargo insta review` to see the
differences side by side. Running `cargo insta review --accept` (or pressing
the accept key in interactive mode) approves new snapshots when the change is
intended. Committing the updated snapshots to version control makes future test
runs use those as the baseline.

In CI, snapshot tests typically run as part of `cargo test`. If there is a
failure, the CI logs will show which snapshot did not match. CI artifacts can
include the new snapshot suggestions for manual download and inspection.
However, it is often easier to reproduce the failure locally, run the review,
and then update the files.

**Example output:** Suppose the border character in the UI changes from `│` to
`|`. A snapshot diff might look like:

```diff
 - │ Item 1
 - │ Item 2
 + | Item 1
 + | Item 2
```

This small difference would fail the test. If it is a regression (the box-drawn
border was meant to be retained), the view code needs adjustment. If the change
was intentional (perhaps simplifying to ASCII), the change is accepted and the
new snapshot will contain the `|`. The snapshot review diff is effectively a
visual check of the TUI, almost like viewing the UI side-by-side before and
after.

As a rule of thumb, snapshots can serve as living documentation of the TUI.
Reading through a `.snap` file should give a reasonable picture of what the
screen looks like (even though colour codes and some alignment might be harder
to grok in raw text). Some developers even include representative snapshot
files in their docs or pull requests to show what the UI output is. Since insta
stores snapshots as plain text, they work well for this purpose.

## Conclusion

Snapshot testing a Bubbletea-rs application with insta allows verification of
terminal UI output with confidence and ease. By capturing the full-screen state
after a series of simulated inputs, a robust regression test is created that
flags any unintended UI change. The guide covers how to set up a stable test
environment (fixed terminal size, controlled inputs), how to integrate with
rstest’s powerful fixture and BDD syntax for clarity, and how to handle tricky
dynamic aspects via insta’s redactions/filters. The result is a suite of tests
that act as a safety net for the TUI: refactor the code fearlessly, and let the
snapshots indicate whether anything looks different.

Keep in mind that snapshot tests are most effective when curated – focus on key
states of the UI (no need to snapshot every possible screen if it is not
necessary), and keep dynamic data in check. When used appropriately, they can
be **“golden files”** for a project’s behaviour, providing quick feedback on
changes. As the UI evolves, update the snapshots intentionally and ensure they
remain up-to-date with the expected output.

By addressing determinism (for example, seeding randoms and fixing timestamps)
and isolating each test scenario, tests run reliably in CI and avoid flaky
failures. Each test effectively reproduces a user’s journey in a controlled
way. This approach is reminiscent of end-to-end tests but executed at the
program logic level, striking a good balance between coverage and
maintainability.

In summary: *leverage insta to assert a Bubbletea app’s text-based UI just as a
data structure would be asserted*. The benefits include quick diffing and
approval workflow, with the rich semantic context of seeing terminal UI
content. When a test fails, the change in the UI is immediately visible.
Combined with rstest-bdd, test code can read almost like a specification of the
UI’s behaviour. This not only helps catch bugs but also serves as documentation
for how the TUI is supposed to react to input.

[^1]: Ratatui snapshot testing note on colour handling:
      <https://ratatui.rs/recipes/testing/snapshots/#:~:text=Note>
[^2]: Ratatui snapshot recipe examples showing line-by-line snapshots:
      <https://ratatui.rs/recipes/testing/snapshots/#:~:text=snapshots%2Fdemo2__tests__render_app>
       and
      <https://ratatui.rs/recipes/testing/snapshots/#:~:text=,Traceroute%20%20Weather>
[^3]: bubbletea-rs defines `Msg` as `Box<dyn Any + Send>` (messages must be boxed):
      <https://docs.rs/bubbletea-rs/latest/bubbletea_rs/event/type.Msg.html>
[^4]: bubbletea-rs `DummyTerminal` (headless terminal implementation for tests):
      <https://docs.rs/bubbletea-rs/latest/bubbletea_rs/terminal/struct.DummyTerminal.html>
[^5]: ratatui-testlib overview and `TuiTestHarness` API (PTY-based integration testing with insta support):
      <https://docs.rs/ratatui-testlib/latest/ratatui_testlib/>
       and
      <https://docs.rs/ratatui-testlib/latest/ratatui_testlib/struct.TuiTestHarness.html>
[^6]: bubbletea-rs defines `Cmd` as a boxed future returning `Option<Msg>`:
      <https://docs.rs/bubbletea-rs/latest/bubbletea_rs/command/type.Cmd.html>

**Sources:**

- bubbletea-rs documentation (Model/Msg/Cmd and DummyTerminal)
- ratatui-testlib documentation (TuiTestHarness and PTY-based integration testing)
- Ratatui snapshot testing recipe (buffer-based snapshot testing with insta)
- Charm’s Bubble Tea teatest (Go) golden-file testing inspiration
- Insta documentation (filters/redactions and snapshot review workflow)
