//! PTY-backed snapshot tests for terminal resize behaviour in the review TUI.

use std::env;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use ratatui_testlib::{
    events::{KeyCode, KeyEvent},
    CommandBuilder, Result, ScreenState, TestTerminal,
};
use rstest::rstest;

const CAPTURE_ATTEMPTS: usize = 2;
const FRAME_SETTLE_DELAY_MS: u64 = 40;
const FRAME_READ_TIMEOUT_MS: u64 = 250;
const STARTUP_STABILISE_DELAY_MS: u64 = 120;

struct TuiFixture {
    terminal: TestTerminal,
    state: ScreenState,
}

impl TuiFixture {
    fn new(width: u16, height: u16) -> Result<Self> {
        let mut terminal = TestTerminal::new(width, height)?;
        let mut command = CommandBuilder::new(fixture_binary_path());
        let state = ScreenState::new(width, height);

        command.env("NO_COLOR", "1");
        command.args(&["--auto-exit-ms", "4500"]);
        terminal.spawn(command)?;

        Ok(Self { terminal, state })
    }

    fn resize(&mut self, width: u16, height: u16) -> Result<()> {
        self.terminal.resize(width, height)?;
        self.state = ScreenState::new(width, height);
        Ok(())
    }

    fn capture_frame(&mut self, with_probe: bool) -> Result<String> {
        if with_probe {
            self.send_probe_keys()?;
        }

        self.drain_for_frame()?;
        Ok(self.state.contents())
    }

    fn send_probe_keys(&mut self) -> Result<()> {
        let down = KeyEvent::new(KeyCode::Down);
        let up = KeyEvent::new(KeyCode::Up);
        let mut input = Vec::with_capacity(down.to_bytes().len() + up.to_bytes().len());

        input.extend_from_slice(&down.to_bytes());
        input.extend_from_slice(&up.to_bytes());
        thread::sleep(Duration::from_millis(20));

        self.terminal.write_all(&input)?;
        thread::sleep(Duration::from_millis(FRAME_SETTLE_DELAY_MS));

        Ok(())
    }

    fn drain_for_frame(&mut self) -> Result<()> {
        let mut bytes = vec![0_u8; 16_384];
        let mut got_data = false;

        for _ in 0..CAPTURE_ATTEMPTS {
            thread::sleep(Duration::from_millis(20));
            match self
                .terminal
                .read_timeout(&mut bytes, Duration::from_millis(FRAME_READ_TIMEOUT_MS))
            {
                Ok(0) => {
                    if got_data {
                        break;
                    }

                    continue;
                }
                Ok(length) => {
                    self.state.feed(&bytes[..length]);
                    got_data = true;
                    if length == bytes.len() {
                        continue;
                    }

                    break;
                }
                Err(ratatui_testlib::TermTestError::Timeout { .. }) => {
                    break;
                }
                Err(error) => return Err(error),
            }
        }

        Ok(())
    }
}

fn fixture_binary_path() -> String {
    let workspace_root = env::var("CARGO_MANIFEST_DIR").ok().map(PathBuf::from);

    if let Some(explicit_path) = env::var_os("CARGO_BIN_EXE_tui_resize_snapshot_fixture") {
        let explicit = PathBuf::from(explicit_path);
        if let Some(resolved) = resolve_binary_path(explicit, workspace_root.as_ref()) {
            return resolved;
        }
    }

    let candidates = [
        PathBuf::from("target/debug/tui_resize_snapshot_fixture"),
        PathBuf::from("target/release/tui_resize_snapshot_fixture"),
        PathBuf::from("target/debug/deps/tui_resize_snapshot_fixture"),
        PathBuf::from("target/release/deps/tui_resize_snapshot_fixture"),
        PathBuf::from("../target/debug/tui_resize_snapshot_fixture"),
        PathBuf::from("../target/release/tui_resize_snapshot_fixture"),
    ];

    for candidate in candidates {
        if let Some(resolved) = resolve_binary_path(candidate, workspace_root.as_ref()) {
            return resolved;
        }
    }

    if let Some(root) = workspace_root {
        root.join("target/debug/tui_resize_snapshot_fixture")
            .to_string_lossy()
            .into_owned()
    } else {
        String::from("target/debug/tui_resize_snapshot_fixture")
    }
}

fn resolve_binary_path(candidate: PathBuf, workspace_root: Option<&PathBuf>) -> Option<String> {
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        let root = workspace_root?;
        root.join(candidate)
    };

    if resolved.is_file() {
        resolved.to_str().map(str::to_owned)
    } else {
        None
    }
}

fn spawn_tui_fixture(width: u16, height: u16) -> Result<TuiFixture> {
    TuiFixture::new(width, height)
}

#[rstest]
#[case::startup_small(80, 24)]
#[case::startup_large(80, 40)]
fn startup_snapshot_reflects_configured_size(#[case] width: u16, #[case] height: u16) -> Result<()> {
    let mut fixture = spawn_tui_fixture(width, height)?;
    thread::sleep(Duration::from_millis(STARTUP_STABILISE_DELAY_MS));
    let frame = fixture.capture_frame(true)?;

    assert!(
        frame.contains("Frankie - Review Comments"),
        "startup frame should include the app header"
    );
    assert!(frame.contains("Filter:"), "startup frame should include the filter bar");
    assert!(
        frame.lines().count() >= 3,
        "startup frame should contain visible content"
    );
    assert_eq!(
        frame.lines().count(),
        height as usize,
        "terminal height must be reflected in rendered rows"
    );

    // Ensure snapshots are captured for both small and large terminal heights.
    insta::assert_snapshot!(frame);

    Ok(())
}

#[test]
fn resize_sequence_captures_small_and_large_layouts() -> Result<()> {
    let mut fixture = spawn_tui_fixture(80, 24)?;
    thread::sleep(Duration::from_millis(STARTUP_STABILISE_DELAY_MS));

    let frame_small = fixture.capture_frame(true)?;

    fixture.resize(80, 14)?;
    let frame_shrunk = fixture.capture_frame(false)?;

    fixture.resize(80, 36)?;
    let frame_expanded = fixture.capture_frame(false)?;

    // Verify the expected state transitions are rendered and captured.
    assert!(frame_small.contains("Frankie - Review Comments"));
    assert!(frame_shrunk.contains("Frankie - Review Comments"));
    assert!(frame_expanded.contains("Frankie - Review Comments"));
    assert_eq!(frame_small.lines().count(), 24, "small frame should use 24 rows");
    assert_eq!(frame_shrunk.lines().count(), 14, "shrunk frame should use 14 rows");
    assert_eq!(
        frame_expanded.lines().count(),
        36,
        "expanded frame should use 36 rows"
    );

    insta::assert_snapshot!("resize_start", frame_small);
    insta::assert_snapshot!("resize_shrunk", frame_shrunk);
    insta::assert_snapshot!("resize_enlarged", frame_expanded);

    Ok(())
}
