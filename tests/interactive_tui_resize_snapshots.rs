//! PTY-backed snapshot tests for terminal resize behaviour in the review TUI.

use std::env;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use ratatui_testlib::{CommandBuilder, Result, ScreenState, TestTerminal};
use rstest::rstest;

const CAPTURE_ATTEMPTS: usize = 2;
const FRAME_SETTLE_DELAY_MS: u64 = 40;
const FRAME_READ_TIMEOUT_MS: u64 = 250;
const STARTUP_STABILISE_DELAY_MS: u64 = 120;

struct TuiFixture {
    terminal: TestTerminal,
    state: ScreenState,
    width: u16,
    height: u16,
}

impl TuiFixture {
    fn new(width: u16, height: u16) -> Result<Self> {
        let mut terminal = TestTerminal::new(width, height)?;
        let mut command = CommandBuilder::new(fixture_binary_path());
        let state = ScreenState::new(width, height);

        command.env("NO_COLOR", "1");
        command.args(["--auto-exit-ms", "15000"]);
        terminal.spawn(command)?;

        Ok(Self {
            terminal,
            state,
            width,
            height,
        })
    }

    fn capture_frame(&mut self, with_probe: bool) -> Result<String> {
        if with_probe {
            self.send_resize_probe()?;
        }

        self.drain_for_frame()?;
        Ok(self.state.contents())
    }

    fn send_resize_probe(&mut self) -> Result<()> {
        // Jiggle terminal size to force a redraw event on PTY-backed apps.
        let probe_width = if self.width > 1 {
            self.width.saturating_sub(1)
        } else {
            self.width.saturating_add(1)
        };
        self.terminal.resize(probe_width, self.height)?;
        self.terminal.resize(self.width, self.height)?;
        thread::sleep(Duration::from_millis(FRAME_SETTLE_DELAY_MS));
        Ok(())
    }

    fn drain_for_frame(&mut self) -> Result<()> {
        let mut bytes = vec![0_u8; 16_384];
        let mut got_data = false;
        for _ in 0..CAPTURE_ATTEMPTS {
            thread::sleep(Duration::from_millis(20));

            let read_result = self
                .terminal
                .read_timeout(&mut bytes, Duration::from_millis(FRAME_READ_TIMEOUT_MS));

            match read_result {
                Ok(length) if length > 0 => {
                    self.state.feed(bytes.get(0..length).unwrap_or_default());
                    got_data = true;
                }
                Ok(0) | Err(ratatui_testlib::TermTestError::Timeout { .. }) if got_data => break,
                Ok(_) | Err(ratatui_testlib::TermTestError::Timeout { .. }) => {}
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

    workspace_root.map_or_else(
        || String::from("target/debug/tui_resize_snapshot_fixture"),
        |root| {
            root.join("target/debug/tui_resize_snapshot_fixture")
                .to_string_lossy()
                .into_owned()
        },
    )
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

fn assert_visible_frame(frame: &str, expected_rows: usize, test_name: &str) {
    let row_count = frame.lines().count();
    assert!(
        frame.contains("Frankie - Review Comments"),
        "{test_name} missing app header"
    );
    assert!(frame.contains("Filter:"), "{test_name} missing filter bar");
    assert!(row_count >= 3, "{test_name} must contain visible content");
    assert_eq!(
        row_count, expected_rows,
        "{test_name} expected {expected_rows} rows"
    );
}

fn assert_review_rows_are_contiguous(frame: &str, test_name: &str) {
    let review_rows: Vec<usize> = frame
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if line.starts_with("  [reviewer-") || line.starts_with("> [reviewer-") {
                Some(index)
            } else {
                None
            }
        })
        .collect();

    let has_gap = review_rows.windows(2).any(|pair| {
        if let [first, second] = pair {
            *second != first.saturating_add(1)
        } else {
            false
        }
    });
    assert!(
        !has_gap,
        "{test_name} unexpectedly contains blank rows between adjacent review rows"
    );
}

#[rstest]
#[case::startup_small("startup_snapshot_reflects_configured_size_small", 80, 24)]
#[case::startup_large("startup_snapshot_reflects_configured_size_large", 80, 40)]
fn startup_snapshot_reflects_configured_size(
    #[case] snapshot_name: &str,
    #[case] width: u16,
    #[case] height: u16,
) {
    let mut fixture = spawn_tui_fixture(width, height).expect("fixture should start");
    thread::sleep(Duration::from_millis(STARTUP_STABILISE_DELAY_MS));
    let frame = fixture.capture_frame(true).expect("capture startup frame");

    assert_visible_frame(&frame, height as usize, "startup snapshot");

    // Ensure snapshots are captured for both small and large terminal heights.
    match snapshot_name {
        "startup_snapshot_reflects_configured_size_small" => {
            insta::assert_snapshot!("startup_snapshot_reflects_configured_size_small", frame);
        }
        _ => {
            insta::assert_snapshot!("startup_snapshot_reflects_configured_size_large", frame);
        }
    }
}

#[test]
fn resize_sequence_captures_small_and_large_layouts() {
    let mut small_fixture = spawn_tui_fixture(80, 24).expect("fixture should start");
    thread::sleep(Duration::from_millis(STARTUP_STABILISE_DELAY_MS));
    let frame_small = small_fixture
        .capture_frame(true)
        .expect("capture initial layout frame");

    let mut shrunk_fixture = spawn_tui_fixture(80, 14).expect("fixture should start");
    thread::sleep(Duration::from_millis(STARTUP_STABILISE_DELAY_MS));
    let frame_shrunk = shrunk_fixture
        .capture_frame(true)
        .expect("capture shrunk layout frame");

    let mut expanded_fixture = spawn_tui_fixture(80, 36).expect("fixture should start");
    thread::sleep(Duration::from_millis(STARTUP_STABILISE_DELAY_MS));
    let frame_expanded = expanded_fixture
        .capture_frame(true)
        .expect("capture expanded layout frame");

    assert_visible_frame(&frame_small, 24, "small resize frame");
    assert_visible_frame(&frame_shrunk, 14, "shrunk resize frame");
    assert_visible_frame(&frame_expanded, 36, "expanded resize frame");

    insta::assert_snapshot!("resize_start", frame_small);
    insta::assert_snapshot!("resize_shrunk", frame_shrunk);
    insta::assert_snapshot!("resize_enlarged", frame_expanded);
}

#[test]
fn horizontal_resize_keeps_review_rows_contiguous() {
    let mut shrink_fixture = spawn_tui_fixture(72, 24).expect("fixture should start");
    thread::sleep(Duration::from_millis(STARTUP_STABILISE_DELAY_MS));
    let frame_narrow = shrink_fixture
        .capture_frame(true)
        .expect("capture narrow layout frame");

    let mut expand_fixture = spawn_tui_fixture(110, 24).expect("fixture should start");
    thread::sleep(Duration::from_millis(STARTUP_STABILISE_DELAY_MS));
    let frame_wide_again = expand_fixture
        .capture_frame(true)
        .expect("capture widened layout frame");

    assert_visible_frame(&frame_narrow, 24, "horizontal resize narrow frame");
    assert_visible_frame(&frame_wide_again, 24, "horizontal resize widened frame");
    assert_review_rows_are_contiguous(&frame_narrow, "horizontal resize narrow frame");
    assert_review_rows_are_contiguous(&frame_wide_again, "horizontal resize widened frame");

    insta::assert_snapshot!("resize_horizontal_shrunk", frame_narrow);
    insta::assert_snapshot!("resize_horizontal_expanded", frame_wide_again);
}
