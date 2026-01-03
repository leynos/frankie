//! Scenario state for review sync BDD tests.

use std::sync::Arc;

pub(crate) use frankie::github::models::test_support::{create_reviews, review_with_id};
use frankie::telemetry::test_support::RecordingTelemetrySink;
use frankie::tui::app::ReviewApp;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

/// State shared across steps in a review sync scenario.
#[derive(ScenarioState, Default)]
pub(crate) struct SyncState {
    /// The TUI application model under test.
    pub(crate) app: Slot<ReviewApp>,
    /// Recording telemetry sink for capturing events.
    pub(crate) telemetry_sink: Slot<Arc<RecordingTelemetrySink>>,
    /// Last recorded latency in milliseconds.
    pub(crate) last_latency_ms: Slot<u64>,
    /// Whether the telemetry sink was successfully wired (`OnceLock` first-writer wins).
    pub(crate) telemetry_wired: Slot<bool>,
}
