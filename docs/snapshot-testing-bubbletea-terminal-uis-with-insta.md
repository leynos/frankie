# Snapshot Testing Bubbletea Terminal UIs with Insta

## Introduction to Snapshot Testing TUIs

Snapshot testing is a powerful technique to catch unintended UI changes by
comparing the current output of your program to a stored reference (the
“snapshot”). In the context of terminal UIs (TUIs) built with **bubbletea-rs**,
snapshots can ensure that refactors or feature changes don’t unintentionally
alter your interface. Bubbletea’s architecture follows an Elm-like
Model-View-Update pattern, so the UI is a deterministic function of the state.
This determinism makes it feasible to capture the rendered screen content and
use it as a golden reference. Instead of writing fragile assertions against
specific text or layout, you capture the whole TUI output once and then simply
check that future test runs produce the same output. This guide will walk
through setting up snapshot tests for bubbletea-rs using the **insta** crate,
including simulating user input (keypresses), structuring tests with **rstest**
and **rstest-bdd**, and dealing with common challenges like dynamic output and
terminal sizing.

**Why snapshot-test a TUI?**  Traditional assertions are tedious for TUIs –
you’d have to check every piece of text, whitespace, or color manually.
Snapshot tests (also called golden file tests) capture the entire screen output
and let you review differences when they occur. This is invaluable for catching
regressions: a small change in your `View` implementation (say, a missing
character or a layout shift) will cause the snapshot to differ, alerting you to
inspect the change. For complex interactive TUIs, snapshot tests provide broad
coverage with minimal boilerplate. The trade-off is that *any* intentional UI
change will also break the test – so you’ll need to update the snapshot files
when you accept those changes. As a result, snapshot tests are most useful when
your UI layout is relatively stable or when you’re doing large refactors and
want to ensure the output stays consistent (or changes only in expected ways).

## Test Strategy: Model vs. End-to-End

There are multiple levels at which you can test a Bubbletea TUI. In increasing
order of scope: (1) drive only the update logic (simulate messages and inspect
model state), (2) mix update calls with some direct model manipulation, (3)
test just the view output given a known model state, or (4) treat the entire
app as a black-box and simulate real terminal input and output. Fully
end-to-end tests (approach 4) treat the TUI like a user would – spawning the
program, sending actual keystrokes, and capturing screen bytes – but this can
be complex and flaky (timing issues, external terminal dependencies, etc.).
Instead, a pragmatic approach is to **simulate user interactions at the message
level and snapshot the view’s output**. This corresponds to a middle ground
between integration and unit testing. We let the Bubbletea **update loop**
apply messages to the model (just as it would at runtime), but we don’t run a
real terminal UI or asynchronous event loop. By exercising the update logic and
then calling the model’s `view()` method to get the drawn UI, we get a
deterministic snapshot of the TUI after a sequence of inputs. This approach
gives us confidence that the UI reacts correctly to events (almost like an
end-to-end test) while still running fully in-memory and under our control
(making it easier to enforce determinism and test isolation).

**Tip:** If your Bubbletea app has heavy side-effects or asynchronous commands
in its update logic, you might need to stub those out or use model injection to
avoid nondeterminism. For example, if an update returns a command that fetches
network data or the current time, a pure snapshot test should not actually
perform that action. One way is to design your model so that such commands are
optional or can be replaced with test doubles. In our testing strategy, we’ll
focus on the **synchronous model update and view rendering**, assuming that any
asynchronous commands are either disabled or their resulting messages can be
simulated directly in tests.

## Setting Up Insta for Bubbletea Snapshot Tests

First, add `insta` to your development dependencies, and install the companion
CLI tool for reviewing snapshots:

```toml
[dev-dependencies]
insta = { version = "1.31.0", features = ["filters"] }
rstest = "0.26"
rstest-bdd = "0.1.0-alpha4"
```

```bash
cargo install cargo-insta
```

Ensure you enable the `"filters"` feature for insta, as we’ll use regex filters
to redact dynamic text in snapshots. With these in place, you can write tests
that call `insta::assert_snapshot!` on strings or debug representations. By
default, snapshot files (with extension `.snap`) will be saved under a
`snapshots/` directory next to your test file.

Because bubbletea-rs UIs often include ANSI color/style codes (via the Lipgloss
styling library), decide how you want to handle them in snapshots. The insta
crate will capture the raw string including any escape sequences. This is fine
(it actually lets you detect styling changes), but it can make the snapshot
hard to read. If you prefer to strip ANSI codes for clarity, you can use
insta’s filter feature to remove them. For example, before asserting, add a
filter to delete ANSI escape sequences:

```rust
use insta::Settings;

let mut settings = Settings::new();
settings.add_filter(r"\x1B\\[[0-9;]*[A-Za-z]", ""); // regex to match ANSI codes
settings.bind(|| {
    insta::assert_snapshot!(cleaned_output);
});
```

In practice, many developers leave color codes in the snapshot, unless the
output becomes too noisy. (The Ratatui project notes that its built-in snapshot
support doesn’t yet handle color, effectively ignoring color in
comparisons([1](https://ratatui.rs/recipes/testing/snapshots/#:~:text=Note)).)
If colors are not critical to your test, you can also run your view with a
“no-color” setting (e.g., set the `NO_COLOR` env variable or configure Lipgloss
to disable color) so that the output is plain text. This is optional – just be
consistent with whatever choice you make so your snapshots remain comparable.

Finally, when writing snapshot tests for TUIs, it’s crucial to **fix the
terminal dimensions** during tests. The rendered output of many TUIs depends on
the terminal size (for wrapping text, layout, etc.). If you don’t specify a
size, the program might default to your current console size or a previous
value. For reliable tests, simulate a known size (e.g. 80×24). In a Bubbletea
app, this typically means sending a `WindowSizeMsg` to your update function
before capturing the view. We’ll show an example of this. Using a consistent
size ensures reproducible results across different environments.

> **Note:** If your tests are in an async context (for example, using
> `#[tokio::test]` because your update may require a runtime), make sure to
> enable the appropriate feature in rstest-bdd (such as
> `rstest-bdd-macros/tokio`). This will allow scenario tests to run in a Tokio
> context as needed. Many Bubbletea commands are async, but if we’re not
> actually running the `Program`, you might not need a full async test – often,
> you can call your model’s update and view in a normal sync test since they
> just return data (commands that would have been awaited are returned as
> objects and can be ignored or manually executed if needed).

## Capturing Bubbletea TUI Output

In bubbletea-rs, your Model’s `view()` method returns a `String` representing
the entire screen contents (including newlines and any ANSI styling). This
makes capturing output straightforward: **we can call `model.view()` directly
in a test to get the draw output**. The key is to ensure the model is in the
desired state first. A typical snapshot test will look like:

```rust
#[test]
fn main_menu_initial_render() {
    let mut model = MyAppModel::new(); // Create your Model (initial state)
    model.update(WindowSizeMsg { width: 80, height: 24 }); // Simulate a terminal size of 80x24
    let output = model.view();
    insta::assert_snapshot!(output);
}
```

Let’s break that down:

- **Initialize the model:** Construct your Bubbletea model in a known state.
  This might involve calling the model’s constructor or `init()` function. If
  your `Model::init()` returns a `Cmd` (command) that kicks off background
  work, you may want to avoid actually running that command in the test. Often,
  it’s enough to ignore it. For example, if `MyAppModel::init()` returns
  something like a subscription or an HTTP fetch command, you can drop it or
  replace it with a no-op in test configuration. The focus is on the visible
  state/output.

- **Set terminal size:** Bubbletea automatically sends a `WindowSizeMsg` to the
  program on startup to let the model know the terminal dimensions. In tests,
  you should do this manually. Construct a `WindowSizeMsg` with fixed
  dimensions and pass it to your model’s `update`. If `WindowSizeMsg` is not
  publicly exported, you may have a similar message in your app (some models
  define their own size message). Alternatively, if your model doesn’t
  explicitly handle a size message, it might be reading the size from the
  environment; in that case, you could set env vars or override how size is
  determined. The simplest approach, however, is to modify your app to store
  width/height in the model upon receiving `WindowSizeMsg`, and ensure to feed
  one in tests. By using a fixed size (like 80×24), your snapshot won’t change
  if you run tests on different terminals or CI systems.

- **Render the view:** Call `model.view()` to get the screen content. This
  should be a pure function of the model’s state (in Bubbletea, view should not
  have side effects). The returned string will contain everything your program
  would draw to the terminal at that moment. It may include multiple lines,
  box-drawing characters, etc., exactly as a user would see. If your app uses
  multiple frames (for animation) or alternate screens, note that `view()` is
  usually just the latest frame. We typically write one snapshot per test
  scenario, so decide which point in the interaction you want to capture (often
  the end state or an important intermediate state).

- **Assert snapshot:** Use `insta::assert_snapshot!` to compare the output
  against a saved snapshot. On the first run, this will create a new snapshot
  file (e.g. `my_app__main_menu_initial_render.snap`). On subsequent runs,
  insta will diff the current output with the file. If they differ, the test
  fails, and you can run `cargo insta review` to see the diff and decide
  whether to accept the changes. The snapshot file will contain the text
  exactly as printed by your TUI, line by
  line([1](https://ratatui.rs/recipes/testing/snapshots/#:~:text=snapshots%2Fdemo2__tests__render_app))([1](https://ratatui.rs/recipes/testing/snapshots/#:~:text=,Traceroute%20%20Weather)).
   It’s a good idea to commit these `.snap` files to your VCS, as they
  represent the expected output.

**Tip:** If your model’s view output contains non-deterministic elements (for
example, a timestamp, a random number, or an ID that changes each run), you
must **stabilize** those for the snapshot to be useful. There are a few ways to
do this:

- **Redactions/Filters:** Insta allows regex filters over the final string to
  replace volatile parts with placeholders. For instance, if your UI prints
  `Last updated at 2025-12-05 19:51`, you could add a filter to replace the
  date/time with a fixed token like `<TIMESTAMP>`. This way, the snapshot will
  contain `<TIMESTAMP>` instead of an actual date, and it won’t fail every time
  the date changes. Use `settings.add_filter(pattern, "replacement")` as shown
  earlier. Keep the patterns specific enough to not accidentally match real
  text.

- **Deterministic seeding:** If randomness is involved (say your view shows a
  random quote or a spinner that picks a random frame), modify your code for
  tests to use a fixed seed or sequence. For example, if using `rand`, seed the
  RNG with a constant in test mode. Or if the spinner rotates on each tick, you
  can simulate exactly N tick updates so you know which frame is shown. The
  goal is to eliminate unpredictability. It may require adding test hooks or
  dependency-injecting an RNG. Many snapshot testing veterans create small
  hooks in their code such as `fn now() -> Instant` that they can override in
  tests to return a fixed time.

- **Ignore ephemeral UI elements:** Sometimes the easiest path is to not
  include certain dynamic elements in the snapshot at all. For example, if your
  TUI displays a live clock or progress percentage, you might choose not to
  snapshot that portion. Design your view to omit or zero-out such info when a
  debug/test flag is set. This is more of a last resort, as it changes the
  behavior under test; but it can be acceptable if those elements don’t affect
  the rest of the layout and you verify them via other means.

By preparing your model state carefully and cleaning any dynamic data, you
ensure that snapshot comparisons are meaningful and stable. As an illustration,
the maintainers of Bubble Tea’s Go version have a testing tool (`teatest`) that
works similarly: it feeds the program events and then supports golden-file
comparisons of the full output. In our Rust scenario, we’re doing this manually
with insta, but the effect is the same – we capture the **entire TUI screen**
for verification.

## Simulating User Inputs (Key Presses)

A snapshot test becomes much more powerful when you can simulate sequences of
user input to drive the UI into various states. In bubbletea-rs, user
interactions (keypresses, mouse events, etc.) are delivered to your `update`
method as message types. Specifically, keystrokes arrive as `KeyMsg` messages
(which contain a `crossterm::event::KeyEvent` with a KeyCode and modifiers). To
simulate a key press in a test, you can directly create a `KeyMsg` and call
`model.update(Msg::from(KeyMsg))`.

**Example:** Suppose pressing **“q”** in your app triggers a quit confirmation
dialog. In a test, you could do:

```rust
use bubbletea_rs::event::KeyMsg;
use crossterm::event::{KeyCode, KeyModifiers};

// ... inside test ...
model.update(WindowSizeMsg { width: 80, height: 24 }); // initial size
model.update(KeyMsg::new(KeyCode::Char('q'), KeyModifiers::NONE));
// (Assuming KeyMsg::new or similar constructor exists; otherwise construct the struct)
let output = model.view();
insta::assert_snapshot!(output);
```

After the `KeyMsg` update, the model should have transitioned to the “quit
confirmation” state, and the view output will reflect that (e.g. showing
`"Quit? (y/N)"`). By snapshotting the output, we verify that the correct prompt
appears on the screen after pressing "q". You can chain multiple inputs in one
test to simulate a longer interaction.

For instance, consider testing a simple flow: open a menu, navigate, and select
an item. You could simulate arrow key presses and the Enter key:

```rust
model.update(KeyMsg::from(KeyCode::Down)); // move selection down
model.update(KeyMsg::from(KeyCode::Down)); // move down again
model.update(KeyMsg::from(KeyCode::Enter)); // activate selection
let output = model.view();
assert_snapshot!(output);
```

Each call to `update` feeds one input to the model, just as if the user pressed
a key. Make sure to use the correct `KeyCode` variants from
`crossterm::event::KeyCode` for special keys (e.g. `KeyCode::Up`,
`KeyCode::Esc`, `KeyCode::Backspace`, etc.). For keys with modifiers (Ctrl+C,
etc.), include the `KeyModifiers`. Bubbletea-rs might provide some ergonomic
constructors for common keys (for example, a `KeyMsg::ctrl_c()` helper), but
constructing them manually is straightforward. After simulating the sequence,
call `view()` to get the final screen.

If your app logic sends its own custom messages (for example, a message
indicating a task was added), you can also simulate those by calling `update`
with that message. Essentially, any `Msg` that your update can handle can be
injected in tests. This includes timer ticks or external events – you can
create the corresponding message struct and pass it to `update`. By driving the
state purely with messages, you’re exactly mirroring how the real program runs,
without needing to spin up the full runtime. As one Bubble Tea testing article
notes, *“the test emulates the user pressing keys and checking that the program
responds in kind”* – our approach does the same with bubbletea-rs.

A practical tip for simulating text input: If your TUI has a text field (e.g.
using the `TextInput` component from bubbletea-widgets), and you want to
simulate typing a word, you’ll need to send each character as a separate
`KeyMsg`. It can be helpful to write a small helper, such as:

```rust
fn send_text(model: &mut MyAppModel, text: &str) {
    for ch in text.chars() {
        let kc = KeyCode::Char(ch);
        model.update(KeyMsg::new(kc, KeyModifiers::NONE));
    }
}
```

Then in your test,
`send_text(&mut model, "hello"); model.update(KeyMsg::from(KeyCode::Enter));`
would simulate typing “hello” and pressing Enter. Snapshot the output to verify
that the input was handled (for example, the new item “hello” appears in a
list). Remember to also simulate special keys like Enter, Tab, etc., as needed
by your UI flow.

By combining sequences of inputs, you can script any user journey and assert
the final screen. If intermediate screens are also important, you can take
snapshots at multiple points – though that often means splitting into multiple
tests (one per significant step) or using multiple assertions in one test with
distinct names. Insta allows multiple snapshots in one test function if you
give each a name, e.g. `assert_snapshot!("after_two_downs", model.view());` and
then after another input, `assert_snapshot!("after_selection", model.view());`.
Each will produce a separate `.snap` file. However, a cleaner approach is
usually to have distinct test cases for different end states unless the
intermediate state is needed for context.

One more consideration: Bubbletea’s update returns an `Option<Cmd>`. If your
update logic schedules asynchronous commands (like `Cmd::spawn` to do something
later), those commands won’t run automatically in our test (since we’re not
running the full program loop). If the output of your view *depends* on a
command’s result, you have two choices: either invoke the command manually and
then call update with its resulting message, or better, refactor the logic so
that the view reflects only the model state and not immediate async results. In
many cases, you can ignore the returned `Cmd` in tests. But if, say, pressing a
key triggers a `Cmd` that after 1 second sends a `TickMsg` which changes the
UI, you might simulate that by directly calling `update(TickMsg)` in your test
(instead of actually waiting one second). This gives you fine-grained control
to advance the app state in a deterministic way. The goal is to avoid real time
delays in tests – simulate the passage of time or the completion of async tasks
by injecting the corresponding message.

## Structuring Tests with Rstest and BDD Scenarios

Using **rstest** fixtures and **rstest-bdd** can greatly improve the clarity
and reusability of your test code. Rstest allows parameterized tests and
reusable fixtures, while rstest-bdd introduces a Given-When-Then style API that
maps well to describing user interactions. Here’s how you can leverage them in
our context:

**Fixtures for Reusable Setup:** You can define a fixture for your Bubbletea
model that handles common setup, such as initializing the model and applying a
window size. For example:

```rust
use rstest::fixture;
use bubbletea_rs::event::KeyMsg;
use crossterm::event::KeyCode;

#[fixture]
fn model() -> MyAppModel {
    let mut model = MyAppModel::new();
    // Assume our model handles a WindowSizeMsg; simulate 80x24 terminal
    model.update(bubbletea_rs::WindowSizeMsg { width: 80, height: 24 });
    model
}
```

Now any test that takes `model: MyAppModel` as an argument will get a fresh
initialized model with a known terminal size. This ensures test isolation (each
test gets its own state) and DRY setup.

**Parameterized Tests:** If you have similar scenarios with slight variations
(e.g., different input sequences or different initial states), you can use
`#[rstest]` to parametrize them. For instance, suppose you want to test that
pressing “h”, “j”, “k”, “l” in normal mode triggers the same action as arrow
keys (a Vim-style keybinding). You could write:

```rust
#[rstest]
#[case(KeyCode::Left, "left_arrow_output")]
#[case(KeyCode::Char('h'), "left_h_output")]
fn left_keybinds(model: MyAppModel, #[case] key: KeyCode, #[case] snapshot_name: &str) {
    model.update(KeyMsg::from(key));
    let output = model.view();
    insta::assert_snapshot!(snapshot_name, output);
}
```

Here we use `#[case]` to feed in different keys and an identifier to use in the
snapshot name. `insta::assert_snapshot!` allows specifying a manual name for
the snapshot – this is useful to avoid name collisions when one test function
is used for multiple cases. In this example, it will produce files like
`left_keybinds__left_arrow_output.snap` and
`left_keybinds__left_h_output.snap`, each containing the UI after pressing the
respective key. This pattern keeps the test code concise while covering
multiple inputs.

**Behavior-Driven (Given-When-Then) Scenarios:** Rstest-bdd builds on fixtures
and lets you write more narrative tests. Under the hood, it uses Gherkin-style
*.feature* files and binds steps to Rust functions. If you prefer not to write
a separate feature file, you can still use the macros to define steps. For
example, imagine a feature file `tests/features/quit.feature`:

```gherkin
Feature: Quitting the app
  Scenario: User quits from main screen
    Given the app is at the main screen
    When the user presses "q"
    Then a quit confirmation dialog is shown
    And the dialog asks "Quit? (y/N)"
```

You can implement these steps in Rust:

```rust
use rstest_bdd::{given, when, then, scenario};
use bubbletea_rs::event::KeyMsg;
use crossterm::event::{KeyCode, KeyModifiers};

#[fixture]
fn model() -> MyAppModel {
    let mut m = MyAppModel::new();
    m.update(bubbletea_rs::WindowSizeMsg { width: 80, height: 24 });
    m
}

#[given("the app is at the main screen")]
fn app_at_main_screen(mut model: MyAppModel) -> MyAppModel {
    // The fixture already provided an initialized model at main menu
    // If needed, we could navigate to main screen here. In this case, it's already there.
    model
}

#[when("the user presses \"q\"")]
fn user_presses_q(model: &mut MyAppModel) {
    model.update(KeyMsg::new(KeyCode::Char('q'), KeyModifiers::NONE));
}

#[then("a quit confirmation dialog is shown")]
fn quit_dialog_shown(model: &MyAppModel) {
    let output = model.view();
    // Check that the output contains the expected dialog text
    // (We can do a snapshot or a simpler contains check for a specific substring)
    assert!(output.contains("Quit?"), "Dialog text not found in output");
}

#[then("the dialog asks \"Quit? (y/N)\"")]
fn quit_dialog_correct(model: &MyAppModel) {
    let output = model.view();
    // Using snapshot to verify the entire dialog screen (ensuring formatting is correct)
    insta::assert_snapshot!(output);
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
`&MyAppModel` (if it’s just checking). We used a mix of assertions: a basic
`assert!(contains)` for one Then step and a full `assert_snapshot!` for the
final state. You could choose to only use snapshots (especially if the dialog
output spans multiple lines or has colors you want to verify), or use
fine-grained asserts for specific elements and reserve snapshot for the full
screen. Both approaches are valid and can complement each other.

**Note:** The scenario’s test function (annotated with `#[scenario]`) runs
after the steps, meaning by the time we reach its body, all Given/When/Then
have executed. In the example, we left the body empty because the checks were
in the Then steps. You could also perform additional verification in the
scenario body if needed. Each scenario will appear as a separate test in
`cargo test` output, named after the scenario. The insta snapshot within will
be named accordingly (often including the scenario name or test function name –
you can always override the name in the macro if needed).

The advantage of using rstest-bdd is clarity: anyone reading the test can see
the narrative of the user interaction. It also encourages reusing fixtures (the
`model` in this case) and separating the action from the verification. We
could, for instance, have multiple scenarios reuse the same `when` step for
pressing "q" if they start from different states.

**Isolation:** Each scenario gets its own fresh fixture instances, so one
scenario’s state changes won’t leak into another. This is critical for snapshot
tests – if a previous test left the model in some mutated state or didn’t reset
a global, your next test’s snapshot might be inconsistent. Use fixtures to
manage setup/teardown if needed. For example, if your TUI writes to a file or
uses a global config, reset or stub those in a fixture. Snapshot tests should
be deterministic and independent.

## Using Insta Effectively (Redactions, Filters, Snapshot Organization)

With insta, beyond the basics of `assert_snapshot!`, there are some useful
features to make your life easier when testing TUIs:

- **Snapshot names and organization:** By default, insta names the snapshot
  file based on the test function name (and scenario/parameter, if applicable).
  You can override the name by passing a string as the first argument to
  `assert_snapshot!`. For example, in a single test that interacts and
  snapshots multiple screens, you might do:

```rust
assert_snapshot!("screen1_main_menu", output1);
// ... perform some actions ...
assert_snapshot!("screen2_after_delete", output2);
```

This will produce files like `my_test__screen1_main_menu.snap` and
`my_test__screen2_after_delete.snap`. Use descriptive names to identify what
each snapshot represents. In a BDD scenario, if you want the snapshot file to
incorporate scenario details, you could include a placeholder in the feature
and pass it as an argument to the Then step (e.g.,
`Then the screen should match snapshot "after_delete"` and use that string in
the `assert_snapshot!`). Otherwise, the snapshot will likely use the test name
and a counter.

- **Redacting sensitive or irrelevant data:** We touched on filters earlier.
  Insta also supports structured **redactions** for serde-serializable data,
  but since our output is a plain string, regex filters are the way to go.
  Common use cases:

- Redacting timestamps, as mentioned.

- Redacting random IDs or memory addresses if any appear in the UI.

- Normalizing whitespace if needed (though generally you want to keep exact
  whitespace in a TUI snapshot). For instance, if your UI draws a progress bar
  with changing lengths, you might choose to replace the numeric percentage
  with `[progress]` if verifying the exact percentage isn’t important for the
  test.

You can define filters globally for all tests by calling `Settings::add_filter`
at the beginning of your test module (or using `with_settings!` macro in insta
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

This would replace any string that looks like a datetime `YYYY-MM-DD HH:MM:SS`
with `<TIMESTAMP>` before comparing or writing the snapshot. The filters make
it so you don’t have to manually post-process the string every time in your
test code; it’s handled during the snapshot assertion.

- **Snapshot file location:** By default, insta will create a `snapshots`
  directory in the same folder as your test file (for a unit test in the main
  crate) or in the crate’s root for integration tests, with filenames derived
  from the test name. If you prefer to keep all snapshots in a single place or
  need to adjust the path (say, if running in a workspace with multiple
  crates), you can use `Settings::set_snapshot_path`. For instance:

```rust
Settings::clone_current()
    .set_snapshot_path("../ui_snapshots")
    .bind(|| {
        assert_snapshot!(output);
    });
```

This would tell insta to look in a directory relative to the current one. In
most cases, the default is fine. Organize your tests in a way that each
component or feature has its own test module, which will naturally group the
snapshots.

- **Reviewing and updating snapshots:** When a snapshot test fails (because the
  output changed from the saved snapshot), use `cargo insta review` to
  interactively review differences. This will show you a colored diff of
  expected vs actual output. For TUIs, this might highlight even subtle
  alignment changes. If the changes are intentional, you can approve them,
  which updates the `.snap` file. If not, you investigate which commit/code
  caused the difference. It’s good practice to run `cargo insta review` as part
  of your workflow whenever you change UI code. Additionally, if you anticipate
  a lot of churn, you might batch updates: for example, if you purposely
  restyled the whole app, all snapshots will fail – you can use
  `cargo insta accept --all` for a quick update, but do skim through the diffs
  to ensure everything looks right. Remember that **any** change in the output,
  even whitespace or ANSI codes, will appear in the diff. This strictness is
  what gives snapshot tests their power.

One caveat to bear in mind: because snapshot tests assert the entire screen
content, if your UI changes frequently (e.g., dynamic layouts), you may end up
updating snapshots often. The Ratatui documentation suggests reviewing
snapshots only after significant updates to avoid CI noise. In practical terms,
try to strike a balance – don’t disable the tests, but maybe group minor
cosmetic changes so you update multiple snapshots in one go. Also, consider
writing more focused tests for critical logic (like “pressing X increases the
counter by 1” could be a normal unit test on the model state) and reserve
snapshot tests for verifying the *presentation* of that state.

## Handling Non-Deterministic Elements and Caveats

Snapshot testing Bubbletea UIs does come with some challenges, but they can be
managed:

- **Timing-dependent behavior:** If your UI has animations or time-driven
  changes (spinners, clocks, auto-refresh lists, etc.), ensure your test either
  freezes time or captures a specific moment. For example, if an animated
  spinner advances every 200ms via a tick `Msg`, you can decide to test it at
  the initial state (no ticks applied), or after a certain number of ticks. You
  might simulate “fast-forwarding” time by calling update with a tick message
  multiple times. Alternatively, for things like an ASCII spinner, you might
  simply exclude it from test verification if it’s not important (or assert
  that it’s one of the known spinner frames, rather than doing a snapshot on
  it). The key is to avoid races or sleep calls in tests. All inputs and events
  should be fed synchronously.

- **Randomness:** We covered seeding random number generators. Another pattern
  is to use dependency injection for any random or external data. If your view
  calls a function to get a random quote, in tests you can override that
  function (perhaps by configuring the model with a predictable quote
  provider). The fewer unpredictable sources, the better. That said, it’s
  acceptable to use insta’s redactions to blank out truly random strings and
  just confirm the rest of the layout. You might lose some test rigor (you’re
  not checking the exact random content), but you preserve layout validation.

- **External resources:** If your TUI prints data fetched from a server or
  file, you don’t want your test to rely on the real resource. Use test doubles
  or sample data. For example, if on startup your app loads a config file and
  displays some values, in a test you can have the model use a temp file or
  dummy config instead. Then the snapshot will contain that dummy data. The
  snapshot essentially asserts that whatever data is present is correctly
  rendered – so as long as the structure is the same, using fake data is fine.

- **Terminal quirks:** Bubbletea (like many TUIs) uses special control codes
  for things like hiding the cursor, clearing the screen, or switching to
  alternate screen. When calling `model.view()` directly, you typically get
  just the content, not those setup/teardown codes (since those are handled by
  the `Program` when running for real). If you find any stray characters in
  your snapshot that correspond to such codes, you can filter them out. In most
  cases, you won’t see them because `view()` returns only what you draw (e.g.,
  via Lipgloss or text strings). The **bubbletea-rs** `Program` takes care of
  terminal initialization (entering alternate screen, etc.) which we are
  bypassing in these tests. That’s actually beneficial because it means our
  snapshots focus purely on UI content.

- **Platform differences:** Make sure your tests produce the same output on
  different platforms. If you use characters that might not render the same
  (for example, Windows console might not handle certain Unicode), consider
  that the snapshot will be based on whichever environment ran it. Usually,
  using standard UTF-8 characters and ANSI escapes is fine across OSes as long
  as you use a consistent C locale. If your CI and local environment differ
  (say, line ending differences or locale issues that change unicode icons),
  you may need to normalize those (for instance, always output `\n` as line
  separator, and open files in text mode accordingly). This is generally not a
  problem, but it’s worth keeping in the back of your mind if a snapshot passes
  locally but fails on CI due to an encoding issue.

- **Updating snapshots vs. test assertions:** A golden-file test will alert you
  to *any* change, but it won’t tell you if that change is good or bad – that’s
  up to you during review. For experienced developers, it’s often clear when a
  diff is expected (e.g., you intentionally changed a label) versus a
  regression. But be disciplined: if a diff shows something you didn’t expect,
  investigate your code because you might have broken something subtle. This is
  where snapshot tests shine: they can catch UI regressions that wouldn’t crash
  the program but would degrade user experience. For example, a refactor might
  accidentally remove a highlight or misalign text. A snapshot test failure
  would show the before/after of the UI, prompting you to notice the issue.

To give a concrete example, one developer of a Bubbletea app retrofitted
snapshot tests and noted it forced him to think deeply about the app’s
architecture and state handling. It’s often during this process you realize you
need to separate side-effects from pure updates, or that you could reorganize
code to be more testable. Embrace those improvements – your TUI code will
become cleaner and more maintainable.

Finally, keep in mind that snapshot tests complement other testing methods;
they shouldn’t be the only tests. They cover “did the UI look as expected” very
well, but they don’t directly tell you *why* a change happened. If a snapshot
test fails, you might then write a quick unit test or debug the model’s state
transitions to pinpoint the bug. Also, for logic-heavy components, traditional
assertions on the model state can be simpler and more robust. Use snapshot
tests when verifying the drawn output is important – layout, text content,
etc., especially in combination with multiple inputs where writing individual
assertions would be laborious.

## Running the Tests and Interpreting Results

Once you have your snapshot tests written, run them with `cargo test`. The
first run (or whenever you add new tests) will create initial `.snap` files.
Inspect them to ensure they contain what you expect (you can open them in any
text editor – they show the captured screen content). If a test fails due to a
snapshot mismatch, use `cargo insta review` to see the differences side by
side. You can run `cargo insta review --accept` (or press the accept key in
interactive mode) to accept new snapshots if the change is intended. Committing
the updated snapshots to version control will make future test runs use those
as the baseline.

In CI, you’d typically have snapshot tests run as part of `cargo test`. If
there’s a failure, the CI logs will show which snapshot didn’t match. You can
even have the CI artifacts include the new snapshot suggestions for manual
download and inspection. However, it’s often easier to reproduce the failure
locally, run the review, and then update the files.

**Example output:** Suppose you accidentally changed a border character in your
UI from `│` to `|`. A snapshot diff might look like:

```diff
 - │ Item 1
 - │ Item 2
 + | Item 1
 + | Item 2
```

This small difference would fail the test. If it’s a regression (you intended
to keep the fancy box drawing character), you know exactly what to fix in your
view code. If it was intentional (perhaps simplifying to ASCII), you accept the
change and the new snapshot will have the `|`. The snapshot review diff is
essentially a visual review of your TUI, which is quite fitting – it’s almost
like *looking at the UI* side-by-side before and after.

As a rule of thumb, treat your snapshots as living documentation of your TUI.
Reading through a `.snap` file should give a reasonable picture of what the
screen looks like (even though color codes and some alignment might be harder
to grok in raw text). Some developers even include representative snapshot
files in their docs or pull requests to show what the UI output is. Since insta
stores snapshots as plain text, they work well for this purpose.

## Conclusion

Snapshot testing a Bubbletea-rs application with insta allows you to verify
terminal UI output with confidence and ease. By capturing the full screen state
after a series of simulated inputs, you create a robust regression test that
will flag any unintended UI change. We covered how to set up a stable test
environment (fixed terminal size, controlled inputs), how to integrate with
rstest’s powerful fixture and BDD syntax for clarity, and how to handle tricky
dynamic aspects via insta’s redactions/filters. The result is a suite of tests
that act as a safety net for your TUI: refactor the code fearlessly, and let
the snapshots tell you if anything looks different.

Keep in mind that snapshot tests are most effective when you curate them –
focus on key states of the UI (no need to snapshot every single possible screen
if it’s not necessary), and keep dynamic data in check. When used
appropriately, they can be **“golden files”** for your project’s behavior,
giving you quick feedback on changes. As you maintain your app, update the
snapshots intentionally when you change the UI, and ensure they remain
up-to-date with the expected output.

By addressing determinism (for example, seeding randoms and fixing timestamps)
and isolating each test scenario, you also ensure that tests run reliably in CI
and don’t produce flaky failures. Each test is effectively reproducing a user’s
journey in a controlled way. This approach is reminiscent of end-to-end tests
but executed at the program logic level, which is why it strikes a good balance
between coverage and maintainability.

In summary, for an experienced Rust developer: *leverage insta to assert your
Bubbletea app’s text-based UI just as you would assert a data structure*. You
get the benefits of quick diffing and approval workflow, with the rich semantic
context of seeing your terminal UI’s content. When a test fails, you’ll
immediately *see* what changed in the UI, which is incredibly valuable.
Combined with rstest-bdd, your test code can read almost like a specification
of the UI’s behavior. This not only helps catch bugs but also serves as
documentation for how the TUI is supposed to react to input.

Happy testing, and enjoy the confidence that comes from knowing your terminal
interface is thoroughly checked by your automated tests!

**Sources:**

- Bubbletea TUI testing approaches and snapshot philosophy

- Ratatui snapshot testing recipe (using a fixed 80×20 terminal and insta)

- Charm’s Bubble Tea teatest (Go) using golden files for full output comparison

- Insta crate documentation on filters and snapshot review
