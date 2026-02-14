//! Scenario state and stubs for Codex execution behavioural tests.

use std::sync::Mutex;

use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

use frankie::ai::{
    CodexExecutionHandle, CodexExecutionRequest, CodexExecutionService, CodexExecutionUpdate,
};
use frankie::github::models::test_support::minimal_review;
use frankie::github::{IntakeError, PersonalAccessToken, PullRequestLocator};
use frankie::tui::app::ReviewApp;

type TimedUpdates = Vec<(u64, CodexExecutionUpdate)>;

/// Shared scenario state for Codex execution behaviour tests.
#[derive(ScenarioState, Default)]
pub(crate) struct CodexExecState {
    /// App under test.
    pub(crate) app: Slot<ReviewApp>,
    /// Temporary directory for transcript files.
    pub(crate) temp_dir: Slot<tempfile::TempDir>,
    /// Path to the transcript file for the scenario.
    pub(crate) transcript_path: Slot<String>,
}

/// Stub execution plans used by the fake Codex execution service.
#[derive(Debug)]
pub(crate) enum StubPlan {
    /// Emit the provided updates over time.
    TimedUpdates(TimedUpdates),
}

/// Fake execution service for deterministic TUI behavioural tests.
#[derive(Debug)]
struct StubCodexExecutionService {
    plan: Mutex<Option<StubPlan>>,
}

impl StubCodexExecutionService {
    const fn new(plan: StubPlan) -> Self {
        Self {
            plan: Mutex::new(Some(plan)),
        }
    }
}

fn sleep_for_delay(delay_ms: u64) {
    if delay_ms == 0 {
        return;
    }

    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
}

fn spawn_timed_updates(
    timed_updates: TimedUpdates,
) -> std::sync::mpsc::Receiver<CodexExecutionUpdate> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let (immediate, delayed) = partition_by_delay(timed_updates);
    send_immediate_updates(&sender, immediate);
    spawn_delayed_sender(sender, delayed);
    receiver
}

/// Partition updates into immediate (leading zero-delay) and delayed groups.
fn partition_by_delay(
    timed_updates: TimedUpdates,
) -> (Vec<CodexExecutionUpdate>, Vec<(u64, CodexExecutionUpdate)>) {
    let mut immediate = Vec::new();
    let mut delayed = Vec::new();
    let mut delay_phase_started = false;

    for (delay_ms, update) in timed_updates {
        if !delay_phase_started && delay_ms == 0 {
            immediate.push(update);
        } else {
            delay_phase_started = true;
            delayed.push((delay_ms, update));
        }
    }

    (immediate, delayed)
}

/// Send immediate updates synchronously to avoid races under coverage instrumentation.
fn send_immediate_updates(
    sender: &std::sync::mpsc::Sender<CodexExecutionUpdate>,
    updates: Vec<CodexExecutionUpdate>,
) {
    for update in updates {
        drop(sender.send(update));
    }
}

/// Spawn background thread to send delayed updates.
fn spawn_delayed_sender(
    sender: std::sync::mpsc::Sender<CodexExecutionUpdate>,
    delayed_updates: Vec<(u64, CodexExecutionUpdate)>,
) {
    if !delayed_updates.is_empty() {
        std::thread::spawn(move || {
            for (delay_ms, update) in delayed_updates {
                sleep_for_delay(delay_ms);
                drop(sender.send(update));
            }
        });
    }
}

impl CodexExecutionService for StubCodexExecutionService {
    fn start(&self, _request: CodexExecutionRequest) -> Result<CodexExecutionHandle, IntakeError> {
        let mut plans = self.plan.lock().map_err(|error| IntakeError::Api {
            message: format!("failed to lock stub plan: {error}"),
        })?;

        let Some(next_plan) = plans.take() else {
            return Err(IntakeError::Api {
                message: "stub plan was already consumed".to_owned(),
            });
        };

        match next_plan {
            StubPlan::TimedUpdates(timed_updates) => {
                let receiver = spawn_timed_updates(timed_updates);
                Ok(CodexExecutionHandle::new(receiver))
            }
        }
    }
}

/// Creates a review app configured with a deterministic Codex service plan.
///
/// # Errors
///
/// Returns [`IntakeError`] if refresh context setup fails.
pub(crate) fn app_with_plan(plan: StubPlan) -> Result<ReviewApp, IntakeError> {
    ensure_refresh_context()?;
    let service = std::sync::Arc::new(StubCodexExecutionService::new(plan));
    Ok(ReviewApp::new(sample_reviews()).with_codex_service(service))
}

fn ensure_refresh_context() -> Result<(), IntakeError> {
    let locator = PullRequestLocator::parse("https://github.com/owner/repo/pull/42")?;
    let token = PersonalAccessToken::new("test-token")?;
    // Returns false when already initialised; safe to ignore in tests
    // sharing a process-wide OnceLock.
    let _already_set = frankie::tui::set_refresh_context(locator, token);
    Ok(())
}

fn sample_reviews() -> Vec<frankie::github::models::ReviewComment> {
    vec![frankie::github::models::ReviewComment {
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(12),
        body: Some("Fix this branch".to_owned()),
        ..minimal_review(1, "Fix this branch", "alice")
    }]
}
