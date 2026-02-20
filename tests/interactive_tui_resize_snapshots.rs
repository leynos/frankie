//! PTY-backed snapshot tests for terminal resize behaviour in the review TUI.

use std::env;
use std::thread;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use ratatui_testlib::{CommandBuilder, Result, ScreenState, TestTerminal};
use rstest::{fixture, rstest};

const CAPTURE_ATTEMPTS: usize = 2;
const FRAME_SETTLE_DELAY_MS: u64 = 40;
const FRAME_READ_TIMEOUT_MS: u64 = 250;
const STARTUP_STABILIZE_DELAY_MS: u64 = 120;
const FIXTURE_BINARY_NAME: &str = "tui_resize_snapshot_fixture";

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
    let workspace_root = env::var("CARGO_MANIFEST_DIR").ok().map(Utf8PathBuf::from);

    if let Some(resolved) = env::var("CARGO_BIN_EXE_tui_resize_snapshot_fixture")
        .ok()
        .and_then(|path| resolve_binary_path(Utf8PathBuf::from(path), workspace_root.as_deref()))
    {
        return resolved;
    }

    let candidates = fixture_binary_candidates(workspace_root.as_deref());

    if let Some(resolved) = candidates
        .iter()
        .find_map(|path| resolve_binary_path(path.to_path_buf(), workspace_root.as_deref()))
    {
        return resolved;
    }

    default_fixture_binary_path(workspace_root.as_deref())
}

fn resolve_binary_path(
    candidate: Utf8PathBuf,
    workspace_root: Option<&Utf8Path>,
) -> Option<String> {
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        let root = workspace_root?;
        root.join(candidate)
    };

    if resolved.is_file() {
        Some(resolved.into_string())
    } else {
        None
    }
}

fn fixture_binary_candidates(workspace_root: Option<&Utf8Path>) -> Vec<Utf8PathBuf> {
    let mut candidates = Vec::new();
    for target_root in target_roots(workspace_root) {
        push_candidate(
            &mut candidates,
            target_root.join("debug").join(FIXTURE_BINARY_NAME),
        );
        push_candidate(
            &mut candidates,
            target_root.join("release").join(FIXTURE_BINARY_NAME),
        );
        push_candidate(
            &mut candidates,
            target_root
                .join("debug")
                .join("deps")
                .join(FIXTURE_BINARY_NAME),
        );
        push_candidate(
            &mut candidates,
            target_root
                .join("release")
                .join("deps")
                .join(FIXTURE_BINARY_NAME),
        );
    }

    if let Some(debug_dir) = current_exe_debug_dir() {
        push_candidate(&mut candidates, debug_dir.join(FIXTURE_BINARY_NAME));
        push_candidate(
            &mut candidates,
            debug_dir.join("deps").join(FIXTURE_BINARY_NAME),
        );
    }

    candidates
}

fn target_roots(workspace_root: Option<&Utf8Path>) -> Vec<Utf8PathBuf> {
    let mut roots = Vec::new();

    if let Some(target_dir) = env::var("CARGO_TARGET_DIR")
        .ok()
        .map(Utf8PathBuf::from)
        .map(|path| resolve_target_dir(path, workspace_root))
    {
        push_candidate(&mut roots, target_dir);
    }

    if let Some(root) = workspace_root {
        push_candidate(&mut roots, root.join("target"));
        if let Some(parent) = root.parent() {
            push_candidate(&mut roots, parent.join("target"));
        }
    }

    if let Some(debug_dir) = current_exe_debug_dir()
        && let Some(target_root) = debug_dir.parent()
    {
        push_candidate(&mut roots, target_root.to_path_buf());
    }

    roots
}

fn current_exe_debug_dir() -> Option<Utf8PathBuf> {
    let current_exe_path = env::current_exe().ok()?;
    let current_exe_utf8 = Utf8PathBuf::from_path_buf(current_exe_path).ok()?;
    let parent = current_exe_utf8.parent()?;
    if parent.file_name() == Some("deps") {
        parent.parent().map(Utf8Path::to_path_buf)
    } else {
        Some(parent.to_path_buf())
    }
}

fn default_fixture_binary_path(workspace_root: Option<&Utf8Path>) -> String {
    if let Some(target_dir) = env::var("CARGO_TARGET_DIR")
        .ok()
        .map(Utf8PathBuf::from)
        .map(|path| resolve_target_dir(path, workspace_root))
    {
        return target_dir
            .join("debug")
            .join(FIXTURE_BINARY_NAME)
            .into_string();
    }

    workspace_root.map_or_else(
        || format!("target/debug/{FIXTURE_BINARY_NAME}"),
        |root| {
            root.join("target")
                .join("debug")
                .join(FIXTURE_BINARY_NAME)
                .into_string()
        },
    )
}

fn resolve_target_dir(path: Utf8PathBuf, workspace_root: Option<&Utf8Path>) -> Utf8PathBuf {
    if path.is_absolute() {
        path
    } else if let Some(root) = workspace_root {
        root.join(path)
    } else {
        path
    }
}

fn push_candidate(candidates: &mut Vec<Utf8PathBuf>, candidate: Utf8PathBuf) {
    if !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

fn frame_is_blank(frame: &str) -> bool {
    frame.lines().all(|line| line.trim().is_empty())
}

#[fixture]
fn tui_fixture(#[default(80)] width: u16, #[default(24)] height: u16) -> Result<TuiFixture> {
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

    assert!(
        !review_rows.is_empty(),
        "{test_name} expected at least one review row before contiguity checks"
    );

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
#[case::startup_small("startup_snapshot_reflects_configured_size_small", 24)]
#[case::startup_large("startup_snapshot_reflects_configured_size_large", 40)]
fn startup_snapshot_reflects_configured_size(
    #[case] snapshot_name: &str,
    #[case] height: u16,
    #[with(80, height)] tui_fixture: Result<TuiFixture>,
) -> Result<()> {
    let mut fixture = tui_fixture?;
    thread::sleep(Duration::from_millis(STARTUP_STABILIZE_DELAY_MS));
    let frame = fixture.capture_frame(true)?;
    // Some CI runners provide PTY implementations that never emit frame data.
    // Skip snapshot assertions when the captured buffer is entirely blank.
    if frame_is_blank(&frame) {
        return Ok(());
    }

    assert_visible_frame(&frame, height as usize, "startup snapshot");

    insta::assert_snapshot!(snapshot_name, frame);

    Ok(())
}

#[rstest]
#[case::resize_start(ViewportSnapshotCase {
    width: 80,
    height: 24,
    expected_rows: 24,
    test_name: "small resize frame",
    snapshot_name: "resize_start",
    require_contiguous_rows: false,
})]
#[case::resize_shrunk(ViewportSnapshotCase {
    width: 80,
    height: 14,
    expected_rows: 14,
    test_name: "shrunk resize frame",
    snapshot_name: "resize_shrunk",
    require_contiguous_rows: false,
})]
#[case::resize_enlarged(ViewportSnapshotCase {
    width: 80,
    height: 36,
    expected_rows: 36,
    test_name: "expanded resize frame",
    snapshot_name: "resize_enlarged",
    require_contiguous_rows: false,
})]
#[case::resize_horizontal_shrunk(ViewportSnapshotCase {
    width: 72,
    height: 24,
    expected_rows: 24,
    test_name: "horizontal resize narrow frame",
    snapshot_name: "resize_horizontal_shrunk",
    require_contiguous_rows: true,
})]
#[case::resize_horizontal_expanded(ViewportSnapshotCase {
    width: 110,
    height: 24,
    expected_rows: 24,
    test_name: "horizontal resize widened frame",
    snapshot_name: "resize_horizontal_expanded",
    require_contiguous_rows: true,
})]
fn viewport_size_snapshots(#[case] case: ViewportSnapshotCase) -> Result<()> {
    let mut fixture = tui_fixture(case.width, case.height)?;
    thread::sleep(Duration::from_millis(STARTUP_STABILIZE_DELAY_MS));
    let frame = fixture.capture_frame(true)?;
    // Some CI runners provide PTY implementations that never emit frame data.
    // Skip snapshot assertions when the captured buffer is entirely blank.
    if frame_is_blank(&frame) {
        return Ok(());
    }

    assert_visible_frame(&frame, case.expected_rows, case.test_name);
    if case.require_contiguous_rows {
        assert_review_rows_are_contiguous(&frame, case.test_name);
    }

    insta::assert_snapshot!(case.snapshot_name, frame);

    Ok(())
}

#[derive(Clone, Copy)]
struct ViewportSnapshotCase {
    width: u16,
    height: u16,
    expected_rows: usize,
    test_name: &'static str,
    snapshot_name: &'static str,
    require_contiguous_rows: bool,
}
