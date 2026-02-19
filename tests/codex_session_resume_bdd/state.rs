//! Scenario state and stubs for Codex session resumption behavioural tests.

use std::sync::Mutex;

use camino::Utf8PathBuf;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

use frankie::ai::{
    CodexExecutionHandle, CodexExecutionRequest, CodexExecutionService, CodexExecutionUpdate,
    CodexResumeRequest,
};
use frankie::github::models::test_support::minimal_review;
use frankie::github::{IntakeError, PersonalAccessToken, PullRequestLocator};
use frankie::tui::app::ReviewApp;

type TimedUpdates = Vec<(u64, CodexExecutionUpdate)>;

const SAMPLE_FILE_PATH: &str = "src/main.rs";
const SAMPLE_LINE_NUMBER: u32 = 12;
const SAMPLE_COMMENT_BODY: &str = "Fix this branch";
const SAMPLE_REVIEWER: &str = "alice";

/// Shared scenario state for Codex session resumption behaviour tests.
#[derive(ScenarioState, Default)]
pub(crate) struct ResumeScenarioState {
    /// App under test.
    pub(crate) app: Slot<ReviewApp>,
}

/// Stub execution plans used by the fake Codex execution service.
#[derive(Debug)]
pub(crate) enum StubResumePlan {
    /// Emit the provided updates over time for a fresh start.
    FreshUpdates(TimedUpdates),
    /// Emit the provided updates over time for a resume.
    ResumeUpdates(TimedUpdates),
}

/// Invocation kind used to validate start vs resume wiring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvocationKind {
    FreshStart,
    Resume,
}

impl InvocationKind {
    const fn as_label(self) -> &'static str {
        match self {
            Self::FreshStart => "start",
            Self::Resume => "resume",
        }
    }
}

/// Fake execution service that supports both start and resume.
#[derive(Debug)]
struct StubResumeCodexService {
    plan: Mutex<Option<StubResumePlan>>,
}

impl StubResumeCodexService {
    const fn new(plan: StubResumePlan) -> Self {
        Self {
            plan: Mutex::new(Some(plan)),
        }
    }

    /// Consumes the next plan and spawns timed updates.
    fn execute_plan(
        &self,
        invocation: InvocationKind,
    ) -> Result<CodexExecutionHandle, IntakeError> {
        let mut plans = self.plan.lock().map_err(|error| IntakeError::Api {
            message: format!("failed to lock stub plan: {error}"),
        })?;

        let Some(next_plan) = plans.take() else {
            return Err(IntakeError::Api {
                message: "stub plan was already consumed".to_owned(),
            });
        };

        match next_plan {
            StubResumePlan::FreshUpdates(timed_updates) => {
                if invocation != InvocationKind::FreshStart {
                    return Err(IntakeError::Api {
                        message: format!(
                            "stub plan expected start(), but {}() was called",
                            invocation.as_label()
                        ),
                    });
                }
                let receiver = spawn_timed_updates(timed_updates);
                Ok(CodexExecutionHandle::new(receiver))
            }
            StubResumePlan::ResumeUpdates(timed_updates) => {
                if invocation != InvocationKind::Resume {
                    return Err(IntakeError::Api {
                        message: format!(
                            "stub plan expected resume(), but {}() was called",
                            invocation.as_label()
                        ),
                    });
                }
                let receiver = spawn_timed_updates(timed_updates);
                Ok(CodexExecutionHandle::new(receiver))
            }
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

fn send_immediate_updates(
    sender: &std::sync::mpsc::Sender<CodexExecutionUpdate>,
    updates: Vec<CodexExecutionUpdate>,
) {
    for update in updates {
        drop(sender.send(update));
    }
}

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

impl CodexExecutionService for StubResumeCodexService {
    fn start(&self, _request: CodexExecutionRequest) -> Result<CodexExecutionHandle, IntakeError> {
        self.execute_plan(InvocationKind::FreshStart)
    }

    fn resume(&self, _request: CodexResumeRequest) -> Result<CodexExecutionHandle, IntakeError> {
        self.execute_plan(InvocationKind::Resume)
    }
}

/// Creates a review app configured with a deterministic Codex service plan.
///
/// # Errors
///
/// Returns [`IntakeError`] if refresh context setup fails.
pub(crate) fn app_with_resume_plan(plan: StubResumePlan) -> Result<ReviewApp, IntakeError> {
    ensure_refresh_context()?;
    let service = std::sync::Arc::new(StubResumeCodexService::new(plan));
    Ok(ReviewApp::new(sample_reviews()).with_codex_service(service))
}

fn ensure_refresh_context() -> Result<(), IntakeError> {
    let locator = PullRequestLocator::parse("https://github.com/owner/repo/pull/42")?;
    let token = PersonalAccessToken::new("test-token")?;
    // Returns false when already initialised; safe to ignore in BDD tests
    // sharing a process-wide OnceLock.
    let _already_set = frankie::tui::set_refresh_context(locator, token);
    Ok(())
}

fn sample_reviews() -> Vec<frankie::github::models::ReviewComment> {
    vec![frankie::github::models::ReviewComment {
        file_path: Some(SAMPLE_FILE_PATH.to_owned()),
        line_number: Some(SAMPLE_LINE_NUMBER),
        body: Some(SAMPLE_COMMENT_BODY.to_owned()),
        ..minimal_review(1, SAMPLE_COMMENT_BODY, SAMPLE_REVIEWER)
    }]
}

/// Creates a sample interrupted session state for testing.
pub(crate) fn sample_interrupted_session() -> frankie::ai::SessionState {
    frankie::ai::SessionState {
        status: frankie::ai::SessionStatus::Interrupted,
        transcript_path: Utf8PathBuf::from("/tmp/frankie-bdd-interrupted.jsonl"),
        thread_id: Some("thr_bdd_test".to_owned()),
        owner: "owner".to_owned(),
        repository: "repo".to_owned(),
        pr_number: 42,
        started_at: chrono::Utc::now(),
        finished_at: Some(chrono::Utc::now()),
    }
}
